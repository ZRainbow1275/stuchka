//! ClaimRepo — claim CRUD + state-machine guard (incl. INV-10 withdraw) + money TEXT codec.
//!
//! Amounts are persisted as fixed-point decimal TEXT (REAL forbidden for money — common hard
//! constraint); the state-machine guard mirrors data/01 §1.5.3 and rejects `withdrawn` without
//! `withdraw_inv10_confirmed` (INV-10).

use data_model::enums::{ClaimStatus, ClaimType};
use data_model::state_machine::validate_claim_transition;
use data_model::Claim;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::{DbError, DbResult};

/// Repository over the `claim` table.
#[derive(Clone)]
pub struct ClaimRepo {
    pool: SqlitePool,
}

impl ClaimRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new claim. `case_id` must reference an existing case (FK RESTRICT).
    pub async fn insert(&self, c: &Claim) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO claim (\
                id, case_id, claim_type, amount_pre_tax, amount_post_tax,\
                calculation_breakdown, status, law_refs, fact_refs,\
                withdraw_inv10_confirmed, created_at, updated_at\
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid_to_text(&c.id))
        .bind(uuid_to_text(&c.case_id))
        .bind(val_str(c.claim_type))
        .bind(opt_decimal_to_text(&c.amount_pre_tax))
        .bind(opt_decimal_to_text(&c.amount_post_tax))
        .bind(json_to_text(&c.calculation_breakdown)?)
        .bind(val_str(c.status))
        .bind(uuid_vec_to_json(&c.law_refs)?)
        .bind(uuid_vec_to_json(&c.fact_refs)?)
        .bind(bool_to_int(c.withdraw_inv10_confirmed))
        .bind(ts_to_text(&c.created_at))
        .bind(ts_to_text(&c.updated_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a claim by id.
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<Claim>> {
        let row = sqlx::query("SELECT * FROM claim WHERE id = ?")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_claim(&r)?)),
            None => Ok(None),
        }
    }

    /// List claims for a case.
    pub async fn list_by_case(&self, case_id: &Uuid) -> DbResult<Vec<Claim>> {
        let rows = sqlx::query("SELECT * FROM claim WHERE case_id = ? ORDER BY id")
            .bind(uuid_to_text(case_id))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_claim).collect()
    }

    /// Update a claim, enforcing the status whitelist + INV-10 withdraw confirmation
    /// (data/01 §1.5.3). Illegal moves and unconfirmed withdrawals return `E_INVALID_TRANSITION`.
    pub async fn update(&self, c: &Claim) -> DbResult<()> {
        let existing = self.get(&c.id).await?.ok_or(DbError::NotFound)?;
        validate_claim_transition(existing.status, c.status, c.withdraw_inv10_confirmed)?;

        sqlx::query(
            "UPDATE claim SET \
                claim_type=?, amount_pre_tax=?, amount_post_tax=?, calculation_breakdown=?,\
                status=?, law_refs=?, fact_refs=?, withdraw_inv10_confirmed=?, updated_at=? \
             WHERE id=?",
        )
        .bind(val_str(c.claim_type))
        .bind(opt_decimal_to_text(&c.amount_pre_tax))
        .bind(opt_decimal_to_text(&c.amount_post_tax))
        .bind(json_to_text(&c.calculation_breakdown)?)
        .bind(val_str(c.status))
        .bind(uuid_vec_to_json(&c.law_refs)?)
        .bind(uuid_vec_to_json(&c.fact_refs)?)
        .bind(bool_to_int(c.withdraw_inv10_confirmed))
        .bind(ts_to_text(&c.updated_at))
        .bind(uuid_to_text(&c.id))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a claim by id.
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM claim WHERE id = ?")
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

fn row_to_claim(r: &sqlx::sqlite::SqliteRow) -> DbResult<Claim> {
    let claim_type: String = r.get("claim_type");
    let status: String = r.get("status");
    let pre_tax: Option<String> = r.get("amount_pre_tax");
    let post_tax: Option<String> = r.get("amount_post_tax");
    let breakdown: String = r.get("calculation_breakdown");
    let law_refs: String = r.get("law_refs");
    let fact_refs: String = r.get("fact_refs");
    let withdraw: i64 = r.get("withdraw_inv10_confirmed");

    Ok(Claim {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        case_id: Uuid::from_str(&r.get::<String, _>("case_id"))?,
        claim_type: parse_enum::<ClaimType>(&claim_type)?,
        amount_pre_tax: opt_text_to_decimal(&pre_tax)?,
        amount_post_tax: opt_text_to_decimal(&post_tax)?,
        calculation_breakdown: text_to_json(&breakdown)?,
        status: parse_enum::<ClaimStatus>(&status)?,
        law_refs: json_to_uuid_vec(&law_refs)?,
        fact_refs: json_to_uuid_vec(&fact_refs)?,
        withdraw_inv10_confirmed: int_to_bool(withdraw),
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
        updated_at: text_to_ts(&r.get::<String, _>("updated_at"))?,
    })
}
