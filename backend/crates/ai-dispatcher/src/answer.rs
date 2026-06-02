//! Dispatcher domain types: [`UserQuery`], [`Answer`], [`ScoredAnswer`], [`EvidenceLink`].
//!
//! These are ai-dispatcher-owned domain types (not shared across crates), per brief Caveats. They
//! reuse the data-model shared enums ([`SourceTag`], [`CoverageTag`], [`NextAction`]) rather than
//! redefining them.

use data_model::{CoverageTag, NextAction, SourceTag};
use serde::{Deserialize, Serialize};

use crate::confidence::ConfBucket;
use crate::levels::DegradeLevel;

/// A user query entering [`crate::stage::answer`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuery {
    /// Free-text prompt (Chinese).
    pub text: String,
    /// Optional case id for context.
    pub case_id: Option<String>,
    /// User explicitly chose local-only.
    pub force_local: bool,
    /// User pre-consented to cross-border (overseas) this request.
    pub allow_cross_border: bool,
    /// Names of the structured payload fields accompanying this request (e.g. `"phone_number"`,
    /// `"employer_full_name"`). The data-export guard grades these via
    /// [`crate::routing::max_grade_of_fields`] to compute `input_max_grade` (`compliance/02` §2.1 /
    /// §3.1). Free-text-only requests leave this empty (graded conservatively as L2).
    #[serde(default)]
    pub structured_fields: Vec<String>,
    /// A §4.5 forced-local case scene applies (medical leave / three-periods / minor / sexual
    /// harassment / work-injury appraisal / criminal report / audio transcript). The caller maps
    /// the case subtype / document kind to this flag; the guard treats it as the highest priority
    /// (R0), overriding every user choice (INV-05). Distinct from `force_local` (a user choice).
    #[serde(default)]
    pub scene_forced_local: bool,
}

impl UserQuery {
    /// A minimal query from prompt text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            case_id: None,
            force_local: false,
            allow_cross_border: false,
            structured_fields: Vec::new(),
            scene_forced_local: false,
        }
    }
}

/// One link in the evidence chain (law-ref URN + optional KB fragment hash), surfaced to the api
/// `LlmQueryResp.evidence_chain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLink {
    /// Law-ref URN (D8) or KB stable id.
    pub law_ref: String,
    /// KB fragment hash / score reference, when the link came from a KB hit.
    pub kb_hash: Option<String>,
    /// Where this link came from.
    pub source: SourceTag,
}

impl EvidenceLink {
    /// A rule-engine law-ref link.
    pub fn rule(law_ref: impl Into<String>) -> Self {
        Self {
            law_ref: law_ref.into(),
            kb_hash: None,
            source: SourceTag::Rule,
        }
    }

    /// A KB hit link (stable id + score-derived hash).
    pub fn kb(stable_id: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            law_ref: stable_id.into(),
            kb_hash: Some(hash.into()),
            source: SourceTag::Kb,
        }
    }
}

/// The scored intermediate handed to INV-08 compose (ai/03 §3.4). Carries everything the templates
/// need to render the High / Mid / Low forms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredAnswer {
    /// Raw model / rule content.
    pub content: String,
    /// Final confidence (already capped / penalised).
    pub confidence: f32,
    /// Provenance.
    pub source_tag: SourceTag,
    /// Coverage tag (from the rule engine when present).
    pub coverage_tag: CoverageTag,
    /// Law refs / evidence links backing the answer.
    pub evidence_chain: Vec<EvidenceLink>,
    /// Degrade level the answer was produced under.
    pub level: DegradeLevel,
    /// Human-readable uncertainty reasons (Mid bucket).
    pub uncertainty_reasons: Vec<String>,
    /// Alternative next-best paths (Mid bucket; must be >= 2).
    pub alternatives: Vec<String>,
    /// Key facts to collect (Low bucket; must be >= 3). Sourced from rule next_actions when present.
    pub key_facts: Vec<String>,
    /// Next-action discriminants carried through to the api response.
    pub next_actions: Vec<NextAction>,
    /// Provider id label for the Online source template, if applicable.
    pub provider_label: Option<String>,
    /// KB version label for the Kb source template, if applicable.
    pub kb_version: Option<String>,
}

impl ScoredAnswer {
    /// A bare scored answer with empty derived fields.
    pub fn new(
        content: impl Into<String>,
        confidence: f32,
        source_tag: SourceTag,
        coverage_tag: CoverageTag,
        level: DegradeLevel,
    ) -> Self {
        Self {
            content: content.into(),
            confidence,
            source_tag,
            coverage_tag,
            evidence_chain: Vec::new(),
            level,
            uncertainty_reasons: Vec::new(),
            alternatives: Vec::new(),
            key_facts: Vec::new(),
            next_actions: Vec::new(),
            provider_label: None,
            kb_version: None,
        }
    }
}

/// The final composed answer (ai/01 §1.5 / ai/03 §3.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// User-facing rendered content (INV-08 templated).
    pub content: String,
    /// Final confidence.
    pub confidence: f32,
    /// Provenance.
    pub source_tag: SourceTag,
    /// Coverage tag.
    pub coverage_tag: CoverageTag,
    /// Law-ref / KB evidence chain.
    pub evidence_chain: Vec<EvidenceLink>,
    /// 0-4 degrade level.
    pub fallback_level: u8,
    /// Which INV-08 bucket the answer landed in.
    pub inv08_bucket: ConfBucket,
    /// Low-bucket heuristic follow-up facts (api `heuristic_followups`).
    pub heuristic_followups: Vec<String>,
    /// Next-action discriminants.
    pub next_actions: Vec<NextAction>,
    /// Whether the answer was blocked by a strong PII hit (api `pii_blocked`).
    pub pii_blocked: bool,
    /// Whether the answer is an abstention (no determinate result / refused). Never `true` purely
    /// for "model uncertain" (PM P5) — Low bucket is a heuristic follow-up, not an abstention.
    pub abstained: bool,
}

impl Answer {
    /// Level4 KB-stale warning answer (compensation refused; never falls back).
    pub fn level4_warning() -> Self {
        Self {
            content: "知识库已超过 30 天未更新，赔偿计算已暂停。请先更新知识库后再试，时效与流程提示仍可使用。"
                .to_string(),
            confidence: 0.0,
            source_tag: SourceTag::Kb,
            coverage_tag: CoverageTag::Unknown,
            evidence_chain: Vec::new(),
            fallback_level: DegradeLevel::Level4.as_u8(),
            inv08_bucket: ConfBucket::Low,
            heuristic_followups: vec![
                "更新知识库（联网增量同步）".to_string(),
                "确认本地知识库版本时间".to_string(),
                "更新完成后重新发起计算".to_string(),
            ],
            next_actions: vec![NextAction::ShowUiHint],
            pii_blocked: false,
            abstained: true,
        }
    }

    /// HSD-blocked answer when no local model is available (INV-05): processing stops, no cloud.
    pub fn hsd_blocked_no_local() -> Self {
        Self {
            content: "检测到高敏信息（如身份证、银行卡、电话、就医或录音路径），且本地小模型不可用，已停止处理以保护隐私。请下载本地小模型，或先脱敏后再咨询。"
                .to_string(),
            confidence: 0.0,
            source_tag: SourceTag::Rule,
            coverage_tag: CoverageTag::Unknown,
            evidence_chain: Vec::new(),
            fallback_level: DegradeLevel::Level3.as_u8(),
            inv08_bucket: ConfBucket::Low,
            heuristic_followups: vec![
                "下载本地小模型（5GB 可选）".to_string(),
                "将高敏信息脱敏后再咨询".to_string(),
                "改用纯规则的计算与时效模块".to_string(),
            ],
            next_actions: vec![NextAction::ShowUiHint],
            pii_blocked: true,
            abstained: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level4_warning_is_refused_at_level4() {
        let a = Answer::level4_warning();
        assert_eq!(a.fallback_level, 4);
        assert!(a.abstained);
    }

    #[test]
    fn hsd_blocked_sets_pii_blocked() {
        let a = Answer::hsd_blocked_no_local();
        assert!(a.pii_blocked);
        assert!(a.abstained);
    }
}
