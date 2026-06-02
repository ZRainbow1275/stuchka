//! Incremental-reconciliation tests (06-02 db pass):
//! - user_setting singleton repo get/upsert round-trip + singleton guard;
//! - tightened claim amount-shape CHECK (rejects malformed money TEXT at the DDL layer);
//! - fact `disputed -> deprecated` transition is allowed through the FactRepo guard
//!   (backend/02 §2.3 new-evidence workflow).

mod common;

use chrono::Utc;
use common::*;
use data_model::enums::FactStatus;
use data_model::user_setting::{FsEncryptionStatus, RecoveryMethod};
use data_model::UserSetting;
use db::Store;
use sqlx::Row;
use uuid::Uuid;

// --- user_setting repo ------------------------------------------------------

fn sample_user_setting() -> UserSetting {
    UserSetting {
        id: Uuid::now_v7(),
        fs_encryption_status: FsEncryptionStatus::Enabled,
        recovery_method: Some(RecoveryMethod::Bip39),
        bip39_check_hash: Some("e".repeat(64)),
        master_pwd_argon2: Some("$argon2id$v=19$m=65536,t=3,p=4$abc$def".to_string()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn user_setting_get_is_none_before_first_write() {
    let store = Store::open_in_memory().await.unwrap();
    assert!(store.user_setting.get().await.unwrap().is_none());
}

#[tokio::test]
async fn user_setting_upsert_then_get_roundtrip() {
    let store = Store::open_in_memory().await.unwrap();
    let s = sample_user_setting();
    store.user_setting.upsert(&s).await.unwrap();

    let got = store
        .user_setting
        .get()
        .await
        .unwrap()
        .expect("settings present after upsert");
    assert_eq!(got.id, s.id);
    assert_eq!(got.fs_encryption_status, FsEncryptionStatus::Enabled);
    assert_eq!(got.recovery_method, Some(RecoveryMethod::Bip39));
    assert_eq!(got.bip39_check_hash, s.bip39_check_hash);
    assert_eq!(got.master_pwd_argon2, s.master_pwd_argon2);
}

#[tokio::test]
async fn user_setting_upsert_updates_existing_row_in_place() {
    let store = Store::open_in_memory().await.unwrap();
    let mut s = sample_user_setting();
    store.user_setting.upsert(&s).await.unwrap();

    // Same id, mutate fields: upsert must overwrite, not insert a second row.
    s.fs_encryption_status = FsEncryptionStatus::DisabledWithAck;
    s.recovery_method = Some(RecoveryMethod::Keychain);
    s.master_pwd_argon2 = None;
    s.updated_at = Utc::now();
    store.user_setting.upsert(&s).await.unwrap();

    let got = store.user_setting.get().await.unwrap().unwrap();
    assert_eq!(
        got.fs_encryption_status,
        FsEncryptionStatus::DisabledWithAck
    );
    assert_eq!(got.recovery_method, Some(RecoveryMethod::Keychain));
    assert_eq!(got.master_pwd_argon2, None);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_setting")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1, "upsert on same id must not create a second row");
}

#[tokio::test]
async fn user_setting_optional_fields_null_roundtrip() {
    let store = Store::open_in_memory().await.unwrap();
    let s = UserSetting {
        id: Uuid::now_v7(),
        fs_encryption_status: FsEncryptionStatus::Unknown,
        recovery_method: None,
        bip39_check_hash: None,
        master_pwd_argon2: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    store.user_setting.upsert(&s).await.unwrap();
    let got = store.user_setting.get().await.unwrap().unwrap();
    assert_eq!(got.recovery_method, None);
    assert_eq!(got.bip39_check_hash, None);
    assert_eq!(got.master_pwd_argon2, None);
    assert_eq!(got.fs_encryption_status, FsEncryptionStatus::Unknown);
}

#[tokio::test]
async fn user_setting_singleton_guard_rejects_second_distinct_row() {
    // The partial unique index enforces "at most one row": inserting a second row with a
    // different id must be rejected by SQLite.
    let store = Store::open_in_memory().await.unwrap();
    let first = sample_user_setting();
    store.user_setting.upsert(&first).await.unwrap();

    let now = Utc::now().to_rfc3339();
    let res = sqlx::query(
        "INSERT INTO user_setting (id, fs_encryption_status, recovery_method, \
         bip39_check_hash, master_pwd_argon2, created_at, updated_at) \
         VALUES (?, 'unknown', NULL, NULL, NULL, ?, ?)",
    )
    .bind(Uuid::now_v7().hyphenated().to_string())
    .bind(&now)
    .bind(&now)
    .execute(store.pool())
    .await;
    assert!(
        res.is_err(),
        "singleton unique index must reject a second user_setting row"
    );
}

// --- tightened claim amount CHECK -------------------------------------------

/// Valid fixed-point money TEXT must pass the DDL CHECK (raw insert, bypassing the repo).
#[tokio::test]
async fn claim_amount_check_accepts_valid_money() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    for amount in ["0", "0.00", "8000.00", "123456789012.99", "100"] {
        let res = insert_claim_with_raw_amount(&store, &c.id, amount).await;
        assert!(res.is_ok(), "valid amount {amount:?} must pass the CHECK");
    }
}

/// Malformed money TEXT must be rejected by the tightened CHECK
/// (NOT GLOB '*[^0-9.]*' AND GLOB '[0-9]*' AND NOT GLOB '*.*.*').
#[tokio::test]
async fn claim_amount_check_rejects_malformed_money() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    for bad in [
        "-1.00",    // sign char (non-digit/dot)
        "1.2.3",    // two decimal points
        ".5",       // leading dot (does not start with a digit)
        "1,000.00", // thousands separator
        "8000abc",  // trailing letters (old weak GLOB would have allowed this)
        "1e3",      // scientific notation
        "",         // empty
        " 1",       // leading space
    ] {
        let res = insert_claim_with_raw_amount(&store, &c.id, bad).await;
        assert!(
            res.is_err(),
            "malformed amount {bad:?} must be rejected by the tightened CHECK"
        );
    }
}

async fn insert_claim_with_raw_amount(
    store: &Store,
    case_id: &Uuid,
    amount: &str,
) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO claim (id, case_id, claim_type, amount_pre_tax, amount_post_tax, \
         calculation_breakdown, status, law_refs, fact_refs, withdraw_inv10_confirmed, \
         created_at, updated_at) \
         VALUES (?, ?, 'economic_compensation', ?, NULL, '{}', 'draft', '[]', '[]', 0, ?, ?)",
    )
    .bind(Uuid::now_v7().hyphenated().to_string())
    .bind(case_id.hyphenated().to_string())
    .bind(amount)
    .bind(&now)
    .bind(&now)
    .execute(store.pool())
    .await
}

// --- fact disputed -> deprecated (repo-level guard) -------------------------

/// backend/02 §2.3 / cross-crate-reconciliation: a disputed fact can be retired to deprecated by
/// new evidence. The FactRepo::update guard must pass this transition through to persistence.
#[tokio::test]
async fn fact_repo_allows_disputed_to_deprecated() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let mut f = sample_fact(c.id);
    f.status = FactStatus::Pending;
    store.fact.insert(&f).await.unwrap();

    // pending -> disputed (legal), then disputed -> deprecated (the edge under test).
    f.status = FactStatus::Disputed;
    store.fact.update(&f).await.unwrap();

    f.status = FactStatus::Deprecated;
    store.fact.update(&f).await.unwrap();

    let got = store.fact.get(&f.id).await.unwrap().unwrap();
    assert_eq!(got.status, FactStatus::Deprecated);

    // verify the raw column too (guard did not silently no-op).
    let raw: String = sqlx::query("SELECT status FROM fact WHERE id = ?")
        .bind(f.id.hyphenated().to_string())
        .fetch_one(store.pool())
        .await
        .unwrap()
        .get("status");
    assert_eq!(raw, "deprecated");
}
