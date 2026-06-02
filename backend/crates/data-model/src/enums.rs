//! First-class object enumerations (data/01 §1.0.4 / §1.2 / §1.3 / §1.5, ai/02 §2, ai/03 §3).
//!
//! Every enum serialises `rename_all = "snake_case"` for the D1 Bearer-HTTP JSON contract.
//! `sqlx::Type` derives are gated behind the optional `sqlx` feature via `cfg_attr` so the
//! db layer can persist them while rule-engine never depends on sqlx (INV-01 boundary).
//!
//! The `sqlx(type_name = ...)` attribute names the Postgres ENUM type; under the SQLite
//! fallback the value is stored as `TEXT` and the rename_all snake_case form is the
//! on-disk representation.

use serde::{Deserialize, Serialize};

/// Identity type — 11 forced single-choice (data/01 §1.0.4, prd §3.1.1). R1 immutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "identity_type", rename_all = "snake_case")
)]
pub enum IdentityType {
    StandardFullTime,   // 01 城镇标准全日制
    Dispatch,           // 02 劳务派遣
    PartTime,           // 03 非全日制
    NewEmployment,      // 04 网约车 / 外卖 / 即时配送
    DomesticService,    // 05 家政 / 月嫂 / 钟点工
    ConstructionLabor,  // 06 建筑劳务（农民工）
    IndividualEmployee, // 07 个体工商户雇员
    Intern,             // 08 实习生 / 学徒
    RetiredRehired,     // 09 退休返聘
    Contractor,         // 10 承包 / 承揽 / 自由职业
    DeFactoNoContract,  // 11 未签合同事实劳动
}

/// Dispute subtype — 8 social-insurance branches (data/01 §1.1.4, prd §3.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "dispute_subtype", rename_all = "snake_case")
)]
pub enum DisputeSubtype {
    SocialInsWaiverInvalid,   // 1 弃缴约定无效 + 单方解除（法释 12 号 §19）
    SocialInsArrears,         // 2 普通欠缴
    SocialInsUnderpaidBase,   // 3 低缴（abstention）
    SocialInsIntermittent,    // 4 漏缴
    SocialInsProxy,           // 5 挂靠社保（abstention）
    SocialInsUninsuredInjury, // 6 未参保导致工伤损失
    SocialInsCrossPeriod,     // 7 跨期争议（abstention）
    SocialInsBaseDispute,     // 8 基数缴费争议（abstention）
}

/// Coverage tier — 做深 / 做能用 (data/01 §1.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "coverage_tier", rename_all = "snake_case")
)]
pub enum CoverageTier {
    MakeDeep, // 做深
    #[default]
    MakeUsable, // 做能用（default）
}

/// Case lifecycle status (data/01 §1.1.3 state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "case_status", rename_all = "snake_case")
)]
pub enum CaseStatus {
    #[default]
    Draft,
    Diagnosed,
    Confirmed,
    Frozen,
    Disputed,
}

/// Fact category (data/01 §1.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "fact_category", rename_all = "snake_case")
)]
pub enum FactCategory {
    RelationQualification, // 关系定性
    Wage,                  // 工资
    TimePeriod,            // 时间
    TerminationReason,     // 解除原因
    WorkInjury,            // 工伤
    Discrimination,        // 歧视
    Other,                 // 其他
}

/// Fact lifecycle status (data/01 §1.2.3 state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "fact_status", rename_all = "snake_case")
)]
pub enum FactStatus {
    #[default]
    Pending,
    Confirmed,
    Disputed,
    Deprecated,
}

/// Fact provenance (data/01 §1.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "fact_source", rename_all = "snake_case")
)]
pub enum FactSource {
    UserInput,
    AiInferred,
    RuleEngine,
}

/// Evidence category — 7 classes (data/01 §1.3.3).
///
/// Named `EvidenceCategory` per the API contract (backend/01 `EvidenceCategory`); the
/// data/01 Postgres ENUM type name remains `evidence_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "evidence_type", rename_all = "snake_case")
)]
pub enum EvidenceCategory {
    DocumentaryContract,  // 1 书证 / 合同
    AudioVideo,           // 2 录音 / 录像
    DigitalCommunication, // 3 电子聊天 / 通讯记录
    WitnessStatement,     // 4 证人证言
    ScenePhotoVideo,      // 5 现场照片 / 视频
    ThirdPartyData,       // 6 银行流水 / 税单 / 社保
    Appraisal,            // 7 鉴定 / 评估（工伤 / 职业病 / 伤残）
}

/// Evidence lifecycle status (data/01 §1.3.4 state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "evidence_status", rename_all = "snake_case")
)]
pub enum EvidenceStatus {
    #[default]
    Uploaded,
    Parsed,
    Scored,
    Verified,
    Disputed,
    Quarantined,
}

/// Claim type (data/01 §1.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "claim_type", rename_all = "snake_case")
)]
pub enum ClaimType {
    EconomicCompensation,      // 经济补偿
    EconomicDamage,            // 经济赔偿
    DoubleWageNoContract,      // 二倍工资
    OvertimePay,               // 加班费
    MaliciousArrearsSurcharge, // 50% 加付
    WorkInjuryBenefit,         // 工伤待遇
    Other,
}

/// Claim lifecycle status (data/01 §1.5.3 state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "claim_status", rename_all = "snake_case")
)]
pub enum ClaimStatus {
    #[default]
    Draft,
    Finalized,
    Granted,
    Denied,
    Withdrawn,
}

/// Coverage tag — rule-engine source-coverage classification (ai/02 §2, INV-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "coverage_tag", rename_all = "snake_case")
)]
pub enum CoverageTag {
    Exact,       // 法源精确匹配
    Approximate, // 法源大致适用
    Boundary,    // 法源边界条件
    Unknown,     // 无法源支撑，必须 abstention
}

/// Source tag — answer provenance for INV-08 disclosure (ai/03 §3.1, `crates/data-model/src/source_tag.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "source_tag", rename_all = "snake_case")
)]
pub enum SourceTag {
    Rule,     // 规则引擎（M9 / M5）—— INV-01 优先
    Kb,       // 本地知识库（带 kb_version 冻结 hash）
    Online,   // 联网检索（带 provider + ts）
    Inferred, // 模型推断（最低可信度）
}

/// Audit category projection (backend/01 §1.8 `AuditCategory`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "audit_category", rename_all = "snake_case")
)]
pub enum AuditCategory {
    AiSuggestion,
    UserReview,
    StateChange,
    MergeDecision,
    CryptoOp,
    Export,
    Sync,
}

/// Next-action kind suggested when the rule engine / deadline engine abstains (ai/03 §3.4,
/// shared by rule-engine + ai-dispatcher). The Low-bucket key-facts are populated from these.
///
/// data/01 carries no associated payload here so the value can be a `sqlx::Type` ENUM; the
/// per-variant detail (field name / evidence class / hint text) is carried alongside in the
/// dispatcher's `next_actions` payload, keeping this shared enum a plain discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "next_action", rename_all = "snake_case")
)]
pub enum NextAction {
    CollectFact,     // 缺事实
    CollectEvidence, // 缺证据
    ShowUiHint,      // UI 提示（如 X 阈值免责）
    ManualConfirm,   // M5 时效 buffer 强制人工二次确认（INV-08 锚）
    ConsultLegalAid, // 引导法援热线
}

/// Law hierarchy level (backend/01 §1.7 `LawLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "law_level", rename_all = "snake_case")
)]
pub enum LawLevel {
    Constitution,             // 宪法
    Statute,                  // 法律
    JudicialInterpretation,   // 司法解释
    AdministrativeRegulation, // 行政法规
    DepartmentalRule,         // 部门规章
    LocalRegulation,          // 地方性法规
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_type_serializes_snake_case() {
        let s = serde_json::to_string(&IdentityType::DeFactoNoContract).unwrap();
        assert_eq!(s, "\"de_facto_no_contract\"");
        let s = serde_json::to_string(&IdentityType::StandardFullTime).unwrap();
        assert_eq!(s, "\"standard_full_time\"");
    }

    #[test]
    fn dispute_subtype_serializes_snake_case() {
        let s = serde_json::to_string(&DisputeSubtype::SocialInsWaiverInvalid).unwrap();
        assert_eq!(s, "\"social_ins_waiver_invalid\"");
    }

    #[test]
    fn case_status_roundtrip() {
        for st in [
            CaseStatus::Draft,
            CaseStatus::Diagnosed,
            CaseStatus::Confirmed,
            CaseStatus::Frozen,
            CaseStatus::Disputed,
        ] {
            let s = serde_json::to_string(&st).unwrap();
            let back: CaseStatus = serde_json::from_str(&s).unwrap();
            assert_eq!(st, back);
        }
        assert_eq!(
            serde_json::to_string(&CaseStatus::Frozen).unwrap(),
            "\"frozen\""
        );
    }

    #[test]
    fn defaults_match_spec() {
        assert_eq!(CoverageTier::default(), CoverageTier::MakeUsable);
        assert_eq!(CaseStatus::default(), CaseStatus::Draft);
        assert_eq!(FactStatus::default(), FactStatus::Pending);
        assert_eq!(EvidenceStatus::default(), EvidenceStatus::Uploaded);
        assert_eq!(ClaimStatus::default(), ClaimStatus::Draft);
    }

    #[test]
    fn evidence_category_seven_classes_snake_case() {
        assert_eq!(
            serde_json::to_string(&EvidenceCategory::DocumentaryContract).unwrap(),
            "\"documentary_contract\""
        );
        assert_eq!(
            serde_json::to_string(&EvidenceCategory::ThirdPartyData).unwrap(),
            "\"third_party_data\""
        );
    }

    #[test]
    fn coverage_and_source_tags_snake_case() {
        assert_eq!(
            serde_json::to_string(&CoverageTag::Unknown).unwrap(),
            "\"unknown\""
        );
        assert_eq!(
            serde_json::to_string(&SourceTag::Online).unwrap(),
            "\"online\""
        );
    }

    #[test]
    fn claim_type_snake_case() {
        assert_eq!(
            serde_json::to_string(&ClaimType::MaliciousArrearsSurcharge).unwrap(),
            "\"malicious_arrears_surcharge\""
        );
        assert_eq!(
            serde_json::to_string(&ClaimType::DoubleWageNoContract).unwrap(),
            "\"double_wage_no_contract\""
        );
    }

    #[test]
    fn audit_category_and_law_level_snake_case() {
        assert_eq!(
            serde_json::to_string(&AuditCategory::MergeDecision).unwrap(),
            "\"merge_decision\""
        );
        assert_eq!(
            serde_json::to_string(&LawLevel::JudicialInterpretation).unwrap(),
            "\"judicial_interpretation\""
        );
    }

    #[test]
    fn next_action_snake_case_roundtrip() {
        assert_eq!(
            serde_json::to_string(&NextAction::CollectFact).unwrap(),
            "\"collect_fact\""
        );
        assert_eq!(
            serde_json::to_string(&NextAction::ConsultLegalAid).unwrap(),
            "\"consult_legal_aid\""
        );
        for a in [
            NextAction::CollectFact,
            NextAction::CollectEvidence,
            NextAction::ShowUiHint,
            NextAction::ManualConfirm,
            NextAction::ConsultLegalAid,
        ] {
            let s = serde_json::to_string(&a).unwrap();
            let back: NextAction = serde_json::from_str(&s).unwrap();
            assert_eq!(a, back);
        }
    }
}
