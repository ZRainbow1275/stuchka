//! §4.7 — the `AuditLog` top-level API over the independent `audit.sqlite`.
//!
//! `append(who, why, what) -> seq` (I2: 3 positional args; `category` is derived from `why`,
//! `when` is `Utc::now()` taken internally so it is monotonic and not caller-supplied). Writes are
//! transactional and the chain is read-modify-extended under a per-pool mutex so concurrent
//! appends cannot interleave a stale `prev_hash`.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::cipher::AuditCipher;
use crate::daily_hash::{self, DailyDigest};
use crate::error::{AnchorError, AuditError, DailyHashError};
use crate::hash_chain::{compute_record_hash, GENESIS_PREV_HASH};
use crate::ots::OtsClient;
use crate::reason::AuditReason;
use crate::schema::validate_what;
use crate::subject::Subject;
use crate::trust::TrustState;

/// A raw audit row as stored (ciphertext, not decrypted). Used by chain verification, tests, and
/// the daily-hash path.
#[derive(Debug, Clone)]
pub struct RawRecord {
    pub seq: i64,
    pub prev_hash: String,
    pub record_hash: String,
    pub who_kind: String,
    pub who_payload: Vec<u8>,
    pub who_nonce: Vec<u8>,
    pub when_ts: i64,
    pub why: String,
    pub what_payload: Vec<u8>,
    pub nonce: Vec<u8>,
    pub schema_version: i64,
}

/// A decrypted audit record (DTO-facing projection; backend/01 §1.8 fields).
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub seq: i64,
    pub case_id: Option<Uuid>,
    pub who: Subject,
    pub when: DateTime<Utc>,
    pub why: AuditReason,
    pub what: serde_json::Value,
    pub prev_hash: String,
    pub record_hash: String,
}

/// A row of the `audit_anchor` table.
#[derive(Debug, Clone)]
pub struct AnchorRow {
    pub date: NaiveDate,
    pub last_record_hash: String,
    pub record_count: u64,
    pub daily_sha256: String,
    pub ots_receipt: Option<Vec<u8>>,
    pub ots_anchored_at: Option<String>,
    pub github_commit_sha: Option<String>,
    pub github_anchored_at: Option<String>,
}

/// Outcome of a full-chain verification (§4.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainVerification {
    pub ok: bool,
    pub checked: u64,
    /// First seq whose stored `record_hash` did not match the recomputed value; `None` if intact.
    pub broken_at_seq: Option<i64>,
}

/// The audit log over its own `SqlitePool` (data/04 §4.7). Holds the AEAD cipher keyed by the
/// HKDF-derived audit key; never touches the main store (D3).
#[derive(Clone)]
pub struct AuditLog {
    pool: SqlitePool,
    cipher: Arc<AuditCipher>,
    /// Serialises the read-prev-hash / insert critical section so concurrent appends chain cleanly.
    append_lock: Arc<Mutex<()>>,
}

impl AuditLog {
    /// Construct over an already-migrated audit pool and a 32-byte audit key.
    ///
    /// The audit key is `crypto::hkdf_subkey(DEK, crypto::AUDIT_INFO)` — derive it once at unlock
    /// and pass the bytes here (the DEK itself never enters this crate).
    #[must_use]
    pub fn new(pool: SqlitePool, audit_key: &[u8; 32]) -> Self {
        Self {
            pool,
            cipher: Arc::new(AuditCipher::new(audit_key)),
            append_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Borrow the underlying pool (anchor path / tests).
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Append a record and return its `seq` (§4.7).
    ///
    /// Steps: validate `what` against the `why` schema; encrypt who/what with independent random
    /// nonces (W5); read `prev_hash`; compute `record_hash`; INSERT in a transaction.
    pub async fn append(
        &self,
        who: Subject,
        why: AuditReason,
        what: serde_json::Value,
    ) -> Result<i64, AuditError> {
        validate_what(why, &what)?;

        let who_kind = who.who_kind().to_string();
        let who_json = serde_json::to_vec(&who)?;
        let what_json = serde_json::to_vec(&what)?;
        let (who_ct, who_nonce) = self.cipher.encrypt(&who_json)?;
        let (what_ct, what_nonce) = self.cipher.encrypt(&what_json)?;
        let when_ts = Utc::now().timestamp_millis();
        let why_text = why.as_str();

        // Serialise the prev-hash read + insert so concurrent appends do not race the tail.
        let _guard = self.append_lock.lock().await;

        let mut tx = self.pool.begin().await?;
        let prev_hash: String =
            sqlx::query_scalar("SELECT record_hash FROM audit_log ORDER BY seq DESC LIMIT 1")
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());

        let record_hash = compute_record_hash(
            &prev_hash,
            &who_kind,
            &who_ct,
            &who_nonce,
            when_ts,
            why_text,
            &what_ct,
            &what_nonce,
        );

        let seq: i64 = sqlx::query(
            "INSERT INTO audit_log \
             (prev_hash, record_hash, who_kind, who_payload, who_nonce, when_ts, why, \
              what_payload, nonce, schema_version) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1) RETURNING seq",
        )
        .bind(&prev_hash)
        .bind(&record_hash)
        .bind(&who_kind)
        .bind(&who_ct)
        .bind(&who_nonce)
        .bind(when_ts)
        .bind(why_text)
        .bind(&what_ct)
        .bind(&what_nonce)
        .fetch_one(&mut *tx)
        .await?
        .get(0);

        tx.commit().await?;
        Ok(seq)
    }

    /// Fetch a raw (still-encrypted) record by seq.
    pub async fn fetch_raw(&self, seq: i64) -> Result<RawRecord, AuditError> {
        let row = sqlx::query(
            "SELECT seq, prev_hash, record_hash, who_kind, who_payload, who_nonce, when_ts, why, \
             what_payload, nonce, schema_version FROM audit_log WHERE seq = ?",
        )
        .bind(seq)
        .fetch_one(&self.pool)
        .await?;
        Ok(row_to_raw(&row))
    }

    /// Decrypt a raw record into the DTO-facing [`AuditRecord`].
    pub fn decrypt_record(&self, raw: &RawRecord) -> Result<AuditRecord, AuditError> {
        let who_json = self.cipher.decrypt(&raw.who_payload, &raw.who_nonce)?;
        let what_json = self.cipher.decrypt(&raw.what_payload, &raw.nonce)?;
        let who: Subject = serde_json::from_slice(&who_json)?;
        let what: serde_json::Value = serde_json::from_slice(&what_json)?;
        let why: AuditReason = serde_json::from_value(serde_json::Value::String(raw.why.clone()))?;
        let case_id = match what.get("case_id") {
            Some(serde_json::Value::String(s)) => Uuid::parse_str(s).ok(),
            _ => None,
        };
        Ok(AuditRecord {
            seq: raw.seq,
            case_id,
            who,
            when: DateTime::<Utc>::from_timestamp_millis(raw.when_ts).unwrap_or_else(Utc::now),
            why,
            what,
            prev_hash: raw.prev_hash.clone(),
            record_hash: raw.record_hash.clone(),
        })
    }

    /// Query decrypted records whose `what.case_id` equals `case_id`, ordered by seq (§4.7).
    pub async fn query_by_case(&self, case_id: Uuid) -> Result<Vec<AuditRecord>, AuditError> {
        let rows = sqlx::query(
            "SELECT seq, prev_hash, record_hash, who_kind, who_payload, who_nonce, when_ts, why, \
             what_payload, nonce, schema_version FROM audit_log ORDER BY seq ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::new();
        for row in &rows {
            let raw = row_to_raw(row);
            let rec = self.decrypt_record(&raw)?;
            if rec.case_id == Some(case_id) {
                out.push(rec);
            }
        }
        Ok(out)
    }

    /// Verify the whole chain from seq ascending, recomputing each `record_hash` and checking the
    /// prev-link (§4.7). Returns the first broken seq if any.
    pub async fn verify_chain(&self) -> Result<ChainVerification, AuditError> {
        let rows = sqlx::query(
            "SELECT seq, prev_hash, record_hash, who_kind, who_payload, who_nonce, when_ts, why, \
             what_payload, nonce, schema_version FROM audit_log ORDER BY seq ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut checked = 0u64;
        let mut expected_prev = GENESIS_PREV_HASH.to_string();
        for row in &rows {
            let raw = row_to_raw(row);
            checked += 1;
            // prev-link must match the running expectation
            if raw.prev_hash != expected_prev {
                return Ok(ChainVerification {
                    ok: false,
                    checked,
                    broken_at_seq: Some(raw.seq),
                });
            }
            // recompute record_hash over the stored fields
            let recomputed = compute_record_hash(
                &raw.prev_hash,
                &raw.who_kind,
                &raw.who_payload,
                &raw.who_nonce,
                raw.when_ts,
                &raw.why,
                &raw.what_payload,
                &raw.nonce,
            );
            if recomputed != raw.record_hash {
                return Ok(ChainVerification {
                    ok: false,
                    checked,
                    broken_at_seq: Some(raw.seq),
                });
            }
            expected_prev = raw.record_hash.clone();
        }

        Ok(ChainVerification {
            ok: true,
            checked,
            broken_at_seq: None,
        })
    }

    /// Startup hook (data/04 §4.9, C-C-15): run `verify_chain` and return
    /// `Err(AuditError::ChainBroken)` if any record fails — the caller then refuses new audit
    /// writes and guides the user to legal aid.
    pub async fn verify_on_startup(&self) -> Result<u64, AuditError> {
        let v = self.verify_chain().await?;
        if v.ok {
            Ok(v.checked)
        } else {
            Err(AuditError::ChainBroken(v.broken_at_seq))
        }
    }

    /// Compute the daily digest (instance wrapper over [`daily_hash::compute_daily_hash`]).
    pub async fn compute_daily_hash(&self, date: NaiveDate) -> Result<DailyDigest, DailyHashError> {
        daily_hash::compute_daily_hash(&self.pool, date).await
    }

    /// Anchor a day to OTS via `client` and persist the receipt into `audit_anchor` (§4.6.2).
    ///
    /// R1a done criterion: after success `audit_anchor.ots_receipt IS NOT NULL AND
    /// ots_anchored_at IS NOT NULL` (AU-09).
    pub async fn anchor_to_ots<C: OtsClient>(
        &self,
        client: &C,
        digest: &DailyDigest,
    ) -> Result<AnchorRow, AuditError> {
        let hash_bytes = daily_hash::compute_daily_sha256(digest);
        let receipt = client
            .stamp(&hash_bytes)
            .await
            .map_err(|e| AnchorError::Upstream(e.to_string()))?;
        // Guard against a mis-bound receipt before persisting (§4.6.2 step 3).
        C::verify_binding(&receipt, &hash_bytes).map_err(|_| AnchorError::BadBinding)?;

        let daily_hex: String = hash_bytes.iter().map(|b| format!("{b:02x}")).collect();
        let anchored_at = Utc::now().to_rfc3339();
        let date_str = digest.date.format("%Y-%m-%d").to_string();

        sqlx::query(
            "INSERT INTO audit_anchor \
             (date, last_record_hash, record_count, daily_sha256, ots_receipt, ots_anchored_at, \
              schema_version) \
             VALUES (?, ?, ?, ?, ?, ?, 1) \
             ON CONFLICT(date) DO UPDATE SET \
              last_record_hash = excluded.last_record_hash, \
              record_count = excluded.record_count, \
              daily_sha256 = excluded.daily_sha256, \
              ots_receipt = excluded.ots_receipt, \
              ots_anchored_at = excluded.ots_anchored_at",
        )
        .bind(&date_str)
        .bind(&digest.last_record_hash)
        .bind(digest.record_count as i64)
        .bind(&daily_hex)
        .bind(&receipt)
        .bind(&anchored_at)
        .execute(&self.pool)
        .await
        .map_err(AnchorError::Db)?;

        self.fetch_anchor(digest.date)
            .await?
            .ok_or_else(|| AuditError::Anchor(AnchorError::Upstream("anchor row vanished".into())))
    }

    /// Fetch the anchor row for a date (`None` if not yet anchored).
    pub async fn fetch_anchor(&self, date: NaiveDate) -> Result<Option<AnchorRow>, AuditError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let row = sqlx::query(
            "SELECT date, last_record_hash, record_count, daily_sha256, ots_receipt, \
             ots_anchored_at, github_commit_sha, github_anchored_at FROM audit_anchor WHERE date = ?",
        )
        .bind(&date_str)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            let date_text: String = r.get("date");
            AnchorRow {
                date: NaiveDate::parse_from_str(&date_text, "%Y-%m-%d").unwrap_or(date),
                last_record_hash: r.get("last_record_hash"),
                record_count: r.get::<i64, _>("record_count") as u64,
                daily_sha256: r.get("daily_sha256"),
                ots_receipt: r.get("ots_receipt"),
                ots_anchored_at: r.get("ots_anchored_at"),
                github_commit_sha: r.get("github_commit_sha"),
                github_anchored_at: r.get("github_anchored_at"),
            }
        }))
    }

    /// Aggregate the three-layer trust state for a date (§4.8). Layer 1 = chain intact; Layer 2 =
    /// OTS receipt present and binds to the day's `daily_sha256`; Layer 3 = a GitHub commit anchor
    /// is recorded.
    pub async fn trust_state_for<C: OtsClient>(
        &self,
        date: NaiveDate,
    ) -> Result<TrustState, AuditError> {
        let chain_ok = self.verify_chain().await?.ok;
        let anchor = self.fetch_anchor(date).await?;
        let (ots_verified, github_anchored) = match anchor {
            Some(a) => {
                let ots_verified = match &a.ots_receipt {
                    Some(receipt) => {
                        // bind against the stored daily_sha256
                        match hex32(&a.daily_sha256) {
                            Some(h) => C::verify_binding(receipt, &h).is_ok(),
                            None => false,
                        }
                    }
                    None => false,
                };
                (ots_verified, a.github_commit_sha.is_some())
            }
            None => (false, false),
        };
        Ok(TrustState::new(chain_ok, ots_verified, github_anchored))
    }
}

/// Decode a 64-hex string into a 32-byte array.
fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn row_to_raw(row: &sqlx::sqlite::SqliteRow) -> RawRecord {
    RawRecord {
        seq: row.get("seq"),
        prev_hash: row.get("prev_hash"),
        record_hash: row.get("record_hash"),
        who_kind: row.get("who_kind"),
        who_payload: row.get("who_payload"),
        who_nonce: row.get("who_nonce"),
        when_ts: row.get("when_ts"),
        why: row.get("why"),
        what_payload: row.get("what_payload"),
        nonce: row.get("nonce"),
        schema_version: row.get("schema_version"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex32_round_trip() {
        let s = "a".repeat(64);
        let bytes = hex32(&s).unwrap();
        assert_eq!(bytes, [0xaau8; 32]);
        assert!(hex32("short").is_none());
        assert!(hex32(&"z".repeat(64)).is_none());
    }
}
