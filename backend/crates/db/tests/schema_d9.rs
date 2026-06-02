//! D9 schema gates + D3 no-audit-in-main (backend/02 §2.14 CI gates).

use db::schema;
use db::Store;

#[tokio::test]
async fn table_names_are_singular() {
    let store = Store::open_in_memory().await.unwrap();
    assert!(
        schema::assert_singular_table_names(store.pool())
            .await
            .unwrap(),
        "no plural table names allowed (D9)"
    );

    let tables = schema::list_tables(store.pool()).await.unwrap();
    for t in schema::FIRST_CLASS_TABLES {
        assert!(
            tables.contains(&t.to_string()),
            "expected first-class table {t}"
        );
    }
    for t in schema::ASSOCIATION_TABLES {
        assert!(
            tables.contains(&t.to_string()),
            "expected association table {t}"
        );
    }
}

#[tokio::test]
async fn main_store_has_no_audit_tables() {
    // D3 / INV-06: audit lives in an independent audit.sqlite, never in the main store.
    let store = Store::open_in_memory().await.unwrap();
    assert!(
        schema::assert_no_audit_tables(store.pool()).await.unwrap(),
        "main store must contain no audit_log / audit_anchor table (D3)"
    );
}
