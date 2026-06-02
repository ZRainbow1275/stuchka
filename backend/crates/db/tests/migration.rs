//! Migration runner tests (backend/02 §2.13 / §2.14): idempotent re-run, single-direction.

use db::{current_version, migrate, run_migrations, Store};

#[tokio::test]
async fn migrations_apply_then_reapply_is_noop() {
    let pool = db::pool::open_in_memory().await.unwrap();

    let first = run_migrations(&pool).await.unwrap();
    assert_eq!(
        first.applied,
        vec![1, 2, 3, 4, 5, 6, 7, 8],
        "first run applies all eight"
    );
    assert!(first.skipped.is_empty());

    let second = run_migrations(&pool).await.unwrap();
    assert!(second.is_noop(), "second run must be a no-op (idempotent)");
    assert_eq!(second.skipped, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(second.applied.is_empty());

    assert_eq!(current_version(&pool).await.unwrap(), 8);
}

#[tokio::test]
async fn no_down_migrations_exist() {
    // backend/02 §2.14 migration_oneway: no *_down.sql, no DROP TABLE in any migration.
    assert!(migrate::has_no_down_migrations());
}

#[tokio::test]
async fn store_open_in_memory_is_ready() {
    let store = Store::open_in_memory().await.unwrap();
    // schema is present: count on a fresh store is zero, query succeeds.
    assert_eq!(store.case.count().await.unwrap(), 0);
}
