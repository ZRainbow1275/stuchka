//! GroupRepo — "group" + group_contributor CRUD (data/01 §1.6).
//!
//! `group` is a SQL reserved word and is always double-quoted. `group_contributor` has an
//! `ON DELETE CASCADE` FK to `"group"` (enforced by SQLite with foreign_keys=ON).

use data_model::{Group, GroupContributor};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

use crate::codec::*;
use crate::error::DbResult;

/// Repository over `"group"` + `group_contributor`.
#[derive(Clone)]
pub struct GroupRepo {
    pool: SqlitePool,
}

impl GroupRepo {
    /// Wrap a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a group. `case_id` must reference an existing case (FK RESTRICT).
    pub async fn insert(&self, g: &Group) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO \"group\" (id, case_id, initiator_id, inv10_confirmed_at, created_at) \
             VALUES (?,?,?,?,?)",
        )
        .bind(uuid_to_text(&g.id))
        .bind(uuid_to_text(&g.case_id))
        .bind(uuid_to_text(&g.initiator_id))
        .bind(ts_to_text(&g.inv10_confirmed_at))
        .bind(ts_to_text(&g.created_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch a group by id.
    pub async fn get(&self, id: &Uuid) -> DbResult<Option<Group>> {
        let row = sqlx::query("SELECT * FROM \"group\" WHERE id = ?")
            .bind(uuid_to_text(id))
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_group(&r)?)),
            None => Ok(None),
        }
    }

    /// Delete a group by id. group_contributor children cascade (FK ON DELETE CASCADE).
    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM \"group\" WHERE id = ?")
            .bind(uuid_to_text(id))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Add a contributor to a group (FK ON DELETE CASCADE to "group").
    pub async fn add_contributor(&self, gc: &GroupContributor) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO group_contributor (group_id, contributor_id, authorization_chain, joined_at) \
             VALUES (?,?,?,?)",
        )
        .bind(uuid_to_text(&gc.group_id))
        .bind(uuid_to_text(&gc.contributor_id))
        .bind(json_to_text(&gc.authorization_chain)?)
        .bind(ts_to_text(&gc.joined_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List contributors for a group.
    pub async fn list_contributors(&self, group_id: &Uuid) -> DbResult<Vec<GroupContributor>> {
        let rows = sqlx::query(
            "SELECT * FROM group_contributor WHERE group_id = ? ORDER BY contributor_id",
        )
        .bind(uuid_to_text(group_id))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_contributor).collect()
    }
}

fn row_to_group(r: &sqlx::sqlite::SqliteRow) -> DbResult<Group> {
    Ok(Group {
        id: Uuid::from_str(&r.get::<String, _>("id"))?,
        case_id: Uuid::from_str(&r.get::<String, _>("case_id"))?,
        initiator_id: Uuid::from_str(&r.get::<String, _>("initiator_id"))?,
        inv10_confirmed_at: text_to_ts(&r.get::<String, _>("inv10_confirmed_at"))?,
        created_at: text_to_ts(&r.get::<String, _>("created_at"))?,
    })
}

fn row_to_contributor(r: &sqlx::sqlite::SqliteRow) -> DbResult<GroupContributor> {
    Ok(GroupContributor {
        group_id: Uuid::from_str(&r.get::<String, _>("group_id"))?,
        contributor_id: Uuid::from_str(&r.get::<String, _>("contributor_id"))?,
        authorization_chain: text_to_json(&r.get::<String, _>("authorization_chain"))?,
        joined_at: text_to_ts(&r.get::<String, _>("joined_at"))?,
    })
}
