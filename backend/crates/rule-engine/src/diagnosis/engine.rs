//! M1 诊断引擎 (prd/04 §4.1, C-A-1 / C-A-2) — the DETERMINISTIC diagnosis engine.
//!
//! INV-01 / automation level A (AI 不参与): given a finished 问诊树 answer path plus the case
//! metadata, this engine produces the structured [`DiagnosisOutput`] (§4.1.2) with **pure rule
//! logic** — no AI, no HTTP, no sqlx. An LLM may only ASSIST by mapping the user's free-text
//! description onto the structured answer values (`Answer::value`); once those values exist the
//! engine stands alone and is fully reproducible.
//!
//! The output is the §4.1.2 object field-for-field:
//! `identity_type` · `dispute_subtype` · `dispute_category` · `coverage_tier` · `confidence` ·
//! `coverage_tag` · `recommended_procedures` · `next_actions`.

use data_model::{CoverageTag, CoverageTier, DisputeSubtype, IdentityType, NextAction};
use serde::{Deserialize, Serialize};

use crate::diagnosis::catalog::{CatalogEntry, DiagnosisCatalog, RecommendedProcedures};
use crate::diagnosis::tree::DecisionTree;
use crate::error::RuleError;

/// The diagnosis input — case metadata (prd §4.1.1) + the completed 问诊树 answer path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisInput {
    /// Identity triage (11 选 1, immutable, prd §4.1.1).
    pub identity_type: IdentityType,
    /// Social-insurance dispute subtype (8 选 1, prd §4.1.1 / §3.3.2).
    pub dispute_subtype: DisputeSubtype,
    /// The ordered 问诊树 answer values (one `Answer::value` per question). The free-text
    /// description is mapped onto these by the LLM assist BEFORE the engine runs (INV-01).
    pub answer_path: Vec<String>,
    /// Number of questions the user skipped (prd §4.1.4: ≥ 3 skips → 启发式追问 / Low confidence).
    #[serde(default)]
    pub skipped_questions: u32,
}

/// A diagnosed next-action with its detail payload (mirrors the rule-engine `RuleNextAction`
/// shape used by M9/M5, so the dispatcher surfaces them identically).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosisNextAction {
    /// Collect a missing fact before computation.
    CollectFact { field: String },
    /// Show a UI hint (e.g. region-missing, deep/usable disclaimer).
    ShowUiHint { text: String },
    /// Route the user to legal-aid hotlines (make-usable guidance).
    ConsultLegalAid { hint: String },
    /// Start a recommended procedure (主路径 / 并行 / 备用).
    StartProcedure { procedure: String, role: String },
}

impl DiagnosisNextAction {
    /// Project onto the shared `data_model::NextAction` discriminant (D9 boundary crossing).
    pub fn discriminant(&self) -> NextAction {
        match self {
            DiagnosisNextAction::CollectFact { .. } => NextAction::CollectFact,
            DiagnosisNextAction::ShowUiHint { .. } => NextAction::ShowUiHint,
            DiagnosisNextAction::ConsultLegalAid { .. } => NextAction::ConsultLegalAid,
            DiagnosisNextAction::StartProcedure { .. } => NextAction::CollectEvidence,
        }
    }
}

/// The structured diagnosis result object (prd §4.1.2), tagged `status` so an `out_of_scope`
/// abstention is distinguishable from a determinate `ok` (INV-01 / prd §4.1.5 abstention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum DiagnosisOutput {
    /// A determinate diagnosis — exactly one of the 85 subcategories.
    #[serde(rename = "ok")]
    Ok(Box<DiagnosisResult>),
    /// No determinate subcategory (abstention). `coverage_tag` is `Unknown`; never guesses.
    #[serde(rename = "out_of_scope")]
    OutOfScope {
        coverage_tag: CoverageTag,
        reasons: Vec<String>,
        next_actions: Vec<DiagnosisNextAction>,
    },
}

/// The determinate diagnosis payload (prd §4.1.2 object).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisResult {
    /// 11 选 1.
    pub identity_type: IdentityType,
    /// 8 选 1.
    pub dispute_subtype: DisputeSubtype,
    /// `LD-NN-NN` subcategory id (20 大类 85 子类).
    pub dispute_category: String,
    /// Subcategory Chinese name (convenience for the UI).
    pub dispute_category_name: String,
    /// 做深 / 做能用.
    pub coverage_tier: CoverageTier,
    /// 0-1 confidence (deterministic; skips lower it, prd §4.1.4).
    pub confidence: f32,
    /// exact / approximate / boundary / unknown (INV-01).
    pub coverage_tag: CoverageTag,
    /// Primary LawRef URN (§0.5, D8) for the subcategory.
    pub law_ref: String,
    /// 主路径 / 并行 / 备用 (prd §4.1.2 recommended_procedures).
    pub recommended_procedures: RecommendedProcedures,
    /// User next-step concrete actions (prd §4.1.2 next_actions — no "建议你思考").
    pub next_actions: Vec<DiagnosisNextAction>,
}

/// The deterministic diagnosis engine (prd §4.1). Holds the immutable catalog + tree; `diagnose`
/// is `&self`, synchronous, pure, and reproducible (INV-01).
#[derive(Debug, Clone)]
pub struct DiagnosisEngine {
    catalog: DiagnosisCatalog,
    tree: DecisionTree,
}

impl DiagnosisEngine {
    /// Construct from the bundled catalog + decision tree (both validated on load).
    pub fn new() -> Result<Self, RuleError> {
        let catalog = DiagnosisCatalog::bundled()?;
        let tree = DecisionTree::bundled(&catalog)?;
        Ok(Self { catalog, tree })
    }

    /// Borrow the catalog (for the API to list categories / look up a code).
    pub fn catalog(&self) -> &DiagnosisCatalog {
        &self.catalog
    }

    /// Borrow the decision tree (for the API to drive the interactive 问诊树).
    pub fn tree(&self) -> &DecisionTree {
        &self.tree
    }

    /// Run the deterministic diagnosis (prd §4.1.2). Walks the 问诊树 over `input.answer_path`;
    /// a completed path yields a determinate [`DiagnosisOutput::Ok`], an incomplete / invalid
    /// path yields [`DiagnosisOutput::OutOfScope`] (abstention, never a guess — prd §4.1.5).
    pub fn diagnose(&self, input: &DiagnosisInput) -> DiagnosisOutput {
        // ≥ 3 skipped questions → heuristic follow-up mode (prd §4.1.4): we cannot reach a
        // determinate subcategory, so abstain (INV-08 Low-confidence path).
        if input.skipped_questions >= 3 {
            return DiagnosisOutput::OutOfScope {
                coverage_tag: CoverageTag::Unknown,
                reasons: vec![format!(
                    "用户跳过 {} 项问诊（≥3），进入启发式追问，无法确定子类",
                    input.skipped_questions
                )],
                next_actions: vec![DiagnosisNextAction::ShowUiHint {
                    text: "请补全问诊以便系统精确识别争议子类".to_string(),
                }],
            };
        }

        match self.tree.walk(&input.answer_path) {
            Ok(code) => match self.catalog.get(&code) {
                Some(entry) => DiagnosisOutput::Ok(Box::new(self.build_result(input, entry))),
                // The tree validated against the catalog at load, so this is unreachable in
                // practice; abstain rather than panic (defensive, INV-01).
                None => DiagnosisOutput::OutOfScope {
                    coverage_tag: CoverageTag::Unknown,
                    reasons: vec![format!("识别到子类 {code} 但目录缺失")],
                    next_actions: vec![],
                },
            },
            Err(e) => DiagnosisOutput::OutOfScope {
                coverage_tag: CoverageTag::Unknown,
                reasons: vec![e.to_string()],
                next_actions: vec![DiagnosisNextAction::ShowUiHint {
                    text: "请继续完成问诊以确定争议子类".to_string(),
                }],
            },
        }
    }

    /// Build the determinate result for a reached catalog entry.
    fn build_result(&self, input: &DiagnosisInput, entry: &CatalogEntry) -> DiagnosisResult {
        // Coverage tag: 做深 → Exact, 做能用 → Approximate (mirrors the M9 `region_tag` policy so
        // the abstention semantics are consistent across the deterministic engines, INV-01).
        let coverage_tag = match entry.coverage_tier {
            CoverageTier::MakeDeep => CoverageTag::Exact,
            CoverageTier::MakeUsable => CoverageTag::Approximate,
        };

        // Deterministic confidence: a fully-answered path is high; each skipped question shaves a
        // fixed amount. 做深 anchors higher than 做能用. No randomness, fully reproducible.
        let base = match entry.coverage_tier {
            CoverageTier::MakeDeep => 0.95,
            CoverageTier::MakeUsable => 0.80,
        };
        let confidence = (base - 0.1 * input.skipped_questions as f32).clamp(0.0, 1.0);

        let next_actions = self.next_actions_for(entry);

        DiagnosisResult {
            identity_type: input.identity_type,
            dispute_subtype: input.dispute_subtype,
            dispute_category: entry.code.clone(),
            dispute_category_name: entry.name_zh.clone(),
            coverage_tier: entry.coverage_tier,
            confidence,
            coverage_tag,
            law_ref: entry.law_ref.clone(),
            recommended_procedures: entry.procedures.clone(),
            next_actions,
        }
    }

    /// Derive the concrete `next_actions` (prd §4.1.2: 用户下一步具体动作, 不留"建议你思考").
    fn next_actions_for(&self, entry: &CatalogEntry) -> Vec<DiagnosisNextAction> {
        let mut actions = Vec::new();
        // The main procedure is the first concrete action.
        actions.push(DiagnosisNextAction::StartProcedure {
            procedure: procedure_label(entry.procedures.main).to_string(),
            role: "main".to_string(),
        });
        for p in &entry.procedures.parallel {
            actions.push(DiagnosisNextAction::StartProcedure {
                procedure: procedure_label(*p).to_string(),
                role: "parallel".to_string(),
            });
        }
        if let Some(fb) = entry.procedures.fallback {
            actions.push(DiagnosisNextAction::StartProcedure {
                procedure: procedure_label(fb).to_string(),
                role: "fallback".to_string(),
            });
        }
        // make-usable subcategories carry a legal-aid hint (prd §4.1.5: ≥做能用 全量覆盖).
        if entry.coverage_tier == CoverageTier::MakeUsable {
            actions.push(DiagnosisNextAction::ConsultLegalAid {
                hint: "本子类为做能用覆盖，建议结合当地法律援助综合判断".to_string(),
            });
        }
        actions
    }
}

/// Chinese label for a [`crate::diagnosis::catalog::Procedure`].
fn procedure_label(p: crate::diagnosis::catalog::Procedure) -> &'static str {
    use crate::diagnosis::catalog::Procedure::*;
    match p {
        Arbitration => "劳动仲裁",
        Litigation => "法院诉讼",
        Inspection => "劳动监察投诉",
        Mediation => "调解",
        Recognition => "工伤认定",
        Appraisal => "鉴定",
        Negotiation => "协商",
        CriminalReport => "刑事报案",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> DiagnosisEngine {
        DiagnosisEngine::new().expect("engine loads")
    }

    fn input(path: &[&str]) -> DiagnosisInput {
        DiagnosisInput {
            identity_type: IdentityType::StandardFullTime,
            dispute_subtype: DisputeSubtype::SocialInsArrears,
            answer_path: path.iter().map(|s| s.to_string()).collect(),
            skipped_questions: 0,
        }
    }

    #[test]
    fn diagnoses_no_written_contract() {
        let out = engine().diagnose(&input(&["contract_formation", "no_written"]));
        match out {
            DiagnosisOutput::Ok(r) => {
                assert_eq!(r.dispute_category, "LD-01-01");
                assert_eq!(r.coverage_tier, CoverageTier::MakeDeep);
                assert_eq!(r.coverage_tag, CoverageTag::Exact);
                assert_eq!(r.law_ref, "law:中华人民共和国劳动合同法/v2012-12-28/§82");
                assert!(r.confidence > 0.9);
                assert!(!r.next_actions.is_empty());
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn diagnoses_make_usable_with_lower_confidence() {
        let out = engine().diagnose(&input(&["new_employment", "algorithm"]));
        match out {
            DiagnosisOutput::Ok(r) => {
                assert_eq!(r.dispute_category, "LD-13-02");
                assert_eq!(r.coverage_tier, CoverageTier::MakeUsable);
                assert_eq!(r.coverage_tag, CoverageTag::Approximate);
                assert!((r.confidence - 0.80).abs() < f32::EPSILON);
                // make-usable carries a legal-aid action.
                assert!(r
                    .next_actions
                    .iter()
                    .any(|a| matches!(a, DiagnosisNextAction::ConsultLegalAid { .. })));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn skips_three_or_more_abstains() {
        let mut i = input(&["contract_formation", "no_written"]);
        i.skipped_questions = 3;
        let out = engine().diagnose(&i);
        assert!(matches!(out, DiagnosisOutput::OutOfScope { coverage_tag: CoverageTag::Unknown, .. }));
    }

    #[test]
    fn incomplete_path_abstains_not_guesses() {
        let out = engine().diagnose(&input(&["work_injury", "occupational"]));
        assert!(matches!(out, DiagnosisOutput::OutOfScope { .. }));
    }

    #[test]
    fn invalid_answer_abstains() {
        let out = engine().diagnose(&input(&["contract_formation", "nope"]));
        assert!(matches!(out, DiagnosisOutput::OutOfScope { .. }));
    }

    #[test]
    fn skipped_questions_lower_confidence_deterministically() {
        let mut i = input(&["wage", "arrears"]);
        i.skipped_questions = 2;
        match engine().diagnose(&i) {
            DiagnosisOutput::Ok(r) => {
                // 做深 base 0.95 - 0.2 = 0.75.
                assert!((r.confidence - 0.75).abs() < 1e-6);
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }
}
