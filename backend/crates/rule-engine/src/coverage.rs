//! Shared M9 / M5 outcome types (ai/02 §2.2, ai/06 §6.1).
//!
//! INC-1 ruling: `CoverageTag` is authored in `data-model` (D9 object-model authority, already
//! implemented) — this module **re-exports** it rather than redefining (ai/02 §2.2's inline copy
//! is illustrative). rule-engine adds the engine-specific wrappers `RuleOutcome` /
//! `ComputedValue` / `DerivationStep` / `RuleLawRef` / `RuleNextAction` here.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::deadline::buffer::DeadlineValue;
use crate::error::RuleError;

/// Re-export the authoritative coverage tag (INC-1: defined in `data-model`, not redefined).
pub use data_model::CoverageTag;

/// Light-weight LawRef carrier (INC-2). rule-engine holds no DB rows; it emits only the §0.5 URN
/// string. The single D8 parser lives in `data-model` (D8) — `parse` delegates to it for
/// validation and never builds a second parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleLawRef {
    /// D8 URN, e.g. `law:中华人民共和国劳动合同法/v2012-12-28/§47`.
    pub urn: String,
}

impl RuleLawRef {
    /// Validate URN shape via the data-model D8 parser, then construct. Returns
    /// [`RuleError::BadLawRef`] on a malformed URN (never panics).
    pub fn parse(urn: &str) -> Result<Self, RuleError> {
        data_model::parse_law_ref(urn).map_err(|e| RuleError::BadLawRef(format!("{urn}: {e}")))?;
        Ok(Self {
            urn: urn.to_string(),
        })
    }

    /// Build a slice of validated `RuleLawRef`s from URN literals (helper for fixed law sets).
    pub fn parse_all(urns: &[&str]) -> Result<Vec<Self>, RuleError> {
        urns.iter().map(|u| Self::parse(u)).collect()
    }
}

/// The computed payload of a successful rule outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComputedValue {
    /// M9 monetary output (`rust_decimal`, never f64 — OM-05).
    Money(Decimal),
    /// M5 deadline output (10% buffer + INV-08 manual-confirm flag).
    Deadline(DeadlineValue),
}

/// One step of a money derivation, surfaced row-by-row in the calculator UI
/// (`Prototype/pages/calculator.html`: 项目 / 公式 / 金额 / 依据).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationStep {
    /// Chinese label for the row (e.g. "工龄折算").
    pub label_zh: String,
    /// Literal expression for the row (e.g. "8000 × 8.0").
    pub expression: String,
    /// The numeric value contributed by this step.
    pub value: Decimal,
}

impl DerivationStep {
    pub fn new(label_zh: impl Into<String>, expression: impl Into<String>, value: Decimal) -> Self {
        Self {
            label_zh: label_zh.into(),
            expression: expression.into(),
            value,
        }
    }
}

/// rule-engine next-action with the per-variant payload the dispatcher needs (INC-3).
///
/// `data_model::NextAction` is the shared **discriminant-only** enum (it must be a `sqlx::Type`).
/// rule-engine carries the payload-bearing form here and maps to the bare discriminant when it
/// must cross the data-model boundary (see [`RuleNextAction::discriminant`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleNextAction {
    /// A required fact is missing — collect it before retrying.
    CollectFact { field: String },
    /// A class of evidence is missing.
    CollectEvidence { class: data_model::EvidenceCategory },
    /// Show a UI hint (e.g. the X-threshold disclaimer, D11).
    ShowUiHint { text: String },
    /// Deadline output must be human-reconfirmed (INV-08 anchor).
    ManualConfirm { reason: String },
    /// Route the user to legal-aid hotlines (make-usable 26-province guidance).
    ConsultLegalAid { hotline: Vec<String> },
}

impl RuleNextAction {
    /// Project onto the shared `data_model::NextAction` discriminant (D9 boundary crossing).
    pub fn discriminant(&self) -> data_model::NextAction {
        match self {
            RuleNextAction::CollectFact { .. } => data_model::NextAction::CollectFact,
            RuleNextAction::CollectEvidence { .. } => data_model::NextAction::CollectEvidence,
            RuleNextAction::ShowUiHint { .. } => data_model::NextAction::ShowUiHint,
            RuleNextAction::ManualConfirm { .. } => data_model::NextAction::ManualConfirm,
            RuleNextAction::ConsultLegalAid { .. } => data_model::NextAction::ConsultLegalAid,
        }
    }
}

/// The unified rule-engine outcome (ai/02 §2.2). Tagged `status` for the D1 Bearer-HTTP contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum RuleOutcome {
    /// A determinate result. `coverage_tag` records source confidence; AI may not override it
    /// (INV-01).
    #[serde(rename = "ok")]
    Ok {
        value: ComputedValue,
        coverage_tag: CoverageTag,
        law_refs: Vec<RuleLawRef>,
        derivation: Vec<DerivationStep>,
    },
    /// No determinate result. `coverage_tag` is always `Unknown` (or `Boundary` for phase gates);
    /// `next_actions` drives the Low-bucket heuristic follow-up (INV-08).
    #[serde(rename = "out_of_scope")]
    OutOfScope {
        coverage_tag: CoverageTag,
        reasons: Vec<String>,
        next_actions: Vec<RuleNextAction>,
    },
}

impl RuleOutcome {
    /// Convenience constructor for an `OutOfScope { Unknown }` caused by a single missing fact
    /// (C-C-6: never guess a default value).
    pub fn missing_fact(field: &str, reason: impl Into<String>) -> Self {
        RuleOutcome::OutOfScope {
            coverage_tag: CoverageTag::Unknown,
            reasons: vec![reason.into()],
            next_actions: vec![RuleNextAction::CollectFact {
                field: field.to_string(),
            }],
        }
    }

    /// True when the outcome carries a determinate value.
    pub fn is_ok(&self) -> bool {
        matches!(self, RuleOutcome::Ok { .. })
    }

    /// The coverage tag regardless of variant.
    pub fn coverage_tag(&self) -> CoverageTag {
        match self {
            RuleOutcome::Ok { coverage_tag, .. } => *coverage_tag,
            RuleOutcome::OutOfScope { coverage_tag, .. } => *coverage_tag,
        }
    }
}
