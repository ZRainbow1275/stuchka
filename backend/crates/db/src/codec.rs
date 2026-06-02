//! SQLite fallback codecs (data/01 §1.0.5 mapping matrix).
//!
//! Because R1 stores everything in SQLite via the fallback mapping, the repository layer
//! hand-codes the column representations rather than relying on `sqlx::FromRow` (which would
//! need native UUID / array / NUMERIC support absent under SQLite):
//!
//! - `UUID`        -> `TEXT(36)` canonical string
//! - `TIMESTAMPTZ` -> `TEXT` RFC3339 UTC
//! - `DATE`        -> `TEXT` 'YYYY-MM-DD'
//! - `JSONB`       -> `TEXT` JSON
//! - `UUID[]`      -> `TEXT` JSON array of UUID strings
//! - `TEXT[]`      -> `TEXT` JSON array of strings
//! - `NUMERIC`     -> `TEXT` fixed-point decimal string (REAL forbidden for money)
//! - `BOOLEAN`     -> `INTEGER` 0/1

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::{DbError, DbResult};

/// UUID -> canonical 36-char TEXT.
pub fn uuid_to_text(id: &Uuid) -> String {
    id.hyphenated().to_string()
}

/// TEXT -> UUID.
pub fn text_to_uuid(s: &str) -> DbResult<Uuid> {
    Ok(Uuid::parse_str(s)?)
}

/// Optional UUID -> Option<TEXT>.
pub fn opt_uuid_to_text(id: &Option<Uuid>) -> Option<String> {
    id.as_ref().map(uuid_to_text)
}

/// Optional TEXT -> Option<UUID>.
pub fn opt_text_to_uuid(s: &Option<String>) -> DbResult<Option<Uuid>> {
    match s {
        Some(s) => Ok(Some(text_to_uuid(s)?)),
        None => Ok(None),
    }
}

/// `DateTime<Utc>` -> RFC3339 TEXT (UTC, millisecond precision, trailing `Z`).
pub fn ts_to_text(t: &DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// RFC3339 TEXT -> `DateTime<Utc>`.
pub fn text_to_ts(s: &str) -> DbResult<DateTime<Utc>> {
    let dt = DateTime::parse_from_rfc3339(s)
        .map_err(|e| DbError::Sqlx(sqlx::Error::Decode(Box::new(e))))?;
    Ok(dt.with_timezone(&Utc))
}

/// Optional timestamp -> Option<TEXT>.
pub fn opt_ts_to_text(t: &Option<DateTime<Utc>>) -> Option<String> {
    t.as_ref().map(ts_to_text)
}

/// Optional TEXT -> Option<timestamp>.
pub fn opt_text_to_ts(s: &Option<String>) -> DbResult<Option<DateTime<Utc>>> {
    match s {
        Some(s) => Ok(Some(text_to_ts(s)?)),
        None => Ok(None),
    }
}

/// `NaiveDate` -> 'YYYY-MM-DD' TEXT.
pub fn date_to_text(d: &NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// 'YYYY-MM-DD' TEXT -> `NaiveDate`.
pub fn text_to_date(s: &str) -> DbResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| DbError::Sqlx(sqlx::Error::Decode(Box::new(e))))
}

/// Fixed-point money `Decimal` -> TEXT (REAL is forbidden for money — common hard constraint).
pub fn decimal_to_text(d: &Decimal) -> String {
    d.to_string()
}

/// TEXT -> fixed-point money `Decimal` (no precision loss vs REAL).
pub fn text_to_decimal(s: &str) -> DbResult<Decimal> {
    Ok(Decimal::from_str(s)?)
}

/// Optional money `Decimal` -> Option<TEXT>.
pub fn opt_decimal_to_text(d: &Option<Decimal>) -> Option<String> {
    d.as_ref().map(decimal_to_text)
}

/// Optional TEXT -> Option<money `Decimal`>.
pub fn opt_text_to_decimal(s: &Option<String>) -> DbResult<Option<Decimal>> {
    match s {
        Some(s) => Ok(Some(text_to_decimal(s)?)),
        None => Ok(None),
    }
}

/// `Vec<Uuid>` -> JSON-array TEXT (`UUID[]` fallback).
pub fn uuid_vec_to_json(v: &[Uuid]) -> DbResult<String> {
    let strs: Vec<String> = v.iter().map(uuid_to_text).collect();
    Ok(serde_json::to_string(&strs)?)
}

/// JSON-array TEXT -> `Vec<Uuid>`.
pub fn json_to_uuid_vec(s: &str) -> DbResult<Vec<Uuid>> {
    let strs: Vec<String> = serde_json::from_str(s)?;
    strs.iter().map(|x| text_to_uuid(x)).collect()
}

/// `Vec<String>` -> JSON-array TEXT (`TEXT[]` fallback).
pub fn str_vec_to_json(v: &[String]) -> DbResult<String> {
    Ok(serde_json::to_string(v)?)
}

/// JSON-array TEXT -> `Vec<String>`.
pub fn json_to_str_vec(s: &str) -> DbResult<Vec<String>> {
    Ok(serde_json::from_str(s)?)
}

/// `serde_json::Value` -> TEXT (`JSONB` fallback).
pub fn json_to_text(v: &serde_json::Value) -> DbResult<String> {
    Ok(serde_json::to_string(v)?)
}

/// TEXT -> `serde_json::Value`.
pub fn text_to_json(s: &str) -> DbResult<serde_json::Value> {
    Ok(serde_json::from_str(s)?)
}

/// Optional `serde_json::Value` -> Option<TEXT>.
pub fn opt_json_to_text(v: &Option<serde_json::Value>) -> DbResult<Option<String>> {
    match v {
        Some(v) => Ok(Some(json_to_text(v)?)),
        None => Ok(None),
    }
}

/// Optional TEXT -> Option<`serde_json::Value`>.
pub fn opt_text_to_json(s: &Option<String>) -> DbResult<Option<serde_json::Value>> {
    match s {
        Some(s) => Ok(Some(text_to_json(s)?)),
        None => Ok(None),
    }
}

/// `bool` -> `INTEGER` 0/1 (SQLite has no native boolean).
pub fn bool_to_int(b: bool) -> i64 {
    i64::from(b)
}

/// `INTEGER` -> `bool`.
pub fn int_to_bool(i: i64) -> bool {
    i != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_text_roundtrip() {
        let id = Uuid::now_v7();
        let s = uuid_to_text(&id);
        assert_eq!(s.len(), 36);
        assert_eq!(text_to_uuid(&s).unwrap(), id);
    }

    #[test]
    fn decimal_text_roundtrip_no_precision_loss() {
        for raw in [
            "0.00",
            "0.01",
            "8000.00",
            "123456789012.99",
            "999999999.999999999",
        ] {
            let d = Decimal::from_str(raw).unwrap();
            let s = decimal_to_text(&d);
            assert_eq!(text_to_decimal(&s).unwrap(), d, "lost precision for {raw}");
        }
    }

    #[test]
    fn uuid_vec_json_roundtrip() {
        let v = vec![Uuid::now_v7(), Uuid::now_v7()];
        let j = uuid_vec_to_json(&v).unwrap();
        assert_eq!(json_to_uuid_vec(&j).unwrap(), v);
    }

    #[test]
    fn ts_and_date_roundtrip() {
        let t = Utc::now();
        let s = ts_to_text(&t);
        assert!(s.ends_with('Z'));
        let back = text_to_ts(&s).unwrap();
        // millisecond precision round-trip
        assert_eq!(back.timestamp_millis(), t.timestamp_millis());

        let d = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        assert_eq!(text_to_date(&date_to_text(&d)).unwrap(), d);
    }
}
