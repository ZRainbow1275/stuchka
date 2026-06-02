//! 履行监控 + 违约触发执行时效 — performance monitoring + breach-triggered enforcement countdown
//! (M16, prd/04 §4.10). Pure rules, zero AI / zero HTTP (INV-01).
//!
//! 给定一份履行计划（裁决书 / 判决 / 和解协议的按期付款时间表），本模块逐期计算履行状态
//! （未到期 / 按期履行 / 逾期履行 / 违约）。任一期违约 = 默认触发点：以「首个违约到期日」为锚，
//! 复用已实装的 [`super::enforcement::resolve`]（民诉法 §250 两年执行申请时效 + 10% 余量 +
//! INV-08 强制人工二次确认）产出执行时效倒计时。
//!
//! 金额一律 `rust_decimal::Decimal`，绝不使用 f64（OM-05）。所有日期差为真实日历差，零捏造：
//! 缺 `effective_date` / 计划为空 → 调用方按 `OutOfScope { Unknown, CollectFact }` 处理（C-C-6），
//! 本模块不臆造默认值。「违约自动触发提醒」的推送通道属 M12（R1b），本模块只产出真实的违约 +
//! 倒计时数据，由桌面端呈现（诚实接缝，不在此伪造通知器）。

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::anchor::{DeadlineFacts, DeadlineKind};
use crate::coverage::RuleOutcome;
use crate::error::RuleError;
use crate::region::RegionParams;

/// 履行文书类型 — the instrument whose performance schedule is being monitored (§4.10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentKind {
    /// 仲裁裁决书。
    Award,
    /// 法院判决。
    Judgment,
    /// 和解协议。
    Settlement,
}

/// 履行节点 — one scheduled payment node (§4.10.1 按期付款时间表).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceInstallment {
    /// 应付款日。
    pub due_on: NaiveDate,
    /// 应付金额（Decimal，绝不 f64）。
    pub amount: Decimal,
    /// 实际付款日（未付为 `None`）。
    #[serde(default)]
    pub paid_on: Option<NaiveDate>,
    /// 实际付款金额（未付 / 部分付款 → `None` / 小于 `amount`）。
    #[serde(default)]
    pub paid_amount: Option<Decimal>,
}

/// 单期履行状态 — purely derived from dates + Decimal amounts (no AI, no float).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InstallmentState {
    /// 未到期且未付款（`as_of < due_on`，尚无付款）。
    NotYetDue,
    /// 按期足额履行（付款日 ≤ 应付日，且付款金额 ≥ 应付金额）。
    PaidOnTime,
    /// 逾期但已足额履行（付款金额 ≥ 应付，但付款日 > 应付日）。
    PaidLate { days_late: i64 },
    /// 违约：到期未付 或 部分付款（缺口 = 应付 − 已付）。
    Overdue { overdue_days: i64, shortfall: Decimal },
}

/// 单期评估结果 — the installment plus its derived state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallmentAssessment {
    pub due_on: NaiveDate,
    pub amount: Decimal,
    pub paid_on: Option<NaiveDate>,
    pub paid_amount: Option<Decimal>,
    pub state: InstallmentState,
}

/// 履行监控输入 — the full performance schedule + the evaluation date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceFacts {
    pub instrument_kind: InstrumentKind,
    /// 文书生效日（履行期起算）。
    pub effective_date: NaiveDate,
    pub installments: Vec<PerformanceInstallment>,
    pub as_of: NaiveDate,
}

/// 履行监控输出 — per-installment timeline + the breach-triggered enforcement countdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceStatus {
    pub installments: Vec<InstallmentAssessment>,
    pub total_amount: Decimal,
    pub total_paid: Decimal,
    /// 累计违约缺口（所有 `Overdue` 期的 shortfall 之和）。
    pub total_shortfall: Decimal,
    /// 首个违约到期日（默认触发点）；无违约时为 `None`。
    pub first_breach_at: Option<NaiveDate>,
    /// 执行申请时效倒计时：仅当存在违约期时为 `Some`（以首个违约到期日为锚复用 §250 引擎），
    /// 无违约则为 `None`（不触发执行）。
    pub enforcement: Option<RuleOutcome>,
}

/// Assess one installment against `as_of`. Every branch is a real date / Decimal comparison.
fn assess(inst: &PerformanceInstallment, as_of: NaiveDate) -> InstallmentState {
    let paid_amount = inst.paid_amount.unwrap_or(Decimal::ZERO);
    let fully_paid = inst.paid_on.is_some() && paid_amount >= inst.amount;
    if fully_paid {
        let paid_on = inst.paid_on.expect("fully_paid implies paid_on is Some");
        let days_late = (paid_on - inst.due_on).num_days();
        if days_late <= 0 {
            InstallmentState::PaidOnTime
        } else {
            InstallmentState::PaidLate { days_late }
        }
    } else {
        // Not fully paid. If still before the due date and nothing paid → not yet due.
        if inst.paid_on.is_none() && as_of < inst.due_on {
            return InstallmentState::NotYetDue;
        }
        // Partial payment, or due date reached without full payment → breach (default-trigger).
        let overdue_days = (as_of - inst.due_on).num_days().max(0);
        let shortfall = inst.amount - paid_amount;
        InstallmentState::Overdue {
            overdue_days,
            shortfall,
        }
    }
}

/// Evaluate the performance schedule: per-installment state + (on any breach) the enforcement
/// 2-year countdown reanchored at the first breach due date (§250). Pure — no AI, no network.
///
/// `effective_date` and a non-empty schedule are required by the caller (C-C-6: a missing schedule
/// must surface as `OutOfScope { Unknown, CollectFact }` at the boundary, never a fabricated 0).
pub fn evaluate(
    facts: &PerformanceFacts,
    region: &RegionParams,
) -> Result<PerformanceStatus, RuleError> {
    let mut assessments = Vec::with_capacity(facts.installments.len());
    let mut total_amount = Decimal::ZERO;
    let mut total_paid = Decimal::ZERO;
    let mut total_shortfall = Decimal::ZERO;
    let mut first_breach_at: Option<NaiveDate> = None;

    for inst in &facts.installments {
        let state = assess(inst, facts.as_of);
        total_amount += inst.amount;
        total_paid += inst.paid_amount.unwrap_or(Decimal::ZERO);
        if let InstallmentState::Overdue { shortfall, .. } = state {
            total_shortfall += shortfall;
            // First breach = earliest overdue due date (the default-trigger anchor).
            first_breach_at = Some(match first_breach_at {
                Some(prev) => prev.min(inst.due_on),
                None => inst.due_on,
            });
        }
        assessments.push(InstallmentAssessment {
            due_on: inst.due_on,
            amount: inst.amount,
            paid_on: inst.paid_on,
            paid_amount: inst.paid_amount,
            state,
        });
    }

    // On a breach, reuse the REAL §250 enforcement engine anchored at the first breach due date.
    // The enforcement period runs from the (last day of the) performance period; the first breach
    // due date is the conservative default-trigger anchor (risk-aware, documented).
    let enforcement = match first_breach_at {
        Some(breach_at) => {
            let ef = DeadlineFacts {
                case_occurred_at: breach_at,
                labor_relationship_active: false,
                labor_relationship_ended_at: None,
                interrupt_events: Vec::new(),
                suspend_intervals: Vec::new(),
                as_of: facts.as_of,
                recognition_conclusion_at: None,
                assessment_conclusion_at: None,
            };
            Some(super::enforcement::resolve(&ef, region)?)
        }
        None => None,
    };

    let _ = DeadlineKind::Enforcement; // documents the kind the enforcement outcome carries.
    Ok(PerformanceStatus {
        installments: assessments,
        total_amount,
        total_paid,
        total_shortfall,
        first_breach_at,
        enforcement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::ComputedValue;
    use crate::deadline::interrupt::DeadlineState;
    use crate::region::{RegionParams, RegionTable};
    use rust_decimal::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn region() -> RegionParams {
        RegionTable::load_embedded().unwrap().get("44").unwrap().clone()
    }

    fn facts(installments: Vec<PerformanceInstallment>, as_of: NaiveDate) -> PerformanceFacts {
        PerformanceFacts {
            instrument_kind: InstrumentKind::Award,
            effective_date: d(2024, 1, 1),
            installments,
            as_of,
        }
    }

    #[test]
    fn all_on_time_has_no_enforcement_countdown() {
        let f = facts(
            vec![
                PerformanceInstallment {
                    due_on: d(2024, 2, 1),
                    amount: dec!(5000),
                    paid_on: Some(d(2024, 1, 30)),
                    paid_amount: Some(dec!(5000)),
                },
                PerformanceInstallment {
                    due_on: d(2024, 3, 1),
                    amount: dec!(5000),
                    paid_on: Some(d(2024, 3, 1)),
                    paid_amount: Some(dec!(5000)),
                },
            ],
            d(2024, 3, 2),
        );
        let s = evaluate(&f, &region()).unwrap();
        assert_eq!(s.installments[0].state, InstallmentState::PaidOnTime);
        assert_eq!(s.installments[1].state, InstallmentState::PaidOnTime);
        assert!(s.first_breach_at.is_none());
        assert!(s.enforcement.is_none(), "no breach -> no enforcement clock");
        assert_eq!(s.total_shortfall, Decimal::ZERO);
    }

    #[test]
    fn one_overdue_triggers_enforcement_anchored_at_breach() {
        let f = facts(
            vec![
                PerformanceInstallment {
                    due_on: d(2024, 2, 1),
                    amount: dec!(5000),
                    paid_on: Some(d(2024, 2, 1)),
                    paid_amount: Some(dec!(5000)),
                },
                PerformanceInstallment {
                    due_on: d(2024, 3, 1),
                    amount: dec!(5000),
                    paid_on: None,
                    paid_amount: None,
                },
            ],
            d(2024, 4, 1),
        );
        let s = evaluate(&f, &region()).unwrap();
        assert_eq!(s.installments[0].state, InstallmentState::PaidOnTime);
        assert_eq!(s.first_breach_at, Some(d(2024, 3, 1)));
        match s.installments[1].state {
            InstallmentState::Overdue { overdue_days, shortfall } => {
                assert_eq!(overdue_days, 31); // 2024-03-01 -> 2024-04-01
                assert_eq!(shortfall, dec!(5000));
            }
            ref other => panic!("expected Overdue, got {other:?}"),
        }
        // Enforcement: real §250 engine, anchored at the breach (2024-03-01), 2yr window.
        let ef = s.enforcement.expect("breach must produce an enforcement countdown");
        match ef {
            RuleOutcome::Ok { value: ComputedValue::Deadline(v), .. } => {
                assert!(v.manual_confirm_required, "INV-08 always true");
                assert_eq!(v.kind, DeadlineKind::Enforcement);
                // raw = 2*365 - (2024-04-01 - 2024-03-01) = 730 - 31 = 699; buffered = floor(699*0.9).
                assert_eq!(v.raw_remaining_days, 699);
                assert_eq!(v.buffered_remaining_days, (699 * 9) / 10);
                assert!(matches!(v.state, DeadlineState::Running { .. }));
            }
            other => panic!("expected Ok deadline, got {other:?}"),
        }
    }

    #[test]
    fn paid_late_but_full_is_not_a_breach() {
        let f = facts(
            vec![PerformanceInstallment {
                due_on: d(2024, 2, 1),
                amount: dec!(5000),
                paid_on: Some(d(2024, 2, 20)),
                paid_amount: Some(dec!(5000)),
            }],
            d(2024, 3, 1),
        );
        let s = evaluate(&f, &region()).unwrap();
        assert_eq!(s.installments[0].state, InstallmentState::PaidLate { days_late: 19 });
        assert!(s.first_breach_at.is_none());
        assert!(s.enforcement.is_none());
    }

    #[test]
    fn partial_payment_is_overdue_with_exact_shortfall() {
        let f = facts(
            vec![PerformanceInstallment {
                due_on: d(2024, 2, 1),
                amount: dec!(5000),
                paid_on: Some(d(2024, 2, 1)),
                paid_amount: Some(dec!(3000.50)),
            }],
            d(2024, 3, 1),
        );
        let s = evaluate(&f, &region()).unwrap();
        match s.installments[0].state {
            InstallmentState::Overdue { shortfall, .. } => assert_eq!(shortfall, dec!(1999.50)),
            ref other => panic!("expected Overdue, got {other:?}"),
        }
        assert_eq!(s.total_shortfall, dec!(1999.50));
        assert!(s.enforcement.is_some());
    }

    #[test]
    fn future_installment_is_not_yet_due() {
        let f = facts(
            vec![PerformanceInstallment {
                due_on: d(2024, 6, 1),
                amount: dec!(5000),
                paid_on: None,
                paid_amount: None,
            }],
            d(2024, 3, 1),
        );
        let s = evaluate(&f, &region()).unwrap();
        assert_eq!(s.installments[0].state, InstallmentState::NotYetDue);
        assert!(s.enforcement.is_none());
    }

    #[test]
    fn breach_more_than_two_years_ago_is_expired() {
        let f = facts(
            vec![PerformanceInstallment {
                due_on: d(2021, 1, 1),
                amount: dec!(5000),
                paid_on: None,
                paid_amount: None,
            }],
            d(2024, 1, 1),
        );
        let s = evaluate(&f, &region()).unwrap();
        let ef = s.enforcement.unwrap();
        match ef {
            RuleOutcome::Ok { value: ComputedValue::Deadline(v), .. } => {
                assert!(matches!(v.state, DeadlineState::Expired { .. }));
            }
            other => panic!("expected Ok deadline, got {other:?}"),
        }
    }
}
