//! 工伤三阶段流水线 — work-injury three-phase pipeline (ai/06 §6.5).
//!
//! 法源：`law:工伤保险条例/v2010-12-20/§17`（认定 1 年）+ `§25`（鉴定）。
//! 阶段 1 认定 1 年倒计时；阶段 2 鉴定在阶段 1 结论作出前调用 → `OutOfScope { Boundary }`。

use serde::{Deserialize, Serialize};

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::{effective_remaining, DeadlineState};
use crate::coverage::{CoverageTag, RuleNextAction, RuleOutcome};
use crate::error::RuleError;
use crate::region::RegionParams;

/// 工伤三阶段 — recognition → assessment → benefit payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjuryPhase {
    Recognition,
    Assessment,
    BenefitPayout,
}

/// 阶段间闸 — whether the next phase may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextPhaseGate {
    /// 前阶段已完成，可进入下一阶段。
    Open,
    /// 前阶段结论未作出，下一阶段不可起算。
    BlockedPriorPhase,
}

/// Snapshot of the injury pipeline (structured view for the API layer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InjuryPipeline {
    pub phase: InjuryPhase,
    pub recognition_deadline: DeadlineState,
    pub assessment_deadline: Option<DeadlineState>,
    pub next_phase_gate: NextPhaseGate,
}

impl InjuryPipeline {
    /// 阶段 1：工伤认定 1 年倒计时（自事故 / 诊断之日起）。
    pub fn recognition(facts: &DeadlineFacts) -> Result<RuleOutcome, RuleError> {
        let state = effective_remaining(365, facts);
        buffer::apply(state, DeadlineKind::InjuryRecognition, facts)
    }

    /// 阶段 2：劳动能力鉴定（地方规定时限）。认定结论未作出 → `OutOfScope { Boundary }`。
    pub fn assessment(
        facts: &DeadlineFacts,
        region: &RegionParams,
    ) -> Result<RuleOutcome, RuleError> {
        // 阶段间闸：阶段 1 工伤认定结论未作出 → Boundary。
        let conclusion = match facts.recognition_conclusion_at {
            Some(d) => d,
            None => {
                return Ok(RuleOutcome::OutOfScope {
                    coverage_tag: CoverageTag::Boundary,
                    reasons: vec!["工伤认定结论尚未作出，劳动能力鉴定不可起算".to_string()],
                    next_actions: vec![RuleNextAction::CollectFact {
                        field: "recognition_conclusion_at".to_string(),
                    }],
                });
            }
        };
        let days = match region.deadline.injury_assessment_days {
            Some(d) => d,
            None => {
                // make-usable province: assessment window not deeply covered → guide to legal aid.
                return Ok(RuleOutcome::OutOfScope {
                    coverage_tag: CoverageTag::Approximate,
                    reasons: vec![format!(
                        "{} 劳动能力鉴定时限未做深，请咨询当地法援",
                        region.region_name_zh
                    )],
                    next_actions: vec![RuleNextAction::ConsultLegalAid {
                        hotline: vec!["12348".to_string(), "12333".to_string()],
                    }],
                });
            }
        };
        // 鉴定时效自认定结论作出之日起算。
        let state = effective_remaining(days, &facts.with_anchor(conclusion));
        let tag = if region.deep_coverage {
            CoverageTag::Exact
        } else {
            CoverageTag::Approximate
        };
        buffer::apply_with_tag(state, DeadlineKind::InjuryAssessment, facts, tag)
    }

    /// 阶段 3：工伤待遇核付（benefit payout, ai/06 §6.5 阶段 3）。
    ///
    /// 与阶段 2「劳动能力鉴定」语义不同（评审 FIX 2）：阶段 3 **依赖鉴定结论**作出，
    /// 且待遇金额由 M9 计算引擎 `compute::injury`（ai/02 §2.4.6）核算，**不属于 M5 时效引擎
    /// 自有的倒计时窗口**——31 省 yaml 的 `deadline` 段（ai/06 §6.6）并未定义待遇核付时限。
    ///
    /// 因此本方法遵循 §6.5 阶段间闸语义：
    /// - 劳动能力鉴定结论（`assessment_conclusion_at`）未作出 → `OutOfScope { Boundary }`
    ///   + `CollectFact { assessment_conclusion_at }`（不猜测，C-C-6）。
    /// - 鉴定结论已作出 → 阶段 3 落在 M9 待遇计算，M5 无自有时效窗口，返回
    ///   `OutOfScope { Boundary }` 引导调用方走 `compute::injury`（ai/02 §2.4.6），
    ///   而非复用阶段 2 的鉴定时效窗口。
    pub fn benefit_payout(facts: &DeadlineFacts) -> Result<RuleOutcome, RuleError> {
        // 阶段间闸：前置阶段（劳动能力鉴定）结论未作出 → Boundary。
        if facts.assessment_conclusion_at.is_none() {
            return Ok(RuleOutcome::OutOfScope {
                coverage_tag: CoverageTag::Boundary,
                reasons: vec!["劳动能力鉴定结论尚未作出，工伤待遇核付不可起算".to_string()],
                next_actions: vec![RuleNextAction::CollectFact {
                    field: "assessment_conclusion_at".to_string(),
                }],
            });
        }
        // 鉴定结论已作出：待遇金额由 M9 计算引擎核算（ai/02 §2.4.6），不属于 M5 时效窗口。
        Ok(RuleOutcome::OutOfScope {
            coverage_tag: CoverageTag::Boundary,
            reasons: vec![
                "工伤待遇核付为待遇金额计算（M9 calc::injury, ai/02 §2.4.6），不属于 M5 时效窗口"
                    .to_string(),
            ],
            next_actions: vec![RuleNextAction::CollectFact {
                field: "injury_benefit_computation".to_string(),
            }],
        })
    }
}
