//! `rule-engine` — M9 compute + M5 deadline (pure rules, zero AI / zero HTTP, INV-01).
//!
//! 0529 spec crate (master-index §0.4 / D6; ai/02 + ai/06 + ai/03). M9 lives in [`compute`],
//! M5 in [`deadline`]; both share [`coverage`] (`RuleOutcome` / `CoverageTag`), [`facts`] and
//! [`region`] (31-province YAML). The façade [`RuleEngine::try_resolve`] is **synchronous**
//! (INC-4: the dispatcher wraps async, rule-engine never links tokio). cargo-deny (`deny.toml`) +
//! `tests/isolation.rs` forbid any AI / HTTP / inference crate from the dependency tree.

pub mod compute;
pub mod coverage;
pub mod deadline;
pub mod diagnosis;
pub mod error;
pub mod evidence_score;
pub mod facts;
pub mod region;

pub use coverage::{
    ComputedValue, CoverageTag, DerivationStep, RuleLawRef, RuleNextAction, RuleOutcome,
};
pub use diagnosis::{
    CatalogEntry, DecisionTree, DiagnosisCatalog, DiagnosisEngine, DiagnosisInput,
    DiagnosisNextAction, DiagnosisOutput, DiagnosisResult, Procedure, Question, RecommendedProcedures,
    Step as DiagnosisStep,
};
pub use deadline::performance::{
    evaluate as evaluate_performance, InstallmentAssessment, InstallmentState, InstrumentKind,
    PerformanceFacts, PerformanceInstallment, PerformanceStatus,
};
pub use deadline::{DeadlineFacts, DeadlineKind};
pub use error::RuleError;
pub use evidence_score::{score as score_evidence, EvidenceScoreInputs, ScoreBreakdown};
pub use facts::{FactBundle, WageRecord};
pub use region::{RegionParams, RegionTable};

use serde::{Deserialize, Serialize};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "rule-engine";

/// M9 / M5 intent dispatched by [`RuleEngine::try_resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleIntent {
    Severance,
    Compensation,
    DoublePay,
    Overtime,
    Delay50,
    Injury,
    Tax,
    LitigationCost,
    XThreshold,
    /// M5 deadline intent, carrying the deadline kind.
    Deadline(DeadlineKind),
}

/// A unified rule-engine request (Stage D entry, ai/01 §1.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRequest {
    pub intent: RuleIntent,
    #[serde(default)]
    pub facts: FactBundle,
    #[serde(default)]
    pub deadline_facts: Option<DeadlineFacts>,
    /// GB/T 2260 province code (first 2 digits, D9).
    pub province: String,
    /// City code (first 4 digits, D9). Reserved for city-level wage lookups.
    #[serde(default)]
    pub city: String,
    /// Pre-tax severance amount for the `Tax` intent (post-tax switch input).
    #[serde(default)]
    pub severance_pre_tax: Option<rust_decimal::Decimal>,
}

/// The pure rule engine façade. Loads all 31 provinces at construction.
#[derive(Debug, Clone)]
pub struct RuleEngine {
    regions: RegionTable,
}

impl RuleEngine {
    /// Construct the engine, loading + schema-checking all embedded province YAMLs.
    pub fn new() -> Result<Self, RuleError> {
        Ok(Self {
            regions: RegionTable::load_embedded()?,
        })
    }

    /// Access the loaded region table (for diagnostics / API region listing).
    pub fn regions(&self) -> &RegionTable {
        &self.regions
    }

    /// Stage D unified entry (ai/01 §1.5). **Synchronous** (INC-4). Returns `None` only when the
    /// province is unknown; every covered intent yields `Some(RuleOutcome)` (an `Ok` or an
    /// `OutOfScope`). X-threshold always resolves to `OutOfScope` via `resolve_x_threshold`
    /// (never touches the panicking `compute_x_threshold` body, D11).
    pub fn try_resolve(&self, req: &RuleRequest) -> Option<RuleOutcome> {
        // X-threshold never needs a region and never panics.
        if matches!(req.intent, RuleIntent::XThreshold) {
            return Some(compute::x_threshold::resolve_x_threshold(&req.facts));
        }

        let region = self.regions.get(&req.province)?;

        let outcome = match req.intent {
            RuleIntent::Severance => compute::severance::compute(&req.facts, region),
            RuleIntent::Compensation => compute::compensation::compute(&req.facts, region),
            RuleIntent::DoublePay => compute::double_pay::compute(&req.facts, region),
            RuleIntent::Overtime => compute::overtime::compute(&req.facts, region),
            RuleIntent::Delay50 => compute::delay_50::compute(&req.facts, region),
            RuleIntent::Injury => compute::injury::compute(&req.facts, region),
            RuleIntent::LitigationCost => compute::litigation_cost::compare(&req.facts, region),
            RuleIntent::Tax => {
                let pre_tax = req.severance_pre_tax.unwrap_or(rust_decimal::Decimal::ZERO);
                compute::tax::switch(pre_tax, region)
            }
            RuleIntent::XThreshold => unreachable!("handled above"),
            RuleIntent::Deadline(kind) => match &req.deadline_facts {
                Some(df) => deadline::resolve(kind, df, region),
                None => Ok(RuleOutcome::missing_fact(
                    "case_occurred_at",
                    "缺少起算点（知道或应当知道权利被侵害之日），无法计算时效",
                )),
            },
        };

        // A hard RuleError (malformed input) is surfaced as Unknown rather than panicking.
        match outcome {
            Ok(o) => Some(o),
            Err(e) => Some(RuleOutcome::OutOfScope {
                coverage_tag: CoverageTag::Unknown,
                reasons: vec![format!("规则计算失败：{e}")],
                next_actions: vec![],
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "rule-engine");
    }

    #[test]
    fn engine_loads_all_31_provinces() {
        let engine = RuleEngine::new().expect("engine loads");
        assert_eq!(engine.regions().len(), 31);
    }
}
