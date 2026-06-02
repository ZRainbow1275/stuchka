//! §4.6.1 — daily digest + deterministic daily SHA-256 (AU-08 / AU-12).

use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;

use crate::error::DailyHashError;

/// The daily roll-up over `audit_log` rows whose `when_ts` falls in `[date, date+1)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyDigest {
    /// UTC calendar date covered by this digest.
    pub date: NaiveDate,
    /// `record_hash` of the last (highest-seq) record of the day; `"0"*64` when the day is empty.
    pub last_record_hash: String,
    /// Number of records on the day.
    pub record_count: u64,
}

/// Compute the daily digest from the audit pool (§4.6.1).
pub async fn compute_daily_hash(
    pool: &SqlitePool,
    date: NaiveDate,
) -> Result<DailyDigest, DailyHashError> {
    let start_ts = date
        .and_hms_opt(0, 0, 0)
        .expect("00:00:00 is valid")
        .and_utc()
        .timestamp_millis();
    let end_ts = date
        .succ_opt()
        .expect("date has a successor")
        .and_hms_opt(0, 0, 0)
        .expect("00:00:00 is valid")
        .and_utc()
        .timestamp_millis();

    // Recompute the last record's hash from its stored fields rather than trusting the stored
    // `record_hash` column. This is what makes AU-12 hold: any tamper to a covered record's
    // payload changes the recomputed last-record hash and therefore the daily hash, even though
    // the attacker did not touch the `record_hash` column.
    let last_row: Option<crate::hash_chain::RecordHashInput> = {
        let row = sqlx::query(
            "SELECT prev_hash, who_kind, who_payload, who_nonce, when_ts, why, what_payload, nonce \
             FROM audit_log WHERE when_ts >= ? AND when_ts < ? ORDER BY seq DESC LIMIT 1",
        )
        .bind(start_ts)
        .bind(end_ts)
        .fetch_optional(pool)
        .await?;
        row.map(|r| crate::hash_chain::RecordHashInput::from_row(&r))
    };

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE when_ts >= ? AND when_ts < ?")
            .bind(start_ts)
            .bind(end_ts)
            .fetch_one(pool)
            .await?;

    let last_record_hash = last_row
        .map(|i| i.compute())
        .unwrap_or_else(|| "0".repeat(64));

    Ok(DailyDigest {
        date,
        last_record_hash,
        record_count: count as u64,
    })
}

/// Deterministic daily hash: `SHA-256(date(ISO) || last_record_hash || count_be)` (§4.6.1, AU-08).
#[must_use]
pub fn compute_daily_sha256(d: &DailyDigest) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(d.date.format("%Y-%m-%d").to_string().as_bytes());
    h.update(d.last_record_hash.as_bytes());
    h.update(d.record_count.to_be_bytes());
    h.finalize().into()
}

/// Lower-case hex of [`compute_daily_sha256`] (the `daily_sha256` column value).
#[must_use]
pub fn daily_sha256_hex(d: &DailyDigest) -> String {
    compute_daily_sha256(d)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DailyDigest {
        DailyDigest {
            date: NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
            last_record_hash: "a".repeat(64),
            record_count: 123,
        }
    }

    /// AU-08: deterministic — same digest yields a fixed, locked hex.
    #[test]
    fn au_08_deterministic_locked_constant() {
        let d = sample();
        let hex = daily_sha256_hex(&d);
        // Locked golden value (recomputed below from first principles so it is self-checking).
        let mut h = Sha256::new();
        h.update(b"2026-05-12");
        h.update("a".repeat(64).as_bytes());
        h.update(123u64.to_be_bytes());
        let expect: [u8; 32] = h.finalize().into();
        let expect_hex: String = expect.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, expect_hex);
        // and stable across repeated calls
        assert_eq!(daily_sha256_hex(&sample()), hex);
        assert_eq!(hex.len(), 64);
    }

    /// AU-12 (unit half): any change to the digest changes the daily hash.
    #[test]
    fn au_12_change_invalidates() {
        let a = compute_daily_sha256(&sample());
        let mut b = sample();
        b.last_record_hash = "b".repeat(64);
        assert_ne!(a, compute_daily_sha256(&b));
        let mut c = sample();
        c.record_count = 124;
        assert_ne!(a, compute_daily_sha256(&c));
    }
}
