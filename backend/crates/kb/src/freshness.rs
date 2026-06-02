//! KB freshness / Level-4 expiry gate (data/03 §3.5).
//!
//! The age of the active KB version is measured against `manifest.generated_at`:
//!
//! | age (days)        | level    | behaviour                                              |
//! |-------------------|----------|--------------------------------------------------------|
//! | 0–7               | `Fresh`  | normal use                                             |
//! | 8–29              | `Stale`  | soft start-up reminder                                 |
//! | >= 30             | `Expired`| Level-4 downgrade: M9 compensation calculation refused |
//!
//! M9 MUST call [`ensure_calculable`] before running the compensation engine; an `Expired` KB
//! returns [`KbError::Outdated`] so the caller aborts and writes the `KbExpiredBlock` audit
//! entry (KB-02).

use chrono::{DateTime, Duration, Utc};

use crate::error::KbError;

/// KB freshness level relative to `manifest.generated_at` (data/03 §3.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessLevel {
    /// 0–7 days: normal use.
    Fresh,
    /// 8–29 days: soft reminder.
    Stale,
    /// >= 30 days: Level-4 downgrade — compensation calculation refused.
    Expired,
}

impl FreshnessLevel {
    /// True when the KB is too old to drive a compensation calculation (data/03 §3.5).
    pub fn blocks_calculation(self) -> bool {
        matches!(self, FreshnessLevel::Expired)
    }
}

/// Classify the KB freshness from its generation timestamp and the current instant
/// (data/03 §3.5.1). Boundaries: `< 8d → Fresh`, `< 30d → Stale`, else `Expired` (KB-01).
pub fn freshness(generated_at: DateTime<Utc>, now: DateTime<Utc>) -> FreshnessLevel {
    let age = now - generated_at;
    if age < Duration::days(8) {
        FreshnessLevel::Fresh
    } else if age < Duration::days(30) {
        FreshnessLevel::Stale
    } else {
        FreshnessLevel::Expired
    }
}

/// Whole-day age of the KB (clamped at 0; used for the `current_age_days` config field and the
/// deploy/04 §4.5.1 UI gradient). Negative ages (clock skew / future timestamp) clamp to 0.
pub fn age_days(generated_at: DateTime<Utc>, now: DateTime<Utc>) -> u32 {
    let days = (now - generated_at).num_days();
    days.max(0) as u32
}

/// M9 calculation gate (data/03 §3.5 / KB-02). Returns `Ok(())` when the KB is `Fresh` or
/// `Stale`, and [`KbError::Outdated`] when it is `Expired` so the compensation engine aborts.
/// The caller is responsible for writing the `KbExpiredBlock` audit entry on `Err`.
pub fn ensure_calculable(generated_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<(), KbError> {
    if freshness(generated_at, now).blocks_calculation() {
        Err(KbError::Outdated {
            age_days: (now - generated_at).num_days(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap()
    }

    /// KB-01: age=7d → Fresh; 8d → Stale; 29d → Stale; 30d → Expired (boundary four points).
    #[test]
    fn kb01_freshness_boundary_four_points() {
        let gen = at(2026, 5, 1);
        assert_eq!(
            freshness(gen, gen + Duration::days(7)),
            FreshnessLevel::Fresh
        );
        assert_eq!(
            freshness(gen, gen + Duration::days(8)),
            FreshnessLevel::Stale
        );
        assert_eq!(
            freshness(gen, gen + Duration::days(29)),
            FreshnessLevel::Stale
        );
        assert_eq!(
            freshness(gen, gen + Duration::days(30)),
            FreshnessLevel::Expired
        );
    }

    #[test]
    fn fresh_at_zero_age() {
        let gen = at(2026, 5, 1);
        assert_eq!(freshness(gen, gen), FreshnessLevel::Fresh);
    }

    /// The just-under-8-day boundary (7d 23h) is still Fresh; the just-under-30-day boundary
    /// (29d 23h) is still Stale — the comparison is strict `<` on whole-day Durations.
    #[test]
    fn sub_day_boundaries() {
        let gen = at(2026, 5, 1);
        let almost_8 = gen + Duration::days(7) + Duration::hours(23);
        assert_eq!(freshness(gen, almost_8), FreshnessLevel::Fresh);
        let almost_30 = gen + Duration::days(29) + Duration::hours(23);
        assert_eq!(freshness(gen, almost_30), FreshnessLevel::Stale);
    }

    /// KB-02 gate: Expired KB makes `ensure_calculable` refuse (the M9 abort hook).
    #[test]
    fn kb02_ensure_calculable_blocks_when_expired() {
        let gen = at(2026, 5, 1);
        assert!(ensure_calculable(gen, gen + Duration::days(7)).is_ok());
        assert!(ensure_calculable(gen, gen + Duration::days(29)).is_ok());

        let err = ensure_calculable(gen, gen + Duration::days(45)).unwrap_err();
        assert_eq!(err.code(), "E_KB_OUTDATED");
        assert!(matches!(err, KbError::Outdated { age_days } if age_days == 45));
    }

    #[test]
    fn age_days_clamps_negative() {
        let gen = at(2026, 5, 1);
        assert_eq!(age_days(gen, gen + Duration::days(12)), 12);
        // future generated_at (clock skew) clamps to 0, never panics.
        assert_eq!(age_days(gen, gen - Duration::days(3)), 0);
    }
}
