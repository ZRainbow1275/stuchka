//! EvidenceRepo — evidence CRUD + state-machine guard (data/01 §1.3.4).

use data_model::enums::{EvidenceCategory, EvidenceStatus};
use data_model::state_machine::validate_evidence_transition;
use data_model::Evidence;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::{DbError, DbResult};

/// Repository over the `evidence` table.
#[derive(Clone)]
pub struct EvidenceRepo {
    pool: SqlitePool,
}

impl EvidenceRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new evidence row. `case_id` must reference an existing case (FK RESTRICT).
    pub async fn insert(&self, e: &Evidence) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO evidence (\
                id, case_id, evidence_type, file_path, file_hash, mime_type, byte_size,\
                effective_score, score_breakdown, status, high_sensitivity, collected_at,\
                device_id, gps_coords, chain_membership, created_at, updated_at\
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid_to_text(&e.id))
        .bind(uuid_to_text(&e.case_id))
        .bind(val_str(e.evidence_type))
        .bind(&e.file_path)
        .bind(&e.file_hash)
        .bind(&e.mime_type)
        .bind(e.byte_size)
        .bind(f64::from(e.effective_score))
        .bind(json_to_text(&e.score_breakdown)?)
        .bind(val_str(e.status))
        .bind(bool_to_int(e.high_sensitivity))
        .bind(opt_ts_to_text(&e.collected_at))
        .bind(&e.device_id)
        .bind(opt_json_to_text(&e.gps_coords)?)
        .bind(str_vec_to_json(&e.chain_membership)?)
        .bind(ts_to_text(&e.created_at))
        .bind(ts_to_text(&e.updated_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch an evidence row by id.
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<Evidence>> {
        let row = sqlx::query("SELECT * FROM evidence WHERE id = ?")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_evidence(&r)?)),
            None => Ok(None),
        }
    }

    /// List evidence for a case ordered by effective_score DESC (idx_evidence_score).
    pub async fn list_by_case(&self, case_id: &Uuid) -> DbResult<Vec<Evidence>> {
        let rows = sqlx::query(
            "SELECT * FROM evidence WHERE case_id = ? ORDER BY effective_score DESC, id",
        )
        .bind(uuid_to_text(case_id))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_evidence).collect()
    }

    /// Update an evidence row, enforcing the status whitelist (data/01 §1.3.4). Illegal moves
    /// return `E_INVALID_TRANSITION`.
    pub async fn update(&self, e: &Evidence) -> DbResult<()> {
        let existing = self.get(&e.id).await?.ok_or(DbError::NotFound)?;
        validate_evidence_transition(existing.status, e.status)?;

        sqlx::query(
            "UPDATE evidence SET \
                evidence_type=?, file_path=?, file_hash=?, mime_type=?, byte_size=?,\
                effective_score=?, score_breakdown=?, status=?, high_sensitivity=?,\
                collected_at=?, device_id=?, gps_coords=?, chain_membership=?, updated_at=? \
             WHERE id=?",
        )
        .bind(val_str(e.evidence_type))
        .bind(&e.file_path)
        .bind(&e.file_hash)
        .bind(&e.mime_type)
        .bind(e.byte_size)
        .bind(f64::from(e.effective_score))
        .bind(json_to_text(&e.score_breakdown)?)
        .bind(val_str(e.status))
        .bind(bool_to_int(e.high_sensitivity))
        .bind(opt_ts_to_text(&e.collected_at))
        .bind(&e.device_id)
        .bind(opt_json_to_text(&e.gps_coords)?)
        .bind(str_vec_to_json(&e.chain_membership)?)
        .bind(ts_to_text(&e.updated_at))
        .bind(uuid_to_text(&e.id))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete an evidence row by id.
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM evidence WHERE id = ?")
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

fn row_to_evidence(r: &sqlx::sqlite::SqliteRow) -> DbResult<Evidence> {
    let evidence_type: String = r.get("evidence_type");
    let status: String = r.get("status");
    let score_breakdown: String = r.get("score_breakdown");
    let high_sensitivity: i64 = r.get("high_sensitivity");
    let collected_at: Option<String> = r.get("collected_at");
    let gps_coords: Option<String> = r.get("gps_coords");
    let chain_membership: String = r.get("chain_membership");
    let effective_score: f64 = r.get("effective_score");

    Ok(Evidence {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        case_id: Uuid::from_str(&r.get::<String, _>("case_id"))?,
        evidence_type: parse_enum::<EvidenceCategory>(&evidence_type)?,
        file_path: r.get("file_path"),
        file_hash: r.get("file_hash"),
        mime_type: r.get("mime_type"),
        byte_size: r.get("byte_size"),
        effective_score: effective_score as f32,
        score_breakdown: text_to_json(&score_breakdown)?,
        status: parse_enum::<EvidenceStatus>(&status)?,
        high_sensitivity: int_to_bool(high_sensitivity),
        collected_at: opt_text_to_ts(&collected_at)?,
        device_id: r.get("device_id"),
        gps_coords: opt_text_to_json(&gps_coords)?,
        chain_membership: json_to_str_vec(&chain_membership)?,
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
        updated_at: text_to_ts(&r.get::<String, _>("updated_at"))?,
    })
}
