//! X 阈值 — L0-04 block point (ai/02 §2.3, D11).
//!
//! `compute_x_threshold` stays `unimplemented!` (the Q12 formula was vetoed by C-B-4 / 法律 P4 and
//! the lawyer rewrite under L0-04 has not landed). The public path NEVER calls the panicking body:
//! `try_resolve` (and [`resolve_x_threshold`]) normalise to `OutOfScope { Unknown }` plus a UI hint
//! stating the threshold has no legal binding force (D11 — the two clauses coexist, no conflict).

use rust_decimal::Decimal;

use crate::coverage::{CoverageTag, RuleNextAction, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;

/// L0-04 block: the determinate X-threshold formula is not delivered. Body stays `unimplemented!`.
/// Callers MUST route through [`resolve_x_threshold`] instead of invoking this.
pub fn compute_x_threshold(
    _monthly_wage: Decimal,
    _region_avg_wage: Decimal,
) -> Result<RuleOutcome, RuleError> {
    // Q12 原公式 X = min(社平 3 倍 × 1 月, 最低工资 × 12) 已被 C-B-4 / 法律 P4 否决。
    // L0-04 律师重写法源未交付；函数体保持阻断标记。
    unimplemented!("blocked by L0-04")
}

/// Public normalisation: always `OutOfScope { Unknown }` with the D11 disclaimer hint. Never
/// touches the panicking `compute_x_threshold` body, so the engine never panics outward.
pub fn resolve_x_threshold(_facts: &FactBundle) -> RuleOutcome {
    RuleOutcome::OutOfScope {
        coverage_tag: CoverageTag::Unknown,
        reasons: vec![
            "Q12 X 阈值公式无法源支撑，已废弃".to_string(),
            "L0-04 法律顾问重写工作未交付，无法输出确定性数值".to_string(),
        ],
        next_actions: vec![RuleNextAction::ShowUiHint {
            text: "本阈值为系统建议参考，无法律强制约束力".to_string(),
        }],
    }
}
