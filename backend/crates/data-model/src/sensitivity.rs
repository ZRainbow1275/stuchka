//! Data sensitivity grading (`compliance/02` §2) — the four-tier `DataGrade` and the sensitive
//! field registry shared by `crates/crypto`, `crates/ai-dispatcher` (export guard), and
//! `crates/compliance`-side logic.
//!
//! Wire values are the spec's literal `L1`..`L4` (`compliance/02` §6 audit JSON uses
//! `"input_max_grade": "L3"`), so each variant is `serde(rename = "Ln")` to match exactly. Under
//! the `sqlx` feature the Postgres ENUM `data_grade` stores the same `L1`..`L4` strings.

use serde::{Deserialize, Serialize};

/// Four-tier data sensitivity grade (`compliance/02` §2). Used by the export guard to decide
/// routing (`compliance/02` §3.1):
///
/// - **L4 核心隐私** — never leaves the device (id card, bank card, audio bytes, medical images,
///   minor names).
/// - **L3 高敏个人信息** — domestic cloud only after desensitisation (phone, address, salary
///   breakdown, social-insurance id, injury detail).
/// - **L2 一般个人信息** — domestic cloud raw; overseas cloud requires separate consent.
/// - **L1 公共/非个人** — any channel (law-ref id, rule-engine result, template id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "data_grade"))]
pub enum DataGrade {
    #[serde(rename = "L1")]
    #[cfg_attr(feature = "sqlx", sqlx(rename = "L1"))]
    L1,
    #[serde(rename = "L2")]
    #[cfg_attr(feature = "sqlx", sqlx(rename = "L2"))]
    L2,
    #[serde(rename = "L3")]
    #[cfg_attr(feature = "sqlx", sqlx(rename = "L3"))]
    L3,
    #[serde(rename = "L4")]
    #[cfg_attr(feature = "sqlx", sqlx(rename = "L4"))]
    L4,
}

/// Field name → sensitivity grade map (`compliance/02` §2.1). The export guard looks a field up
/// here to decide its grade; the maximum grade across a payload's fields drives routing.
///
/// The R1 baseline entries below mirror `compliance/02` §2.1; the registry grows toward the
/// ~80-field first release as more fields are graded.
pub const SENSITIVE_FIELD_REGISTRY: &[(&str, DataGrade)] = &[
    // L4 核心隐私 — 强制本地，禁止任何云（含脱敏）
    ("id_card_number", DataGrade::L4),
    ("bank_card_number", DataGrade::L4),
    ("audio_raw_bytes", DataGrade::L4),
    ("medical_record_image", DataGrade::L4),
    ("minor_full_name", DataGrade::L4),
    ("audio_transcript", DataGrade::L4),
    ("medical_record_text", DataGrade::L4),
    // L3 高敏个人信息 — 不可境外云，境内云需脱敏
    ("phone_number", DataGrade::L3),
    ("home_address", DataGrade::L3),
    ("salary_breakdown", DataGrade::L3),
    ("social_insurance_id", DataGrade::L3),
    ("injury_detail_text", DataGrade::L3),
    ("first_description", DataGrade::L3),
    // L2 一般个人信息 — 境内云原文；境外云需单独同意
    ("employer_full_name", DataGrade::L2),
    ("position_title", DataGrade::L2),
    ("hire_date", DataGrade::L2),
    ("termination_date", DataGrade::L2),
    ("claim_demand", DataGrade::L2),
    // L1 公共 / 非个人 — 任意通道
    ("law_ref_id", DataGrade::L1),
    ("rule_engine_result", DataGrade::L1),
    ("template_id", DataGrade::L1),
    ("dispute_category", DataGrade::L1),
];

/// Look up a field's sensitivity grade in [`SENSITIVE_FIELD_REGISTRY`]. Returns `None` for an
/// unregistered field (caller decides the conservative default).
pub fn grade_of(field: &str) -> Option<DataGrade> {
    SENSITIVE_FIELD_REGISTRY
        .iter()
        .find(|(name, _)| *name == field)
        .map(|(_, grade)| *grade)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire value matches the `compliance/02` §6 audit JSON (`"L3"` etc.).
    #[test]
    fn data_grade_serializes_uppercase_l() {
        assert_eq!(serde_json::to_string(&DataGrade::L1).unwrap(), "\"L1\"");
        assert_eq!(serde_json::to_string(&DataGrade::L4).unwrap(), "\"L4\"");
        let back: DataGrade = serde_json::from_str("\"L3\"").unwrap();
        assert_eq!(back, DataGrade::L3);
    }

    /// Ordering reflects sensitivity (L4 is the most sensitive — `input_max_grade` uses `.max()`).
    #[test]
    fn data_grade_orders_by_sensitivity() {
        assert!(DataGrade::L4 > DataGrade::L3);
        assert!(DataGrade::L3 > DataGrade::L2);
        assert!(DataGrade::L2 > DataGrade::L1);
        let payload = [DataGrade::L1, DataGrade::L3, DataGrade::L2];
        assert_eq!(*payload.iter().max().unwrap(), DataGrade::L3);
    }

    #[test]
    fn registry_baseline_entries_present() {
        assert_eq!(grade_of("id_card_number"), Some(DataGrade::L4));
        assert_eq!(grade_of("phone_number"), Some(DataGrade::L3));
        assert_eq!(grade_of("employer_full_name"), Some(DataGrade::L2));
        assert_eq!(grade_of("law_ref_id"), Some(DataGrade::L1));
        assert_eq!(grade_of("not_a_field"), None);
    }

    #[test]
    fn registry_has_no_duplicate_field_names() {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in SENSITIVE_FIELD_REGISTRY {
            assert!(seen.insert(*name), "duplicate field in registry: {name}");
        }
    }
}
