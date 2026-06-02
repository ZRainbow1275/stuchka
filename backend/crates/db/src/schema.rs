//! Schema introspection helpers (backend/02 §2.14 CI gates D9 / no-audit-in-main).
//!
//! These back the CI assertions: table names are singular (no `cases`/`facts`/`evidences`),
//! the main store contains NO audit tables (D3: audit lives in an independent audit.sqlite),
//! and primary keys are UUID-as-TEXT under the SQLite fallback.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::error::DbResult;

/// First-class object tables expected in the R1 main store (data/01 §1, all singular).
pub const FIRST_CLASS_TABLES: &[&str] = &["case", "fact", "evidence", "claim", "law_ref", "group"];

/// Association tables expected in the R1 main store (data/01 §1.0.2 singular semantics).
pub const ASSOCIATION_TABLES: &[&str] = &["case_law_ref", "group_contributor"];

/// Plural table names that must NEVER appear (D9 abolition list).
pub const FORBIDDEN_PLURAL_TABLES: &[&str] = &[
    "cases",
    "facts",
    "evidences",
    "law_refs",
    "groups",
    "claims",
];

/// Audit table names that must NEVER appear in the main store (D3 / INV-06 CI gate).
pub const FORBIDDEN_AUDIT_TABLES: &[&str] = &["audit_log", "audit_anchor", "audit_anchors"];

/// List every user table currently present in the connected SQLite store.
pub async fn list_tables(pool: &SqlitePool) -> DbResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>(0)).collect())
}

/// True when no forbidden plural table name is present (D9).
pub async fn assert_singular_table_names(pool: &SqlitePool) -> DbResult<bool> {
    let tables = list_tables(pool).await?;
    Ok(!tables
        .iter()
        .any(|t| FORBIDDEN_PLURAL_TABLES.contains(&t.as_str())))
}

/// True when the main store contains NO audit tables (D3 / INV-06 CI gate `no_audit_in_main`).
pub async fn assert_no_audit_tables(pool: &SqlitePool) -> DbResult<bool> {
    let tables = list_tables(pool).await?;
    Ok(!tables
        .iter()
        .any(|t| FORBIDDEN_AUDIT_TABLES.contains(&t.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_lists_are_disjoint_from_expected() {
        for t in FIRST_CLASS_TABLES {
            assert!(!FORBIDDEN_PLURAL_TABLES.contains(t));
            assert!(!FORBIDDEN_AUDIT_TABLES.contains(t));
        }
    }
}
