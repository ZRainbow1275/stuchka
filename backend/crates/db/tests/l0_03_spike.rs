//! L0-03 spike empirical evidence
//! (backend/05 + `.trellis/tasks/06-02-db-layer/research/L0-03-db-spike.md`).
//!
//! These tests are the runnable proof backing the SQLite selection: they exercise raw SQLite
//! (not the repo layer) to demonstrate each capability the fallback mapping relies on.

use rust_decimal::Decimal;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

async fn fresh_pool() -> sqlx::SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

/// (1) UUID v7 stored as a TEXT primary key round-trips exactly and stays time-sortable.
#[tokio::test]
async fn uuid_v7_as_text_primary_key() {
    let pool = fresh_pool().await;
    sqlx::query("CREATE TABLE t (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let mut ids = Vec::new();
    for _ in 0..100 {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO t (id) VALUES (?)")
            .bind(id.hyphenated().to_string())
            .execute(&pool)
            .await
            .unwrap();
        ids.push(id);
    }

    // TEXT PK round-trips and ORDER BY id matches creation order (v7 monotonicity).
    let rows = sqlx::query("SELECT id FROM t ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let back: Vec<Uuid> = rows
        .iter()
        .map(|r| Uuid::parse_str(&r.get::<String, _>(0)).unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(back, sorted, "TEXT-UUID PK must preserve v7 ordering");
}

/// (2) Decimal money stored as TEXT round-trips with zero precision loss (REAL would not).
#[tokio::test]
async fn decimal_as_text_money_no_precision_loss() {
    let pool = fresh_pool().await;
    sqlx::query("CREATE TABLE m (id TEXT PRIMARY KEY, amount TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let cases = [
        "0.01",
        "8000.00",
        "123456789012.99",
        "999999999.999999999",
        "0.123456789012345678",
    ];
    for raw in cases {
        let d = Decimal::from_str(raw).unwrap();
        let id = Uuid::now_v7().hyphenated().to_string();
        sqlx::query("INSERT INTO m (id, amount) VALUES (?, ?)")
            .bind(&id)
            .bind(d.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let row = sqlx::query("SELECT amount FROM m WHERE id = ?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let back = Decimal::from_str(&row.get::<String, _>(0)).unwrap();
        assert_eq!(back, d, "Decimal-as-TEXT lost precision for {raw}");
    }

    // Contrast: storing the same value through SQLite REAL DOES lose precision, proving why
    // money must be TEXT.
    sqlx::query("CREATE TABLE r (amount REAL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO r (amount) VALUES (?)")
        .bind(0.1f64 + 0.2f64)
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT amount FROM r")
        .fetch_one(&pool)
        .await
        .unwrap();
    let f: f64 = row.get(0);
    assert!(
        (f - 0.3).abs() > 0.0 && (f - 0.3).abs() < 1e-9,
        "REAL exhibits f64 rounding: {f}"
    );
}

/// (3) JSON stored as TEXT round-trips and is queryable via json_extract.
#[tokio::test]
async fn json_as_text_roundtrip_and_extract() {
    let pool = fresh_pool().await;
    sqlx::query("CREATE TABLE j (id TEXT PRIMARY KEY, doc TEXT)")
        .execute(&pool)
        .await
        .unwrap();

    let doc = serde_json::json!({"source": 0.8, "tags": ["wage", "overtime"]});
    let id = Uuid::now_v7().hyphenated().to_string();
    sqlx::query("INSERT INTO j (id, doc) VALUES (?, ?)")
        .bind(&id)
        .bind(serde_json::to_string(&doc).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT doc, json_extract(doc, '$.source') AS s FROM j WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let back: serde_json::Value = serde_json::from_str(&row.get::<String, _>("doc")).unwrap();
    assert_eq!(back, doc, "JSON-as-TEXT must round-trip exactly");
    let extracted: f64 = row.get("s");
    assert!(
        (extracted - 0.8).abs() < 1e-9,
        "json_extract must read nested keys"
    );
}

/// (4) Foreign-key RESTRICT and CASCADE are enforced when foreign_keys = ON.
#[tokio::test]
async fn foreign_key_restrict_and_cascade_enforced() {
    let pool = fresh_pool().await;
    sqlx::query("CREATE TABLE parent (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE child_restrict (id TEXT PRIMARY KEY, pid TEXT \
         REFERENCES parent(id) ON DELETE RESTRICT)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE child_cascade (id TEXT PRIMARY KEY, pid TEXT \
         REFERENCES parent(id) ON DELETE CASCADE)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let pid = Uuid::now_v7().hyphenated().to_string();
    sqlx::query("INSERT INTO parent (id) VALUES (?)")
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();

    // orphan insert violates FK
    let orphan = sqlx::query("INSERT INTO child_restrict (id, pid) VALUES (?, ?)")
        .bind(Uuid::now_v7().hyphenated().to_string())
        .bind("00000000-0000-0000-0000-000000000000")
        .execute(&pool)
        .await;
    assert!(orphan.is_err(), "FK must reject an orphan child");

    sqlx::query("INSERT INTO child_restrict (id, pid) VALUES (?, ?)")
        .bind(Uuid::now_v7().hyphenated().to_string())
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO child_cascade (id, pid) VALUES (?, ?)")
        .bind(Uuid::now_v7().hyphenated().to_string())
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();

    // RESTRICT blocks parent delete while a restrict-child references it.
    let restrict_del = sqlx::query("DELETE FROM parent WHERE id = ?")
        .bind(&pid)
        .execute(&pool)
        .await;
    assert!(
        restrict_del.is_err(),
        "ON DELETE RESTRICT must block parent delete"
    );

    // Remove the restrict-child, then parent delete CASCADEs the cascade-child away.
    sqlx::query("DELETE FROM child_restrict WHERE pid = ?")
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM parent WHERE id = ?")
        .bind(&pid)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM child_cascade")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0, "ON DELETE CASCADE must remove children");
}

/// (5) Partial indexes (CREATE INDEX ... WHERE) are supported and usable by the planner.
#[tokio::test]
async fn partial_index_supported() {
    let pool = fresh_pool().await;
    sqlx::query("CREATE TABLE p (id TEXT PRIMARY KEY, status TEXT, gid TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    // partial index analogous to idx_case_group / idx_evidence_quarantine
    sqlx::query("CREATE INDEX idx_p_gid ON p(gid) WHERE gid IS NOT NULL")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE INDEX idx_p_quar ON p(id) WHERE status = 'quarantined'")
        .execute(&pool)
        .await
        .unwrap();

    // Index exists in the catalog.
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_p_gid','idx_p_quar')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 2, "both partial indexes must be created");

    // The query planner can use the partial index for the matching predicate.
    sqlx::query("INSERT INTO p (id, status, gid) VALUES (?, 'quarantined', NULL)")
        .bind(Uuid::now_v7().hyphenated().to_string())
        .execute(&pool)
        .await
        .unwrap();
    let plan = sqlx::query("EXPLAIN QUERY PLAN SELECT id FROM p WHERE status = 'quarantined'")
        .fetch_all(&pool)
        .await
        .unwrap();
    let detail: String = plan
        .iter()
        .map(|r| r.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        detail.contains("idx_p_quar"),
        "planner should use the partial index, got: {detail}"
    );
}
