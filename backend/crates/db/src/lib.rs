//! `db` — sqlx dual-dialect access layer (master-index §0.4 / D2).
//!
//! R1 ships **SQLite only** (L0-03 spike conclusion; see the task research report
//! `.trellis/tasks/06-02-db-layer/research/L0-03-db-spike.md`). The crate keeps a `Dialect` seam
//! and preserves the Postgres DDL draft under `migrations/postgres/` for an R-phase reassessment
//! (D2 §2.11 / backend/05 §5.6), but only the SQLite dialect is materialized.
//!
//! Boundaries:
//! - **D3**: audit is NOT in the main store — it lives in an independent `audit.sqlite`
//!   (`crates/audit`). The main store contains no `audit_log` / `audit_anchor` table.
//! - **INV-01**: `data-model` is depended on with its optional `sqlx` feature so FromRow/Type
//!   derives are available here; `rule-engine` never enables it.
//! - sqlx is used via its run-time API only (`query` / `query_as` / `query_scalar`); the
//!   compile-time `query!` macros are never used, so the build is hermetic (no DATABASE_URL).
//!
//! Fallback mapping (data/01 §1.0.5): UUID->TEXT(36), ENUM->TEXT+CHECK IN, TIMESTAMPTZ->TEXT
//! (RFC3339), DATE->TEXT('YYYY-MM-DD'), JSONB->TEXT, UUID[]->TEXT(JSON), NUMERIC->TEXT
//! (fixed-point; REAL forbidden for money). FK RESTRICT/CASCADE is enforced via
//! `PRAGMA foreign_keys = ON` set on every connection.

pub mod codec;
pub mod dialect;
pub mod error;
pub mod migrate;
pub mod pool;
pub mod repo;
pub mod schema;

pub use dialect::Dialect;
pub use error::{DbError, DbResult};
pub use migrate::{current_version, run_migrations, MigrationReport};
pub use pool::{open_file, open_in_memory};
pub use repo::{
    CaseRepo, ClaimRepo, EvidenceRepo, FactRepo, GroupRepo, LawRefRepo, UserSettingRepo,
};

use sqlx::sqlite::SqlitePool;

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "db";

/// A handle bundling the main-store pool with all first-class-object repositories.
///
/// Construct with [`Store::open_file`] (production) or [`Store::open_in_memory`] (tests); both
/// run migrations before returning so the schema is ready.
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    /// Case root-aggregate repository.
    pub case: CaseRepo,
    /// Fact repository.
    pub fact: FactRepo,
    /// Evidence repository.
    pub evidence: EvidenceRepo,
    /// Claim repository.
    pub claim: ClaimRepo,
    /// Law-reference repository.
    pub law_ref: LawRefRepo,
    /// Group repository.
    pub group: GroupRepo,
    /// User-settings singleton repository (auxiliary object; cross-crate-reconciliation B-6).
    pub user_setting: UserSettingRepo,
}

impl Store {
    fn from_pool(pool: SqlitePool) -> Self {
        Self {
            case: CaseRepo::new(pool.clone()),
            fact: FactRepo::new(pool.clone()),
            evidence: EvidenceRepo::new(pool.clone()),
            claim: ClaimRepo::new(pool.clone()),
            law_ref: LawRefRepo::new(pool.clone()),
            group: GroupRepo::new(pool.clone()),
            user_setting: UserSettingRepo::new(pool.clone()),
            pool,
        }
    }

    /// Open (creating if needed) a file-backed main store and run migrations.
    pub async fn open_file(path: impl AsRef<std::path::Path>) -> DbResult<Self> {
        let pool = pool::open_file(path).await?;
        run_migrations(&pool).await?;
        Ok(Self::from_pool(pool))
    }

    /// Open an in-memory main store and run migrations (tests / ephemeral).
    pub async fn open_in_memory() -> DbResult<Self> {
        let pool = pool::open_in_memory().await?;
        run_migrations(&pool).await?;
        Ok(Self::from_pool(pool))
    }

    /// Borrow the underlying pool (for introspection helpers in `schema`).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "db");
    }
}
