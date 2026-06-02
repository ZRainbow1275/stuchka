//! Migration runner (backend/02 §2.13, single-direction migrations — no down.sql).
//!
//! R1 ships SQLite only (L0-03). Migrations are embedded with `include_str!` (hermetic build,
//! no DATABASE_URL, no `query!` macro) and applied idempotently: a `schema_migration` ledger
//! records applied versions so re-running `run_migrations` is a no-op (backend/02 §2.14
//! migration_oneway test). Connection PRAGMAs (`foreign_keys=ON`, `journal_mode=WAL`) are set by
//! the pool connector in `pool.rs`.

use sqlx::sqlite::SqlitePool;

use crate::error::DbResult;

/// One embedded migration: a stable version number + name + SQL body.
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// Embedded R1 SQLite migrations, in apply order. No `*_down.sql` exists (single-direction).
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "case",
        sql: include_str!("../migrations/0001_case.sql"),
    },
    Migration {
        version: 2,
        name: "fact",
        sql: include_str!("../migrations/0002_fact.sql"),
    },
    Migration {
        version: 3,
        name: "evidence",
        sql: include_str!("../migrations/0003_evidence.sql"),
    },
    Migration {
        version: 4,
        name: "claim",
        sql: include_str!("../migrations/0004_claim.sql"),
    },
    Migration {
        version: 5,
        name: "law_ref",
        sql: include_str!("../migrations/0005_law_ref.sql"),
    },
    Migration {
        version: 6,
        name: "group",
        sql: include_str!("../migrations/0006_group.sql"),
    },
    Migration {
        version: 7,
        name: "user_setting",
        sql: include_str!("../migrations/0007_user_setting.sql"),
    },
    Migration {
        version: 8,
        name: "case_soft_delete",
        sql: include_str!("../migrations/0008_case_soft_delete.sql"),
    },
];

/// Report of a migration run (backend/02 §2.13 `MigrationReport`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions newly applied during this run (empty on an up-to-date DB).
    pub applied: Vec<i64>,
    /// Versions already present before this run (skipped).
    pub skipped: Vec<i64>,
}

impl MigrationReport {
    /// True when nothing new was applied (the idempotent re-run case).
    pub fn is_noop(&self) -> bool {
        self.applied.is_empty()
    }
}

/// Highest migration version known to this build.
pub fn latest_version() -> i64 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

/// Apply all pending migrations idempotently against the main SQLite store.
///
/// Each migration runs inside a transaction together with its ledger insert, so a partial
/// failure leaves the ledger consistent. Re-running is a no-op (backend/02 §2.14).
pub async fn run_migrations(pool: &SqlitePool) -> DbResult<MigrationReport> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migration (\
            version INTEGER PRIMARY KEY,\
            name    TEXT NOT NULL,\
            applied_at TEXT NOT NULL\
        )",
    )
    .execute(pool)
    .await?;

    let mut applied = Vec::new();
    let mut skipped = Vec::new();

    for m in MIGRATIONS {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT version FROM schema_migration WHERE version = ?")
                .bind(m.version)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            skipped.push(m.version);
            continue;
        }

        let mut tx = pool.begin().await?;
        // A migration file may contain multiple statements; execute the whole script.
        for stmt in split_sql_statements(m.sql) {
            sqlx::query(&stmt).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO schema_migration (version, name, applied_at) VALUES (?, ?, ?)")
            .bind(m.version)
            .bind(m.name)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        applied.push(m.version);
    }

    Ok(MigrationReport { applied, skipped })
}

/// Current applied schema version (0 if the ledger does not yet exist or is empty).
pub async fn current_version(pool: &SqlitePool) -> DbResult<i64> {
    let row = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migration'",
    )
    .fetch_optional(pool)
    .await?;
    if row.is_none() {
        return Ok(0);
    }
    let v: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM schema_migration")
        .fetch_one(pool)
        .await?;
    Ok(v.unwrap_or(0))
}

/// Split a SQL script into individual statements on top-level semicolons.
///
/// The embedded migrations use only simple DDL (no PL/pgSQL bodies, no `;` inside string
/// literals), so a semicolon split with comment stripping is sufficient and keeps the runner
/// dependency-free.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in sql.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue; // skip full-line comments
        }
        current.push_str(line);
        current.push('\n');
        if line.trim_end().ends_with(';') {
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                out.push(stmt);
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Number of embedded migrations (for CI assertions).
pub fn migration_count() -> usize {
    MIGRATIONS.len()
}

/// True when no embedded migration name ends in `_down` (single-direction guarantee, §2.14).
pub fn has_no_down_migrations() -> bool {
    MIGRATIONS
        .iter()
        .all(|m| !m.name.ends_with("_down") && !m.sql.contains("DROP TABLE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_ddl() {
        let sql = "CREATE TABLE a (id TEXT);\n-- comment\nCREATE INDEX i ON a(id);\n";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].starts_with("CREATE TABLE a"));
        assert!(stmts[1].starts_with("CREATE INDEX i"));
    }

    #[test]
    fn no_down_migrations_present() {
        assert!(has_no_down_migrations());
    }

    #[test]
    fn eight_migrations_in_order() {
        assert_eq!(migration_count(), 8);
        assert_eq!(latest_version(), 8);
        let versions: Vec<i64> = MIGRATIONS.iter().map(|m| m.version).collect();
        assert_eq!(versions, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
