//! UserSettingRepo — auxiliary local-settings singleton get/upsert (backend/04 §4.3/§4.4/§4.7).
//!
//! `UserSetting` is NOT a D9 first-class object (cross-crate-reconciliation B-6); it is the single
//! local row backing encryption/recovery state. The table holds at most one row (enforced by the
//! `idx_user_setting_singleton` partial unique index in migration 0007), so the repo exposes
//! `get` (the lone row, if any) and `upsert` (insert-or-replace by id). All values use the SQLite
//! fallback codecs: UUID->TEXT, ENUM->TEXT, TIMESTAMPTZ->TEXT RFC3339.

use data_model::user_setting::{FsEncryptionStatus, RecoveryMethod};
use data_model::UserSetting;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::{DbError, DbResult};

/// Repository over the `user_setting` singleton table.
#[derive(Clone)]
pub struct UserSettingRepo {
    pool: SqlitePool,
}

impl UserSettingRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Fetch the singleton settings row, if it has been written yet.
    pub async fn get(&self) -> DbResult<Option<UserSetting>> {
        let row = sqlx::query("SELECT * FROM user_setting LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_user_setting(&r)?)),
            None => Ok(None),
        }
    }

    /// Insert or replace the singleton settings row (upsert keyed on `id`).
    ///
    /// `created_at` is preserved on conflict; `updated_at` and all mutable fields are overwritten.
    pub async fn upsert(&self, s: &UserSetting) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO user_setting (\
                id, fs_encryption_status, recovery_method, bip39_check_hash,\
                master_pwd_argon2, created_at, updated_at\
            ) VALUES (?,?,?,?,?,?,?)\
            ON CONFLICT(id) DO UPDATE SET \
                fs_encryption_status = excluded.fs_encryption_status,\
                recovery_method      = excluded.recovery_method,\
                bip39_check_hash     = excluded.bip39_check_hash,\
                master_pwd_argon2    = excluded.master_pwd_argon2,\
                updated_at           = excluded.updated_at",
        )
        .bind(uuid_to_text(&s.id))
        .bind(val_str(s.fs_encryption_status))
        .bind(opt_val_str(s.recovery_method))
        .bind(&s.bip39_check_hash)
        .bind(&s.master_pwd_argon2)
        .bind(ts_to_text(&s.created_at))
        .bind(ts_to_text(&s.updated_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn val_str<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn opt_val_str<T: serde::Serialize>(v: Option<T>) -> Option<String> {
    v.map(val_str)
}

fn parse_enum<T: serde::de::DeserializeOwned>(s: &str) -> DbResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(DbError::Json)
}

fn row_to_user_setting(r: &sqlx::sqlite::SqliteRow) -> DbResult<UserSetting> {
    let fs_status: String = r.get("fs_encryption_status");
    let recovery: Option<String> = r.get("recovery_method");

    Ok(UserSetting {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        fs_encryption_status: parse_enum::<FsEncryptionStatus>(&fs_status)?,
        recovery_method: match recovery {
            Some(s) => Some(parse_enum::<RecoveryMethod>(&s)?),
            None => None,
        },
        bip39_check_hash: r.get("bip39_check_hash"),
        master_pwd_argon2: r.get("master_pwd_argon2"),
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
        updated_at: text_to_ts(&r.get::<String, _>("updated_at"))?,
    })
}
