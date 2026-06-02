//! Time conventions — chrono only (D9 §0.6, data/01 §1.0.1).
//!
//! - Lifecycle / audit timestamps (`created_at` / `updated_at` / `frozen_at`) use
//!   [`Timestamp`] = `DateTime<Utc>`, stored UTC.
//! - `case_occurred_at` is day-precise with no timezone: [`PlainDate`] = `NaiveDate`.
//!
//! chrono is the workspace-wide time library (sqlx 0.8 supports it natively; jiff has
//! no native sqlx support).

use chrono::{DateTime, NaiveDate, Utc};

/// UTC instant for lifecycle and audit fields.
pub type Timestamp = DateTime<Utc>;

/// Day-precise civil date (e.g. `case_occurred_at`); no timezone.
pub type PlainDate = NaiveDate;

/// Current UTC instant for `created_at` / `updated_at` defaults.
#[inline]
pub fn now() -> Timestamp {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_returns_utc() {
        let t1 = now();
        let t2 = now();
        assert!(t2 >= t1, "now() must be non-decreasing");
    }

    #[test]
    fn plain_date_roundtrips_iso() {
        let d = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let s = d.format("%Y-%m-%d").to_string();
        assert_eq!(s, "2026-09-01");
        let parsed = NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap();
        assert_eq!(parsed, d);
    }
}
