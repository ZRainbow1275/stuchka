//! SQLite connection pool for the main store (backend/02 §2.11 / data/01 §1.6.3).
//!
//! Every connection enables `PRAGMA foreign_keys = ON` (so the FK RESTRICT / CASCADE policies
//! in the migrations are enforced — SQLite leaves FK off by default and the pragma is
//! per-connection) and `journal_mode = WAL` (persistent; concurrent reader/writer).

use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous};
use std::path::Path;
use std::str::FromStr;

use crate::error::DbResult;

/// Open a pooled connection to a SQLite file at `path`, creating it if missing.
///
/// Foreign keys are enforced and WAL is enabled on every connection.
pub async fn open_file(path: impl AsRef<Path>) -> DbResult<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    build_pool(opts).await
}

/// Open an in-memory SQLite store (tests / ephemeral). A single shared connection is used so
/// the schema persists for the pool lifetime.
pub async fn open_in_memory() -> DbResult<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(crate::error::DbError::Sqlx)?
        .foreign_keys(true);
    // A single connection keeps the in-memory schema alive across queries.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

async fn build_pool(opts: SqliteConnectOptions) -> DbResult<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn in_memory_has_foreign_keys_on() {
        let pool = open_in_memory().await.unwrap();
        let row = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        let fk: i64 = row.get(0);
        assert_eq!(fk, 1, "foreign_keys must be ON");
    }
}
