//! Strongly-typed region YAML schema (ai/02 §2.5, ai/06 §6.6).
//!
//! One YAML per province is the single shared data source: M9 consumes `wage` / `overtime` /
//! `injury`; M5 consumes `deadline`. The five deep provinces (11/31/32/33/44) carry full
//! `injury.*_months_by_grade` (length 10) and `deadline.injury_assessment_days`.

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::coverage::RuleLawRef;

/// A LawRef entry inside a region YAML (`urn` + display metadata). Only `urn` is load-bearing for
/// the engine; `title` / `effective_at` aid traceability.
#[derive(Debug, Clone, Deserialize)]
pub struct RegionLawRef {
    pub urn: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub effective_at: Option<chrono::NaiveDate>,
}

/// Per-city minimum wage row.
#[derive(Debug, Clone, Deserialize)]
pub struct CityWage {
    pub city_code: String,
    pub city_name_zh: String,
    pub value: Decimal,
    #[serde(default)]
    pub effective_at: Option<chrono::NaiveDate>,
}

/// Wage parameters (M9).
#[derive(Debug, Clone, Deserialize)]
pub struct WageParams {
    #[serde(default)]
    pub region_min_wage_monthly: Vec<CityWage>,
    /// 上年全省全口径月均工资 — prior-year provincial average monthly wage.
    pub avg_monthly_wage_prev_year: Decimal,
    /// 经济补偿封顶倍数 — severance cap multiplier (3 by statute §47).
    pub cap_multiplier_for_severance: u8,
}

/// Overtime parameters (M9).
#[derive(Debug, Clone, Deserialize)]
pub struct OvertimeParams {
    /// 月计薪天数 — monthly paid days divisor (21.75).
    pub hourly_divisor: Decimal,
    pub weekday_rate: Decimal,
    pub weekend_rate: Decimal,
    pub holiday_rate: Decimal,
    /// 工作日延时加班月上限小时 — monthly weekday-overtime cap (36).
    pub monthly_cap_hours: u32,
}

/// Work-injury subsidy parameters (M9). Deep provinces fill all three arrays at length 10.
#[derive(Debug, Clone, Deserialize)]
pub struct InjuryParams {
    /// 一次性伤残补助金月数 1..=10 级 — lump-sum disability subsidy months by grade.
    #[serde(default)]
    pub lump_sum_months_by_grade: Vec<u8>,
    /// 一次性工伤医疗补助金月数 — one-off medical subsidy months by grade (local).
    #[serde(default)]
    pub medical_subsidy_months_by_grade: Vec<u8>,
    /// 一次性伤残就业补助金月数 — one-off employment subsidy months by grade (local).
    #[serde(default)]
    pub employment_subsidy_months_by_grade: Vec<u8>,
}

/// Deadline parameters (M5). M9 does not consume this section.
#[derive(Debug, Clone, Deserialize)]
pub struct DeadlineParams {
    /// 仲裁一般时效（年）— general arbitration limitation (1 year).
    pub arbitration_general_years: u8,
    /// 拖欠工资特殊时效标记 — wage-arrears special-limitation marker.
    pub arbitration_wage_special: String,
    /// 监察投诉时效（年）— labor-inspection complaint limitation (2 years).
    pub inspection_years: u8,
    /// 执行申请时效（年）— enforcement application limitation (2 years).
    pub enforcement_years: u8,
    /// 工伤认定时效（年）— work-injury recognition limitation (1 year).
    #[serde(default)]
    pub injury_recognition_years: Option<u8>,
    /// 劳动能力鉴定时限（天）— labor-ability assessment window (deep provinces required).
    #[serde(default)]
    pub injury_assessment_days: Option<i64>,
    /// 上诉期（天）— appeal window (15 days nationwide).
    #[serde(default)]
    pub appeal_days: Option<u8>,
}

/// 弃权区 — abstention zone keyword + reason (drives `Boundary` coverage).
#[derive(Debug, Clone, Deserialize)]
pub struct AbstentionZone {
    pub keyword: String,
    pub reason: String,
}

/// One province's full parameter set, parsed from `data/<code>_<name>.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RegionParams {
    pub region_code: String,
    pub region_name_zh: String,
    #[serde(default)]
    pub deep_coverage: bool,
    pub data_version: chrono::NaiveDate,
    #[serde(default)]
    pub law_refs: Vec<RegionLawRef>,
    pub wage: WageParams,
    pub overtime: OvertimeParams,
    pub injury: InjuryParams,
    pub deadline: DeadlineParams,
    #[serde(default)]
    pub abstention_zones: Vec<AbstentionZone>,
}

impl RegionParams {
    /// 社平月工资 — provincial average monthly wage (severance cap base).
    pub fn avg_monthly_wage(&self) -> Decimal {
        self.wage.avg_monthly_wage_prev_year
    }

    /// Validate the region-specific LawRef URNs and surface them as [`RuleLawRef`].
    pub fn law_refs(&self) -> Result<Vec<RuleLawRef>, crate::error::RuleError> {
        self.law_refs
            .iter()
            .map(|r| RuleLawRef::parse(&r.urn))
            .collect()
    }
}
