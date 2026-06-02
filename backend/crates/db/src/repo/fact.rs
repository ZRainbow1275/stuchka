//! FactRepo — fact CRUD + state-machine guard (data/01 §1.2.3).

use data_model::enums::{CoverageTag, FactCategory, FactSource, FactStatus};
use data_model::state_machine::validate_fact_transition;
use data_model::CaseFact;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::{DbError, DbResult};

/// Repository over the `fact` table.
#[derive(Clone)]
pub struct FactRepo {
    pool: SqlitePool,
}

impl FactRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new fact. `case_id` must reference an existing case (FK RESTRICT).
    pub async fn insert(&self, f: &CaseFact) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO fact (\
                id, case_id, content, category, status, source, confidence, coverage_tag,\
                evidence_refs, group_id, contributor_id, authorization_chain,\
                created_at, updated_at\
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid_to_text(&f.id))
        .bind(uuid_to_text(&f.case_id))
        .bind(&f.content)
        .bind(val_str(f.category))
        .bind(val_str(f.status))
        .bind(val_str(f.source))
        .bind(f.confidence.map(f64::from))
        .bind(val_str(f.coverage_tag))
        .bind(uuid_vec_to_json(&f.evidence_refs)?)
        .bind(opt_uuid_to_text(&f.group_id))
        .bind(opt_uuid_to_text(&f.contributor_id))
        .bind(opt_json_to_text(&f.authorization_chain)?)
        .bind(ts_to_text(&f.created_at))
        .bind(ts_to_text(&f.updated_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a fact by id.
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<CaseFact>> {
        let row = sqlx::query("SELECT * FROM fact WHERE id = ?")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_fact(&r)?)),
            None => Ok(None),
        }
    }

    /// List facts for a case, ordered by creation (v7 id is time-sortable).
    pub async fn list_by_case(&self, case_id: &Uuid) -> DbResult<Vec<CaseFact>> {
        let rows = sqlx::query("SELECT * FROM fact WHERE case_id = ? ORDER BY id")
            .bind(uuid_to_text(case_id))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_fact).collect()
    }

    /// Update a fact, enforcing the status whitelist (data/01 §1.2.3). Illegal status moves
    /// return `E_INVALID_TRANSITION` via the data-model guard.
    pub async fn update(&self, f: &CaseFact) -> DbResult<()> {
        let existing = self.get(&f.id).await?.ok_or(DbError::NotFound)?;
        validate_fact_transition(existing.status, f.status)?;

        sqlx::query(
            "UPDATE fact SET \
                content=?, category=?, status=?, source=?, confidence=?, coverage_tag=?,\
                evidence_refs=?, group_id=?, contributor_id=?, authorization_chain=?, updated_at=? \
             WHERE id=?",
        )
        .bind(&f.content)
        .bind(val_str(f.category))
        .bind(val_str(f.status))
        .bind(val_str(f.source))
        .bind(f.confidence.map(f64::from))
        .bind(val_str(f.coverage_tag))
        .bind(uuid_vec_to_json(&f.evidence_refs)?)
        .bind(opt_uuid_to_text(&f.group_id))
        .bind(opt_uuid_to_text(&f.contributor_id))
        .bind(opt_json_to_text(&f.authorization_chain)?)
        .bind(ts_to_text(&f.updated_at))
        .bind(uuid_to_text(&f.id))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a fact by id.
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM fact WHERE id = ?")
            .bind(uuid_to_text(id))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn val_str<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn parse_enum<T: serde::de::DeserializeOwned>(s: &str) -> DbResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(DbError::Json)
}

fn row_to_fact(r: &sqlx::sqlite::SqliteRow) -> DbResult<CaseFact> {
    let category: String = r.get("category");
    let status: String = r.get("status");
    let source: String = r.get("source");
    let coverage_tag: String = r.get("coverage_tag");
    let confidence: Option<f64> = r.get("confidence");
    let evidence_refs: String = r.get("evidence_refs");
    let group_id: Option<String> = r.get("group_id");
    let contributor_id: Option<String> = r.get("contributor_id");
    let auth_chain: Option<String> = r.get("authorization_chain");

    Ok(CaseFact {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        case_id: Uuid::from_str(&r.get::<String, _>("case_id"))?,
        content: r.get("content"),
        category: parse_enum::<FactCategory>(&category)?,
        status: parse_enum::<FactStatus>(&status)?,
        source: parse_enum::<FactSource>(&source)?,
        confidence: confidence.map(|c| c as f32),
        coverage_tag: parse_enum::<CoverageTag>(&coverage_tag)?,
        evidence_refs: json_to_uuid_vec(&evidence_refs)?,
        group_id: opt_text_to_uuid(&group_id)?,
        contributor_id: opt_text_to_uuid(&contributor_id)?,
        authorization_chain: opt_text_to_json(&auth_chain)?,
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
        updated_at: text_to_ts(&r.get::<String, _>("updated_at"))?,
    })
}
