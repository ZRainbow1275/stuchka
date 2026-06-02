//! UserSetting — auxiliary local-settings object referenced throughout `backend/04` (encryption).
//!
//! `data/01` (D9) does not list this object; `backend/04` repeatedly references its fields
//! (`fs_encryption_status` §4.3.2, `recovery_method` §4.7.4, `bip39_check_hash` §4.7.2,
//! `master_pwd_argon2` §4.4.1). It is therefore an auxiliary object (NOT one of the D9 six
//! first-class objects), added here so `crates/crypto` (blocked by I-8) and `crates/db` can persist
//! it. Like every other object its `id` is a v7 UUID (even though it is effectively a singleton).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::Timestamp;
use crate::traits::Identified;

/// File-system encryption detection result (`backend/04` §4.3.1 / §4.3.2).
///
/// `Enabled` / `Disabled` / `Unknown` come from the platform probe; `DisabledWithAck` records that
/// the user acknowledged the risk and chose to continue on an unencrypted volume (§4.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "fs_encryption_status", rename_all = "snake_case")
)]
pub enum FsEncryptionStatus {
    Enabled,
    Disabled,
    /// User saw the warning and chose "我已知风险并继续" (`backend/04` §4.3.2).
    DisabledWithAck,
    #[default]
    Unknown,
}

/// Master-password backup method (`backend/04` §4.7). Exactly one of three (C-C-14); must be set
/// via the boot wizard before normal operation (§4.7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "recovery_method", rename_all = "snake_case")
)]
pub enum RecoveryMethod {
    /// Path A — OS keychain (`backend/04` §4.7.1).
    Keychain,
    /// Path B — BIP39 mnemonic (`backend/04` §4.7.2).
    Bip39,
    /// Path C — USB / offline recovery key file (`backend/04` §4.7.3).
    UsbKey,
}

/// Local user settings singleton (`backend/04` §4.3 / §4.4 / §4.7). Auxiliary object — not a D9
/// first-class object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct UserSetting {
    /// v7 UUID (single-row table still uses a v7 id, D9).
    pub id: Uuid,
    /// FS encryption detection result, refreshed each boot (`backend/04` §4.3.2).
    pub fs_encryption_status: FsEncryptionStatus,
    /// Chosen master-password backup method; `None` until the boot wizard completes (§4.7.4).
    pub recovery_method: Option<RecoveryMethod>,
    /// `sha256(mnemonic.to_seed(""))` written after BIP39 re-entry verification (§4.7.2).
    pub bip39_check_hash: Option<String>,
    /// Argon2id hash of the master password (hash only — never the plaintext; §4.4.1).
    pub master_pwd_argon2: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Identified for UserSetting {
    fn id(&self) -> Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;

    #[test]
    fn fs_encryption_status_default_is_unknown() {
        assert_eq!(FsEncryptionStatus::default(), FsEncryptionStatus::Unknown);
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&FsEncryptionStatus::DisabledWithAck).unwrap(),
            "\"disabled_with_ack\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryMethod::UsbKey).unwrap(),
            "\"usb_key\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryMethod::Bip39).unwrap(),
            "\"bip39\""
        );
    }

    #[test]
    fn user_setting_serde_roundtrip_camel_case() {
        let us = UserSetting {
            id: new_id(),
            fs_encryption_status: FsEncryptionStatus::Enabled,
            recovery_method: Some(RecoveryMethod::Keychain),
            bip39_check_hash: None,
            master_pwd_argon2: Some("$argon2id$v=19$m=65536,t=3,p=4$...".to_string()),
            created_at: now(),
            updated_at: now(),
        };
        let j = serde_json::to_string(&us).unwrap();
        assert!(j.contains("\"fsEncryptionStatus\":\"enabled\""), "got {j}");
        assert!(j.contains("\"recoveryMethod\":\"keychain\""), "got {j}");
        let back: UserSetting = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id(), us.id);
        assert_eq!(back.recovery_method, us.recovery_method);
        assert_eq!(back.master_pwd_argon2, us.master_pwd_argon2);
    }
}
