//! `FactBundle` — the M9 compute input (ai/02 §2.4). Missing facts are `Option`/empty and never
//! defaulted (C-C-6); the compute functions translate absence into `OutOfScope`.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::compute::injury::InjuryInputs;

/// One monthly wage record from the 12 months preceding termination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WageRecord {
    /// First day of the wage month (day-precise).
    pub month: NaiveDate,
    /// Gross monthly wage for that month.
    pub gross: Decimal,
}

impl WageRecord {
    pub fn new(month: NaiveDate, gross: Decimal) -> Self {
        Self { month, gross }
    }
}

/// All facts an M9 computation may consume. The `case_occurred_at` anchor and the
/// interrupt / suspend timeline live in `deadline::DeadlineFacts` (M5-specific), not here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FactBundle {
    /// 入职日 — hire date. Absent → `OutOfScope` (no guessing, C-C-6).
    pub start_date: Option<NaiveDate>,
    /// 解除日 — termination date.
    pub end_date: Option<NaiveDate>,
    /// 前 12 个月工资明细 — preceding-12-months wage detail (for average-wage base).
    #[serde(default)]
    pub wage_records: Vec<WageRecord>,
    /// Monthly wage shortcut (used by overtime / double-pay when no 12-month detail is supplied).
    pub monthly_wage: Option<Decimal>,
    /// 补签 / 首次签订书面合同之日 — date the written contract was signed (double-wage cut-off).
    pub contract_signed_at: Option<NaiveDate>,
    /// 工作日延时加班小时 — weekday extended overtime hours.
    #[serde(default)]
    pub weekday_ot_hours: Decimal,
    /// 休息日加班小时 — rest-day overtime hours.
    #[serde(default)]
    pub weekend_ot_hours: Decimal,
    /// 法定节假日加班小时 — statutory-holiday overtime hours.
    #[serde(default)]
    pub holiday_ot_hours: Decimal,
    /// 劳动监察责令限期支付仍逾期 — inspection order issued and still overdue (50% surcharge gate).
    #[serde(default)]
    pub has_inspection_order_overdue: bool,
    /// 拖欠金额 — overdue wage / compensation principal for the 50% surcharge base.
    #[serde(default)]
    pub overdue_wage_amount: Decimal,
    /// 工伤输入 — work-injury benefit inputs (None → injury compute is out of scope).
    pub injury: Option<InjuryInputs>,
}
