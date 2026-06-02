//! CaseRepo — case root aggregate CRUD + state-machine guard + INV-04 KB freeze (backend/02 §2.8).

use data_model::enums::{CaseStatus, CoverageTier, DisputeSubtype, IdentityType};
use data_model::state_machine::validate_case_transition;
use data_model::Case;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::{DbError, DbResult};

/// Repository over the `"case"` table.
#[derive(Clone)]
pub struct CaseRepo {
    pool: SqlitePool,
}

impl CaseRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new case row (status whitelist not applied on insert; defaults to its current value).
    pub async fn insert(&self, c: &Case) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO \"case\" (\
                id, identity_type, dispute_subtype, dispute_category, coverage_tier,\
                case_occurred_at, province, city, region_code, kb_version_hash,\
                kb_version_label, status, group_id, dialogue_template_id,\
                created_at, updated_at, frozen_at\
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid_to_text(&c.id))
        .bind(enum_str(c.identity_type))
        .bind(subtype_str(c.dispute_subtype))
        .bind(&c.dispute_category)
        .bind(tier_str(c.coverage_tier))
        .bind(date_to_text(&c.case_occurred_at))
        .bind(&c.province)
        .bind(&c.city)
        .bind(&c.region_code)
        .bind(&c.kb_version_hash)
        .bind(&c.kb_version_label)
        .bind(status_str(c.status))
        .bind(opt_uuid_to_text(&c.group_id))
        .bind(&c.dialogue_template_id)
        .bind(ts_to_text(&c.created_at))
        .bind(ts_to_text(&c.updated_at))
        .bind(opt_ts_to_text(&c.frozen_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a case by id, excluding soft-deleted cases (migration 0008). A soft-deleted case
    /// reads as absent (`Ok(None)`), so `GET /case/:id` returns `E_NOT_FOUND` (backend/01 §1.2).
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<Case>> {
        let row = sqlx::query("SELECT * FROM \"case\" WHERE id = ? AND deleted_at IS NULL")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_case(&r)?)),
            None => Ok(None),
        }
    }

    /// List active (non-soft-deleted) cases, ordered by id (UUID v7 is time-sortable).
    pub async fn list_active(&self) -> DbResult<Vec<Case>> {
        let rows = sqlx::query("SELECT * FROM \"case\" WHERE deleted_at IS NULL ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_case).collect()
    }

    /// Soft-delete a case (backend/01 §1.2: DELETE /case/:id = soft delete with a 30-day recycle).
    /// Stamps `deleted_at = when` (RFC3339); the row stays for the recycle window so children keep
    /// their FK parent. Idempotent: re-deleting an already-deleted (or absent) case affects 0 rows.
    pub async fn soft_delete(&self, id: &Uuid, when: &data_model::Timestamp) -> DbResult<bool> {
        let res = sqlx::query(
            "UPDATE \"case\" SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(ts_to_text(when))
        .bind(uuid_to_text(id))
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Update a case, enforcing the status whitelist (data/01 §1.1.3) and the INV-04
    /// `kb_version_hash` freeze (backend/02 §2.8). Any attempt to change `kb_version_hash`
    /// returns `KbVersionFrozen` (-> E_INVALID_TRANSITION); any illegal status move returns the
    /// data-model `StateTransitionError`.
    pub async fn update(&self, c: &Case) -> DbResult<()> {
        let existing = self.get(&c.id).await?.ok_or(DbError::NotFound)?;

        // INV-04: kb_version_hash is frozen on creation, never updated.
        if existing.kb_version_hash != c.kb_version_hash {
            return Err(DbError::KbVersionFrozen);
        }

        // State-machine whitelist guard (mirrors SQL trigger).
        validate_case_transition(existing.status, c.status)?;

        sqlx::query(
            "UPDATE \"case\" SET \
                identity_type=?, dispute_subtype=?, dispute_category=?, coverage_tier=?,\
                case_occurred_at=?, province=?, city=?, region_code=?,\
                kb_version_label=?, status=?, group_id=?, dialogue_template_id=?,\
                updated_at=?, frozen_at=? \
             WHERE id=?",
        )
        .bind(enum_str(c.identity_type))
        .bind(subtype_str(c.dispute_subtype))
        .bind(&c.dispute_category)
        .bind(tier_str(c.coverage_tier))
        .bind(date_to_text(&c.case_occurred_at))
        .bind(&c.province)
        .bind(&c.city)
        .bind(&c.region_code)
        .bind(&c.kb_version_label)
        .bind(status_str(c.status))
        .bind(opt_uuid_to_text(&c.group_id))
        .bind(&c.dialogue_template_id)
        .bind(ts_to_text(&c.updated_at))
        .bind(opt_ts_to_text(&c.frozen_at))
        .bind(uuid_to_text(&c.id))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a case by id. FK RESTRICT (enforced by SQLite) blocks deletion while child
    /// fact / evidence / claim / case_law_ref / group rows reference it.
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM \"case\" WHERE id = ?")
            .bind(uuid_to_text(id))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Count active (non-soft-deleted) cases (the <= 3 parallel-case cap, backend/01 §1.2).
    pub async fn count(&self) -> DbResult<i64> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM \"case\" WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }
}

fn enum_str(v: IdentityType) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}
fn subtype_str(v: DisputeSubtype) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}
fn tier_str(v: CoverageTier) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}
fn status_str(v: CaseStatus) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn parse_enum<T: serde::de::DeserializeOwned>(s: &str) -> DbResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(DbError::Json)
}

fn row_to_case(r: &sqlx::sqlite::SqliteRow) -> DbResult<Case> {
    let identity: String = r.get("identity_type");
    let subtype: String = r.get("dispute_subtype");
    let tier: String = r.get("coverage_tier");
    let status: String = r.get("status");
    let region_code: Option<String> = r.get("region_code");
    let group_id: Option<String> = r.get("group_id");
    let frozen_at: Option<String> = r.get("frozen_at");

    Ok(Case {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        identity_type: parse_enum::<IdentityType>(&identity)?,
        dispute_subtype: parse_enum::<DisputeSubtype>(&subtype)?,
        dispute_category: r.get("dispute_category"),
        coverage_tier: parse_enum::<CoverageTier>(&tier)?,
        case_occurred_at: text_to_date(&r.get::<String, _>("case_occurred_at"))?,
        province: r.get("province"),
        city: r.get("city"),
        region_code,
        kb_version_hash: r.get("kb_version_hash"),
        kb_version_label: r.get("kb_version_label"),
        status: parse_enum::<CaseStatus>(&status)?,
        group_id: opt_text_to_uuid(&group_id)?,
        dialogue_template_id: r.get("dialogue_template_id"),
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
        updated_at: text_to_ts(&r.get::<String, _>("updated_at"))?,
        frozen_at: opt_text_to_ts(&frozen_at)?,
    })
}
