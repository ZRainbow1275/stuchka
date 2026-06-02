//! LawRefRepo — law_ref CRUD + case_law_ref association (data/01 §1.4).

use data_model::LawRef;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::DbResult;

/// Repository over `law_ref` + the `case_law_ref` association.
#[derive(Clone)]
pub struct LawRefRepo {
    pool: SqlitePool,
}

impl LawRefRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a law reference snapshot.
    pub async fn insert(&self, lr: &LawRef) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO law_ref (\
                id, stable_id, content_hash, kb_version_label, title, version_date,\
                article, body_snapshot, created_at\
            ) VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid_to_text(&lr.id))
        .bind(&lr.stable_id)
        .bind(&lr.content_hash)
        .bind(&lr.kb_version_label)
        .bind(&lr.title)
        .bind(date_to_text(&lr.version_date))
        .bind(&lr.article)
        .bind(&lr.body_snapshot)
        .bind(ts_to_text(&lr.created_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a law reference by id.
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<LawRef>> {
        let row = sqlx::query("SELECT * FROM law_ref WHERE id = ?")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_law_ref(&r)?)),
            None => Ok(None),
        }
    }

    /// Delete a law reference by id. FK RESTRICT blocks deletion while a case_law_ref references it.
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM law_ref WHERE id = ?")
            .bind(uuid_to_text(id))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Link a case to a law reference for a given usage (diagnosis|document|calculation).
    /// Both FKs are RESTRICT (data/01 §1.8).
    pub async fn link_case(
        &self,
        case_id: &Uuid,
        law_ref_id: &Uuid,
        used_in: &str,
    ) -> DbResult<()> {
        sqlx::query("INSERT INTO case_law_ref (case_id, law_ref_id, used_in) VALUES (?,?,?)")
            .bind(uuid_to_text(case_id))
            .bind(uuid_to_text(law_ref_id))
            .bind(used_in)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List law_ref ids linked to a case.
    pub async fn linked_law_refs(&self, case_id: &Uuid) -> DbResult<Vec<Uuid>> {
        let rows = sqlx::query(
            "SELECT DISTINCT law_ref_id FROM case_law_ref WHERE case_id = ? ORDER BY law_ref_id",
        )
        .bind(uuid_to_text(case_id))
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| text_to_uuid(&r.get::<String, _>("law_ref_id")))
            .collect()
    }
}

fn row_to_law_ref(r: &sqlx::sqlite::SqliteRow) -> DbResult<LawRef> {
    Ok(LawRef {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        stable_id: r.get("stable_id"),
        content_hash: r.get("content_hash"),
        kb_version_label: r.get("kb_version_label"),
        title: r.get("title"),
        version_date: text_to_date(&r.get::<String, _>("version_date"))?,
        article: r.get("article"),
        body_snapshot: r.get("body_snapshot"),
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
    })
}
