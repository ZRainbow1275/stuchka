//! Migration runner for the independent `audit.sqlite` (data/04 §4.3 / §4.6.4).
//!
//! Migrations are embedded with `include_str!` (hermetic, no DATABASE_URL, no `query!` macro) and
//! applied idempotently behind a `schema_migration` ledger, mirroring `crates/db`'s runner but
//! over the audit pool (D3: zero db dependency).

use sqlx::sqlite::SqlitePool;

use crate::error::AuditError;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "audit_log",
        sql: include_str!("../migrations/0001_audit_log.sql"),
    },
    Migration {
        version: 2,
        name: "audit_anchor",
        sql: include_str!("../migrations/0002_audit_anchor.sql"),
    },
];

/// Apply all pending audit-store migrations idempotently.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), AuditError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migration (\
            version INTEGER PRIMARY KEY,\
            name    TEXT NOT NULL,\
            applied_at TEXT NOT NULL\
        )",
    )
    .execute(pool)
    .await?;

    for m in MIGRATIONS {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT version FROM schema_migration WHERE version = ?")
                .bind(m.version)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            continue;
        }
        let mut tx = pool.begin().await?;
        for stmt in split_sql_statements(m.sql) {
            // Connection-level PRAGMAs (journal_mode / synchronous) cannot run inside a
            // transaction and are already applied by the pool connect options (lib::open_*);
            // skip them here so the DDL stays transactional.
            if stmt.trim_start().to_uppercase().starts_with("PRAGMA") {
                continue;
            }
            sqlx::query(&stmt).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO schema_migration (version, name, applied_at) VALUES (?, ?, ?)")
            .bind(m.version)
            .bind(m.name)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Number of embedded audit migrations (CI assertion).
#[must_use]
pub fn migration_count() -> usize {
    MIGRATIONS.len()
}

/// Split a SQL script into individual statements on top-level semicolons (PRAGMA / DDL / triggers).
///
/// `CREATE TRIGGER ... BEGIN ... END;` bodies contain inner semicolons, so the splitter tracks a
/// `BEGIN`/`END` depth and only cuts on a semicolon when not inside a trigger body.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_trigger = false;
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            continue;
        }
        let upper = trimmed.to_uppercase();
        if upper.starts_with("CREATE TRIGGER") {
            in_trigger = true;
        }
        current.push_str(line);
        current.push('\n');

        if in_trigger {
            // The trigger statement ends at a line that is exactly `END;`.
            if upper == "END;" {
                in_trigger = false;
                let stmt = current.trim().to_string();
                if !stmt.is_empty() {
                    out.push(stmt);
                }
                current.clear();
            }
            continue;
        }

        if trimmed.ends_with(';') {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_migrations() {
        assert_eq!(migration_count(), 2);
    }

    #[test]
    fn trigger_bodies_are_not_split() {
        let stmts = split_sql_statements(MIGRATIONS[0].sql);
        // The two triggers must each be a single statement (BEGIN..END not split on inner `;`).
        let trigger_stmts: Vec<_> = stmts
            .iter()
            .filter(|s| s.to_uppercase().contains("CREATE TRIGGER"))
            .collect();
        assert_eq!(trigger_stmts.len(), 2);
        for t in trigger_stmts {
            assert!(t.to_uppercase().contains("RAISE(ABORT"));
            assert!(t.trim_end().ends_with("END;"));
        }
    }
}
