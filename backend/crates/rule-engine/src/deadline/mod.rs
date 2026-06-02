//! M5 时效引擎 — deadline engine (ai/06). Pure rules, zero AI / zero HTTP (INV-01).
//!
//! 起算锚 + 中断 / 中止状态机 + 拖欠工资特殊时效 + 工伤三阶段 + 职业病独立起算 + 10% buffer +
//! INV-08 强制人工二次确认 + 三级预警纯规则部分。

pub mod anchor;
pub mod appeal;
pub mod arbitration;
pub mod buffer;
pub mod enforcement;
pub mod injury_phases;
pub mod inspection;
pub mod interrupt;
pub mod occupational_disease;
pub mod performance;
pub mod warning;

pub use anchor::{DeadlineFacts, DeadlineKind};
pub use buffer::DeadlineValue;
pub use injury_phases::{InjuryPhase, InjuryPipeline, NextPhaseGate};
pub use performance::{
    InstallmentAssessment, InstallmentState, InstrumentKind, PerformanceFacts,
    PerformanceInstallment, PerformanceStatus,
};
pub use interrupt::{
    effective_remaining, DeadlineState, InterruptCause, InterruptEvent, SuspendCause,
    SuspendInterval,
};
pub use warning::{tier_for, WarningTier};

use crate::coverage::RuleOutcome;
use crate::error::RuleError;
use crate::region::RegionParams;

/// Dispatch a deadline request by [`DeadlineKind`] to the matching resolver (ai/06 §6.2).
pub fn resolve(
    kind: DeadlineKind,
    facts: &DeadlineFacts,
    region: &RegionParams,
) -> Result<RuleOutcome, RuleError> {
    match kind {
        DeadlineKind::ArbitrationGeneral => arbitration::general(facts),
        DeadlineKind::ArbitrationWage => arbitration::wage_arbitration(facts),
        DeadlineKind::Inspection => inspection::resolve(facts, region),
        DeadlineKind::AppealFirstInstance => appeal::resolve(facts, region, false),
        DeadlineKind::AppealSecondInstance => appeal::resolve(facts, region, true),
        DeadlineKind::Enforcement => enforcement::resolve(facts, region),
        DeadlineKind::InjuryRecognition => InjuryPipeline::recognition(facts),
        DeadlineKind::InjuryAssessment => InjuryPipeline::assessment(facts, region),
        // FIX 2 (ai/06 §6.5): benefit payout (阶段 3) is distinct from the assessment phase —
        // it depends on the assessment conclusion and the amount is computed by M9 calc::injury,
        // not an M5 deadline window. Route to its own resolver rather than reusing `assessment`.
        DeadlineKind::InjuryBenefitPayout => InjuryPipeline::benefit_payout(facts),
        DeadlineKind::OccupationalDisease => occupational_disease::recognition(facts),
    }
}
