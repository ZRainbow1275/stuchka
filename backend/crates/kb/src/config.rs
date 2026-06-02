//! KB sync configuration (data/03 §3.3.1).
//!
//! Four check-frequency strategies (all R1a) and the sync-state snapshot persisted in the
//! `kb_sync_state` singleton table. The `sqlx::Type` derive is gated behind the optional `sqlx`
//! feature via `cfg_attr` — mirroring the data-model enum convention — so the db layer can
//! persist [`KbCheckFrequency`] without forcing sqlx onto pure consumers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How often the client checks the KB repository for a new version (data/03 §3.3, deploy/04 §4.5).
/// All four options are R1a; `Daily` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "kb_check_frequency", rename_all = "snake_case")
)]
pub enum KbCheckFrequency {
    /// Check on first launch each day (default).
    #[default]
    Daily,
    /// Check on first launch each Monday.
    Weekly,
    /// Check on first launch of the 1st of each month.
    Monthly,
    /// Check only when the user presses "check for updates".
    Manual,
}

/// Snapshot of the client's KB sync state (data/03 §3.3.1). Mirrors the `kb_sync_state`
/// singleton row plus the derived `current_age_days` (computed from `manifest.generated_at`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbSyncConfig {
    pub frequency: KbCheckFrequency,
    pub last_check_at: Option<DateTime<Utc>>,
    pub current_version_label: String,
    pub current_global_hash: String,
    /// Days since `manifest.generated_at` (drives the deploy/04 §4.5.1 UI gradient).
    pub current_age_days: u32,
    /// Whether pulled new versions are applied automatically (only to non-frozen cases, INV-04).
    pub auto_apply: bool,
    pub mirror_url_override: Option<String>,
}

impl Default for KbSyncConfig {
    fn default() -> Self {
        Self {
            frequency: KbCheckFrequency::default(),
            last_check_at: None,
            current_version_label: String::new(),
            current_global_hash: String::new(),
            current_age_days: 0,
            auto_apply: true,
            mirror_url_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_default_is_daily() {
        assert_eq!(KbCheckFrequency::default(), KbCheckFrequency::Daily);
    }

    #[test]
    fn frequency_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&KbCheckFrequency::Daily).unwrap(),
            "\"daily\""
        );
        assert_eq!(
            serde_json::to_string(&KbCheckFrequency::Manual).unwrap(),
            "\"manual\""
        );
        for f in [
            KbCheckFrequency::Daily,
            KbCheckFrequency::Weekly,
            KbCheckFrequency::Monthly,
            KbCheckFrequency::Manual,
        ] {
            let s = serde_json::to_string(&f).unwrap();
            let back: KbCheckFrequency = serde_json::from_str(&s).unwrap();
            assert_eq!(f, back);
        }
    }

    #[test]
    fn default_config_auto_applies_daily() {
        let c = KbSyncConfig::default();
        assert_eq!(c.frequency, KbCheckFrequency::Daily);
        assert!(c.auto_apply);
        assert!(c.last_check_at.is_none());
    }
}
