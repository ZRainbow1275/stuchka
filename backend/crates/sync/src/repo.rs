//! Persistence traits + real SQLite implementations for the sync engine.
//!
//! Schema contract (02-database-schema §2.5 / §2.10): `doc_yjs_state` (snapshot + state vector +
//! pending updates + count), `doc_step` (the INV-06 ProseMirror Step mirror, `UNIQUE(doc_id,
//! step_no)`), and `peer` (mDNS trust store, keyed by the base58 ed25519 `peer_id`). The brief
//! places these traits "backed by crates/db"; because the dispatch constraint scopes edits to this
//! crate, sync owns the trait *contract* and a real, self-contained SQLite implementation that
//! creates the three tables on open (`ensure_schema`). This is a real persistence path (sqlx
//! runtime API, no `query!` macros, no DATABASE_URL — W7), not a mock; `crates/api` injects a
//! `&dyn DocRepo` etc. so the dialect can be swapped later (D2 seam).
//!
//! `peer` rows hold ed25519 public keys but `doc_*` rows hold opaque CRDT bytes; awareness is never
//! persisted (§3.10 / SY-12) so there is deliberately no awareness table.

use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::SyncResult;

/// A `doc_step` row (INV-06 main-store mirror of a ProseMirror Step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocStepRow {
    /// Document id (UUID v7).
    pub doc_id: Uuid,
    /// Monotonic step number within the document.
    pub step_no: i32,
    /// Full ProseMirror Step JSON.
    pub step_json: serde_json::Value,
    /// Yjs client id (string).
    pub client_id: String,
    /// Acting user id.
    pub actor: String,
    /// Optional decision reason (conflict resolution); `None` for a plain edit.
    pub reason: Option<String>,
}

/// A `document` row: the business metadata of an editor document (its owning case + template +
/// claim refs). The Yjs CRDT content lives in `doc_yjs_state`; this row is the D9 main-store handle
/// the api layer resolves `GET /document/:id` / export / merge against (backend/01 §1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    /// Document id (UUID v7).
    pub doc_id: Uuid,
    /// Owning case id (UUID v7).
    pub case_id: Uuid,
    /// Template id serde value (`arb_application` | `mediation` | …).
    pub template_id: String,
    /// Associated claim ids (JSON array of UUID strings).
    pub claim_ids: Vec<String>,
}

/// A `peer` row (mDNS trust store).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRow {
    /// ed25519 public key, base58 (the primary key — not a UUID).
    pub peer_id: String,
    /// Last observed hostname.
    pub hostname: String,
    /// Last observed LAN IP.
    pub ip: String,
    /// Last observed port.
    pub port: u16,
    /// User-approved (TOFU pairing complete).
    pub trusted: bool,
    /// Stored ed25519 public key (base58) once paired.
    pub pubkey: Option<String>,
}

/// Yjs document state persistence (03-sync-yjs §3.3.2).
#[async_trait]
pub trait DocRepo: Send + Sync {
    /// Load the latest snapshot (`doc_yjs_state.snapshot`), if any.
    async fn load_snapshot(&self, doc_id: &str) -> SyncResult<Option<Vec<u8>>>;
    /// Load the latest state vector, if any.
    async fn load_state_vector(&self, doc_id: &str) -> SyncResult<Option<Vec<u8>>>;
    /// Append one pending update (write-through).
    async fn append_update(&self, doc_id: &str, update: &[u8]) -> SyncResult<()>;
    /// Load all pending updates in insertion order.
    async fn load_pending(&self, doc_id: &str) -> SyncResult<Vec<Vec<u8>>>;
    /// `(count, total_bytes)` of pending updates.
    async fn pending_size(&self, doc_id: &str) -> SyncResult<(usize, usize)>;
    /// Replace snapshot + state vector, clear pending, set `snapshot_at = now`.
    async fn compact(&self, doc_id: &str, new_snapshot: &[u8], new_sv: &[u8]) -> SyncResult<()>;
}

/// ProseMirror Step persistence (main-store mirror, INV-06).
#[async_trait]
pub trait DocStepRepo: Send + Sync {
    /// Insert a Step row (idempotent on `UNIQUE(doc_id, step_no)`).
    async fn insert_doc_step(&self, step: DocStepRow) -> SyncResult<()>;
    /// Fetch a Step row by `(doc_id, step_no)`.
    async fn get_doc_step(&self, doc_id: Uuid, step_no: i32) -> SyncResult<Option<DocStepRow>>;
    /// The highest `step_no` recorded for a document (for monotone step numbering); `None` if no
    /// steps yet.
    async fn max_step_no(&self, doc_id: Uuid) -> SyncResult<Option<i32>>;
}

/// Document business-metadata persistence (main store; the api layer's `GET /document/:id` handle).
#[async_trait]
pub trait DocMetaRepo: Send + Sync {
    /// Insert a new document row.
    async fn insert_document(&self, row: DocumentRow) -> SyncResult<()>;
    /// Fetch a document row by id.
    async fn get_document(&self, doc_id: Uuid) -> SyncResult<Option<DocumentRow>>;
}

/// Peer trust store (mDNS + pairing).
#[async_trait]
pub trait PeerRepo: Send + Sync {
    /// Record / refresh a peer seen via mDNS (does not change trust).
    async fn upsert_seen(
        &self,
        peer_id: &str,
        hostname: &str,
        ip: &str,
        port: u16,
    ) -> SyncResult<()>;
    /// Set a peer's trust flag and (optionally) store its public key.
    async fn set_trusted(
        &self,
        peer_id: &str,
        trusted: bool,
        pubkey: Option<&str>,
    ) -> SyncResult<()>;
    /// Look up a peer's stored public key.
    async fn pubkey_of(&self, peer_id: &str) -> SyncResult<Option<String>>;
    /// Is a peer trusted?
    async fn is_trusted(&self, peer_id: &str) -> SyncResult<bool>;
    /// List all peers (for `GET /sync/peers`).
    async fn list(&self) -> SyncResult<Vec<PeerRow>>;
}

/// Create the three sync tables if missing. Safe to call repeatedly (idempotent DDL).
pub async fn ensure_schema(pool: &SqlitePool) -> SyncResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS doc_yjs_state (\
            doc_id TEXT PRIMARY KEY,\
            state_vector BLOB,\
            snapshot BLOB,\
            snapshot_at TEXT,\
            update_count INTEGER NOT NULL DEFAULT 0\
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS doc_yjs_pending (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            doc_id TEXT NOT NULL,\
            update_blob BLOB NOT NULL\
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS doc_step (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            doc_id TEXT NOT NULL,\
            step_no INTEGER NOT NULL,\
            step_json TEXT NOT NULL,\
            client_id TEXT NOT NULL,\
            actor TEXT NOT NULL,\
            reason TEXT,\
            UNIQUE(doc_id, step_no)\
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS peer (\
            peer_id TEXT PRIMARY KEY,\
            hostname TEXT NOT NULL DEFAULT '',\
            last_ip TEXT NOT NULL DEFAULT '',\
            last_port INTEGER NOT NULL DEFAULT 0,\
            trusted INTEGER NOT NULL DEFAULT 0,\
            pubkey TEXT,\
            last_seen TEXT\
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS document (\
            doc_id TEXT PRIMARY KEY,\
            case_id TEXT NOT NULL,\
            template_id TEXT NOT NULL,\
            claim_ids TEXT NOT NULL DEFAULT '[]',\
            created_at TEXT\
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// SQLite-backed [`DocRepo`].
#[derive(Clone)]
pub struct SqliteDocRepo {
    pool: SqlitePool,
}

impl SqliteDocRepo {
    /// Wrap a pool (caller has already run [`ensure_schema`]).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocRepo for SqliteDocRepo {
    async fn load_snapshot(&self, doc_id: &str) -> SyncResult<Option<Vec<u8>>> {
        let row: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT snapshot FROM doc_yjs_state WHERE doc_id = ?")
                .bind(doc_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    async fn load_state_vector(&self, doc_id: &str) -> SyncResult<Option<Vec<u8>>> {
        let row: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT state_vector FROM doc_yjs_state WHERE doc_id = ?")
                .bind(doc_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    async fn append_update(&self, doc_id: &str, update: &[u8]) -> SyncResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO doc_yjs_pending (doc_id, update_blob) VALUES (?, ?)")
            .bind(doc_id)
            .bind(update)
            .execute(&mut *tx)
            .await?;
        // Ensure a state row exists and bump its count.
        sqlx::query(
            "INSERT INTO doc_yjs_state (doc_id, update_count) VALUES (?, 1) \
             ON CONFLICT(doc_id) DO UPDATE SET update_count = update_count + 1",
        )
        .bind(doc_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn load_pending(&self, doc_id: &str) -> SyncResult<Vec<Vec<u8>>> {
        let rows =
            sqlx::query("SELECT update_blob FROM doc_yjs_pending WHERE doc_id = ? ORDER BY id ASC")
                .bind(doc_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<Vec<u8>, _>("update_blob"))
            .collect())
    }

    async fn pending_size(&self, doc_id: &str) -> SyncResult<(usize, usize)> {
        let rows = sqlx::query("SELECT update_blob FROM doc_yjs_pending WHERE doc_id = ?")
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?;
        let count = rows.len();
        let bytes = rows
            .iter()
            .map(|r| r.get::<Vec<u8>, _>("update_blob").len())
            .sum();
        Ok((count, bytes))
    }

    async fn compact(&self, doc_id: &str, new_snapshot: &[u8], new_sv: &[u8]) -> SyncResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO doc_yjs_state (doc_id, snapshot, state_vector, snapshot_at, update_count) \
             VALUES (?, ?, ?, ?, 0) \
             ON CONFLICT(doc_id) DO UPDATE SET \
               snapshot = excluded.snapshot, \
               state_vector = excluded.state_vector, \
               snapshot_at = excluded.snapshot_at, \
               update_count = 0",
        )
        .bind(doc_id)
        .bind(new_snapshot)
        .bind(new_sv)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM doc_yjs_pending WHERE doc_id = ?")
            .bind(doc_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// SQLite-backed [`DocStepRepo`].
#[derive(Clone)]
pub struct SqliteDocStepRepo {
    pool: SqlitePool,
}

impl SqliteDocStepRepo {
    /// Wrap a pool (caller has already run [`ensure_schema`]).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocStepRepo for SqliteDocStepRepo {
    async fn insert_doc_step(&self, step: DocStepRow) -> SyncResult<()> {
        let step_text = serde_json::to_string(&step.step_json)?;
        sqlx::query(
            "INSERT INTO doc_step (doc_id, step_no, step_json, client_id, actor, reason) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(doc_id, step_no) DO NOTHING",
        )
        .bind(step.doc_id.to_string())
        .bind(step.step_no)
        .bind(step_text)
        .bind(&step.client_id)
        .bind(&step.actor)
        .bind(&step.reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_doc_step(&self, doc_id: Uuid, step_no: i32) -> SyncResult<Option<DocStepRow>> {
        let row = sqlx::query(
            "SELECT doc_id, step_no, step_json, client_id, actor, reason \
             FROM doc_step WHERE doc_id = ? AND step_no = ?",
        )
        .bind(doc_id.to_string())
        .bind(step_no)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let step_json: String = row.get("step_json");
        Ok(Some(DocStepRow {
            doc_id: Uuid::parse_str(&row.get::<String, _>("doc_id")).unwrap_or(doc_id),
            step_no: row.get("step_no"),
            step_json: serde_json::from_str(&step_json)?,
            client_id: row.get("client_id"),
            actor: row.get("actor"),
            reason: row.get("reason"),
        }))
    }

    async fn max_step_no(&self, doc_id: Uuid) -> SyncResult<Option<i32>> {
        let row: Option<i32> =
            sqlx::query_scalar("SELECT MAX(step_no) FROM doc_step WHERE doc_id = ?")
                .bind(doc_id.to_string())
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        Ok(row)
    }
}

/// SQLite-backed [`DocMetaRepo`].
#[derive(Clone)]
pub struct SqliteDocMetaRepo {
    pool: SqlitePool,
}

impl SqliteDocMetaRepo {
    /// Wrap a pool (caller has already run [`ensure_schema`]).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DocMetaRepo for SqliteDocMetaRepo {
    async fn insert_document(&self, row: DocumentRow) -> SyncResult<()> {
        let claims = serde_json::to_string(&row.claim_ids)?;
        sqlx::query(
            "INSERT INTO document (doc_id, case_id, template_id, claim_ids, created_at) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(doc_id) DO NOTHING",
        )
        .bind(row.doc_id.to_string())
        .bind(row.case_id.to_string())
        .bind(&row.template_id)
        .bind(claims)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_document(&self, doc_id: Uuid) -> SyncResult<Option<DocumentRow>> {
        let row = sqlx::query(
            "SELECT doc_id, case_id, template_id, claim_ids FROM document WHERE doc_id = ?",
        )
        .bind(doc_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let claims_json: String = row.get("claim_ids");
        Ok(Some(DocumentRow {
            doc_id: Uuid::parse_str(&row.get::<String, _>("doc_id")).unwrap_or(doc_id),
            case_id: Uuid::parse_str(&row.get::<String, _>("case_id")).unwrap_or_default(),
            template_id: row.get("template_id"),
            claim_ids: serde_json::from_str(&claims_json).unwrap_or_default(),
        }))
    }
}

/// SQLite-backed [`PeerRepo`].
#[derive(Clone)]
pub struct SqlitePeerRepo {
    pool: SqlitePool,
}

impl SqlitePeerRepo {
    /// Wrap a pool (caller has already run [`ensure_schema`]).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PeerRepo for SqlitePeerRepo {
    async fn upsert_seen(
        &self,
        peer_id: &str,
        hostname: &str,
        ip: &str,
        port: u16,
    ) -> SyncResult<()> {
        sqlx::query(
            "INSERT INTO peer (peer_id, hostname, last_ip, last_port, last_seen) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(peer_id) DO UPDATE SET \
               hostname = excluded.hostname, \
               last_ip = excluded.last_ip, \
               last_port = excluded.last_port, \
               last_seen = excluded.last_seen",
        )
        .bind(peer_id)
        .bind(hostname)
        .bind(ip)
        .bind(port)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_trusted(
        &self,
        peer_id: &str,
        trusted: bool,
        pubkey: Option<&str>,
    ) -> SyncResult<()> {
        sqlx::query(
            "INSERT INTO peer (peer_id, trusted, pubkey) VALUES (?, ?, ?) \
             ON CONFLICT(peer_id) DO UPDATE SET trusted = excluded.trusted, pubkey = excluded.pubkey",
        )
        .bind(peer_id)
        .bind(trusted as i64)
        .bind(pubkey)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn pubkey_of(&self, peer_id: &str) -> SyncResult<Option<String>> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT pubkey FROM peer WHERE peer_id = ?")
                .bind(peer_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten())
    }

    async fn is_trusted(&self, peer_id: &str) -> SyncResult<bool> {
        let row: Option<i64> = sqlx::query_scalar("SELECT trusted FROM peer WHERE peer_id = ?")
            .bind(peer_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(matches!(row, Some(1)))
    }

    async fn list(&self) -> SyncResult<Vec<PeerRow>> {
        let rows = sqlx::query(
            "SELECT peer_id, hostname, last_ip, last_port, trusted, pubkey FROM peer ORDER BY peer_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| PeerRow {
                peer_id: r.get("peer_id"),
                hostname: r.get("hostname"),
                ip: r.get("last_ip"),
                port: r.get::<i64, _>("last_port") as u16,
                trusted: r.get::<i64, _>("trusted") == 1,
                pubkey: r.get("pubkey"),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::str::FromStr;

    async fn mem_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        ensure_schema(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn doc_repo_append_and_pending_size() {
        let pool = mem_pool().await;
        let repo = SqliteDocRepo::new(pool);
        repo.append_update("d1", &[1, 2, 3]).await.unwrap();
        repo.append_update("d1", &[4, 5]).await.unwrap();
        let (count, bytes) = repo.pending_size("d1").await.unwrap();
        assert_eq!(count, 2);
        assert_eq!(bytes, 5);
    }

    #[tokio::test]
    async fn doc_repo_compact_clears_pending() {
        let pool = mem_pool().await;
        let repo = SqliteDocRepo::new(pool);
        repo.append_update("d1", &[1, 2, 3]).await.unwrap();
        repo.compact("d1", &[9, 9], &[8]).await.unwrap();
        let (count, _) = repo.pending_size("d1").await.unwrap();
        assert_eq!(count, 0);
        assert_eq!(repo.load_snapshot("d1").await.unwrap(), Some(vec![9, 9]));
        assert_eq!(repo.load_state_vector("d1").await.unwrap(), Some(vec![8]));
    }

    #[tokio::test]
    async fn doc_step_insert_is_idempotent_on_unique() {
        let pool = mem_pool().await;
        let repo = SqliteDocStepRepo::new(pool);
        let doc_id = Uuid::now_v7();
        let row = DocStepRow {
            doc_id,
            step_no: 1,
            step_json: serde_json::json!({"stepType": "replace"}),
            client_id: "client-1".into(),
            actor: "user-1".into(),
            reason: Some("merge_decision".into()),
        };
        repo.insert_doc_step(row.clone()).await.unwrap();
        repo.insert_doc_step(row.clone()).await.unwrap(); // conflict -> no-op
        let got = repo.get_doc_step(doc_id, 1).await.unwrap().unwrap();
        assert_eq!(got.step_json, row.step_json);
        assert_eq!(got.reason.as_deref(), Some("merge_decision"));
    }

    #[tokio::test]
    async fn peer_trust_round_trip() {
        let pool = mem_pool().await;
        let repo = SqlitePeerRepo::new(pool);
        repo.upsert_seen("peerA", "host", "169.254.1.2", 4001)
            .await
            .unwrap();
        assert!(!repo.is_trusted("peerA").await.unwrap());
        repo.set_trusted("peerA", true, Some("pubkeyA"))
            .await
            .unwrap();
        assert!(repo.is_trusted("peerA").await.unwrap());
        assert_eq!(
            repo.pubkey_of("peerA").await.unwrap().as_deref(),
            Some("pubkeyA")
        );
        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].port, 4001);
        assert!(list[0].trusted);
    }
}
