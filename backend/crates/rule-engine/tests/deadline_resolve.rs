//! M5 时效单元测试 (ai/06 §6.10 → DL-01..08). >= 20 cases.

use chrono::NaiveDate;

use rule_engine::coverage::{ComputedValue, CoverageTag, RuleNextAction, RuleOutcome};
use rule_engine::deadline::{
    self, arbitration, buffer, effective_remaining, injury_phases::InjuryPipeline,
    occupational_disease, tier_for, warning::WarningTier, DeadlineFacts, DeadlineKind,
    DeadlineState, DeadlineValue, InterruptCause, InterruptEvent, SuspendCause, SuspendInterval,
};
use rule_engine::region::RegionTable;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn gd() -> rule_engine::region::RegionParams {
    RegionTable::load_embedded()
        .unwrap()
        .get("44")
        .unwrap()
        .clone()
}

fn deadline_value(o: &RuleOutcome) -> DeadlineValue {
    match o {
        RuleOutcome::Ok {
            value: ComputedValue::Deadline(v),
            ..
        } => v.clone(),
        other => panic!("expected deadline Ok, got {other:?}"),
    }
}

fn base_facts(occurred: NaiveDate, as_of: NaiveDate) -> DeadlineFacts {
    DeadlineFacts {
        case_occurred_at: occurred,
        labor_relationship_active: false,
        labor_relationship_ended_at: None,
        interrupt_events: vec![],
        suspend_intervals: vec![],
        as_of,
        recognition_conclusion_at: None,
        assessment_conclusion_at: None,
    }
}

// ───────────────────────────── effective_remaining state machine (DL-02) ─────────────────────────────

#[test]
fn dl02_01_running() {
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let s = effective_remaining(365, &f);
    assert_eq!(
        s,
        DeadlineState::Running {
            remaining_days: 151
        }
    );
}

#[test]
fn dl02_02_expired() {
    let f = base_facts(d(2023, 1, 1), d(2025, 1, 1));
    let s = effective_remaining(365, &f);
    assert!(matches!(s, DeadlineState::Expired { .. }));
}

#[test]
fn dl02_03_single_interrupt_resets() {
    // 起算 2024-01-01，中断 2024-10-01 → 重新起算；as_of 2025-01-01 → 365-92=273.
    let mut f = base_facts(d(2024, 1, 1), d(2025, 1, 1));
    f.interrupt_events = vec![InterruptEvent {
        at: d(2024, 10, 1),
        cause: InterruptCause::ApplyMediation,
    }];
    let s = effective_remaining(365, &f);
    assert_eq!(
        s,
        DeadlineState::Running {
            remaining_days: 273
        }
    );
}

#[test]
fn dl02_04_multiple_interrupts_take_last() {
    // 多次中断取最后一次 (2024-11-01)。
    let mut f = base_facts(d(2024, 1, 1), d(2025, 1, 1));
    f.interrupt_events = vec![
        InterruptEvent {
            at: d(2024, 5, 1),
            cause: InterruptCause::AssertRightToParty,
        },
        InterruptEvent {
            at: d(2024, 11, 1),
            cause: InterruptCause::ApplyArbitration,
        },
    ];
    let s = effective_remaining(365, &f);
    // 2024-11-01 → 2025-01-01 = 61 天，365-61=304.
    assert_eq!(
        s,
        DeadlineState::Running {
            remaining_days: 304
        }
    );
}

#[test]
fn dl02_05_suspend_freezes_when_open() {
    // 中止区间未闭合 → Suspended.
    let mut f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    f.suspend_intervals = vec![SuspendInterval {
        from: d(2024, 9, 1),
        to: None,
        cause: SuspendCause::ForceMajeure,
    }];
    let s = effective_remaining(365, &f);
    assert!(matches!(s, DeadlineState::Suspended { .. }));
}

#[test]
fn dl02_06_suspend_closed_deducts_interval() {
    // 起算 2024-06-01，中止 2024-08-01..2024-09-01 (31 天)，as_of 2025-01-01.
    // elapsed = 214-31 = 183; remaining = 365-183 = 182.
    let mut f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    f.suspend_intervals = vec![SuspendInterval {
        from: d(2024, 8, 1),
        to: Some(d(2024, 9, 1)),
        cause: SuspendCause::OtherJustifiedObstacle,
    }];
    let s = effective_remaining(365, &f);
    assert_eq!(
        s,
        DeadlineState::Running {
            remaining_days: 182
        }
    );
}

// ───────────────────────────── buffer (DL-05) + INV-08 (DL-01) ─────────────────────────────

#[test]
fn dl05_07_buffer_floor_151_to_135() {
    // 29_buffer_10pct_floor: raw 151 → buffered 135.
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let state = effective_remaining(365, &f);
    let o = buffer::apply(state, DeadlineKind::ArbitrationGeneral, &f).unwrap();
    let v = deadline_value(&o);
    assert_eq!(v.raw_remaining_days, 151);
    assert_eq!(v.buffered_remaining_days, 135);
    assert!(v.manual_confirm_required, "INV-08");
    assert_eq!(o.coverage_tag(), CoverageTag::Exact);
}

#[test]
fn dl01_08_every_ok_requires_manual_confirm() {
    // DL-01: 任意 Ok 时效输出 manual_confirm_required == true.
    let region = gd();
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    for kind in [
        DeadlineKind::ArbitrationGeneral,
        DeadlineKind::Inspection,
        DeadlineKind::Enforcement,
        DeadlineKind::AppealFirstInstance,
        DeadlineKind::InjuryRecognition,
        DeadlineKind::OccupationalDisease,
    ] {
        let o = deadline::resolve(kind, &f, &region).unwrap();
        if let RuleOutcome::Ok {
            value: ComputedValue::Deadline(v),
            ..
        } = &o
        {
            assert!(
                v.manual_confirm_required,
                "{kind:?} must require manual confirm"
            );
        }
    }
}

#[test]
fn dl05_09_buffer_floor_general_property() {
    // buffered == floor(raw*0.9) for several raw values.
    for (window, occurred, as_of, expect_raw, expect_buf) in [
        (365, d(2024, 6, 1), d(2025, 1, 1), 151, 135),
        (730, d(2024, 1, 1), d(2025, 1, 1), 364, 327),
    ] {
        let f = base_facts(occurred, as_of);
        let state = effective_remaining(window, &f);
        let o = buffer::apply(state, DeadlineKind::ArbitrationGeneral, &f).unwrap();
        let v = deadline_value(&o);
        assert_eq!(v.raw_remaining_days, expect_raw);
        assert_eq!(v.buffered_remaining_days, expect_buf);
    }
}

// ───────────────────────────── wage arrears special (DL-04) ─────────────────────────────

#[test]
fn dl04_10_wage_active_no_limit_exact() {
    let mut f = base_facts(d(2020, 1, 1), d(2025, 1, 1));
    f.labor_relationship_active = true;
    let o = arbitration::wage_arbitration(&f).unwrap();
    assert_eq!(o.coverage_tag(), CoverageTag::Exact);
    let v = deadline_value(&o);
    assert_eq!(v.raw_remaining_days, i64::MAX, "no countdown while active");
    assert!(v.manual_confirm_required);
}

#[test]
fn dl04_11_wage_ended_one_year() {
    // 终止 2024-06-01，as_of 2025-01-01 → 自终止起 1 年内，剩余 151.
    let mut f = base_facts(d(2020, 1, 1), d(2025, 1, 1));
    f.labor_relationship_active = false;
    f.labor_relationship_ended_at = Some(d(2024, 6, 1));
    let o = arbitration::wage_arbitration(&f).unwrap();
    let v = deadline_value(&o);
    assert_eq!(v.raw_remaining_days, 151);
}

#[test]
fn dl04_12_wage_ended_expired() {
    let mut f = base_facts(d(2020, 1, 1), d(2025, 1, 1));
    f.labor_relationship_active = false;
    f.labor_relationship_ended_at = Some(d(2022, 1, 1));
    let o = arbitration::wage_arbitration(&f).unwrap();
    let v = deadline_value(&o);
    assert!(matches!(v.state, DeadlineState::Expired { .. }));
}

#[test]
fn dl04_13_wage_ended_missing_date_abstains() {
    let mut f = base_facts(d(2020, 1, 1), d(2025, 1, 1));
    f.labor_relationship_active = false;
    f.labor_relationship_ended_at = None;
    let o = arbitration::wage_arbitration(&f).unwrap();
    assert!(!o.is_ok());
}

// ───────────────────────────── injury three phases (DL-03) ─────────────────────────────

#[test]
fn dl03_14_injury_recognition_1year() {
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let o = InjuryPipeline::recognition(&f).unwrap();
    let v = deadline_value(&o);
    assert_eq!(v.kind, DeadlineKind::InjuryRecognition);
    assert_eq!(v.raw_remaining_days, 151);
}

#[test]
fn dl03_15_injury_assessment_gate_blocks_before_recognition() {
    // 阶段闸：认定结论未作出 → OutOfScope{Boundary}.
    let region = gd();
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let o = InjuryPipeline::assessment(&f, &region).unwrap();
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Boundary);
            assert!(matches!(
                next_actions.first(),
                Some(RuleNextAction::CollectFact { field }) if field == "recognition_conclusion_at"
            ));
        }
        _ => panic!("expected Boundary gate"),
    }
}

#[test]
fn dl03_16_injury_assessment_runs_after_recognition() {
    let region = gd();
    let mut f = base_facts(d(2024, 6, 1), d(2024, 12, 1));
    f.recognition_conclusion_at = Some(d(2024, 11, 1));
    let o = InjuryPipeline::assessment(&f, &region).unwrap();
    let v = deadline_value(&o);
    assert_eq!(v.kind, DeadlineKind::InjuryAssessment);
    // window 60 days from 2024-11-01, as_of 2024-12-01 → elapsed 30 → remaining 30.
    assert_eq!(v.raw_remaining_days, 30);
}

#[test]
fn dl03_17_injury_assessment_make_usable_abstains() {
    // make-usable 省份未填 injury_assessment_days → Approximate OutOfScope + legal aid.
    let table = RegionTable::load_embedded().unwrap();
    let henan = table.get("41").unwrap();
    let mut f = base_facts(d(2024, 6, 1), d(2024, 12, 1));
    f.recognition_conclusion_at = Some(d(2024, 11, 1));
    let o = InjuryPipeline::assessment(&f, henan).unwrap();
    assert_eq!(o.coverage_tag(), CoverageTag::Approximate);
    assert!(!o.is_ok());
}

#[test]
fn dl03_18_occupational_disease_independent_start() {
    // 职业病自诊断日起算，独立 DeadlineKind。
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let o = occupational_disease::recognition(&f).unwrap();
    let v = deadline_value(&o);
    assert_eq!(v.kind, DeadlineKind::OccupationalDisease);
    assert_eq!(v.raw_remaining_days, 151);
}

// ───────────────── injury benefit payout — phase 3 (FIX 2, ai/06 §6.5) ─────────────────

#[test]
fn dl03_18a_benefit_payout_gate_blocks_before_assessment_conclusion() {
    // 阶段 3 待遇核付：劳动能力鉴定结论未作出 → OutOfScope{Boundary} +
    // CollectFact{assessment_conclusion_at}（不复用阶段 2 鉴定时效，FIX 2）。
    let region = gd();
    let f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    let o = deadline::resolve(DeadlineKind::InjuryBenefitPayout, &f, &region).unwrap();
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Boundary);
            assert!(matches!(
                next_actions.first(),
                Some(RuleNextAction::CollectFact { field }) if field == "assessment_conclusion_at"
            ));
        }
        _ => panic!("expected Boundary gate for benefit payout"),
    }
}

#[test]
fn dl03_18b_benefit_payout_routes_to_m9_after_assessment_conclusion() {
    // 鉴定结论已作出：阶段 3 待遇金额由 M9 calc::injury 核算（ai/02 §2.4.6），
    // M5 无自有时效窗口 → OutOfScope{Boundary} 引导走 M9，绝不复用阶段 2 鉴定窗口（FIX 2）。
    let region = gd();
    let mut f = base_facts(d(2024, 6, 1), d(2025, 1, 1));
    f.recognition_conclusion_at = Some(d(2024, 9, 1));
    f.assessment_conclusion_at = Some(d(2024, 11, 1));
    let o = deadline::resolve(DeadlineKind::InjuryBenefitPayout, &f, &region).unwrap();
    // It must NOT be a Deadline Ok reusing the assessment window.
    assert!(
        !o.is_ok(),
        "benefit payout must not produce an assessment-window deadline (FIX 2)"
    );
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Boundary);
            assert!(matches!(
                next_actions.first(),
                Some(RuleNextAction::CollectFact { field }) if field == "injury_benefit_computation"
            ));
        }
        other => panic!("expected Boundary handoff to M9, got {other:?}"),
    }
}

// ───────────────────────────── missing anchor (DL-06) ─────────────────────────────

#[test]
fn dl06_19_missing_anchor_via_engine_abstains() {
    use rule_engine::{RuleEngine, RuleIntent, RuleRequest};
    let engine = RuleEngine::new().unwrap();
    let req = RuleRequest {
        intent: RuleIntent::Deadline(DeadlineKind::ArbitrationGeneral),
        facts: Default::default(),
        deadline_facts: None, // 缺起算点
        province: "44".to_string(),
        city: String::new(),
        severance_pre_tax: None,
    };
    let o = engine.try_resolve(&req).expect("some outcome");
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Unknown);
            assert!(matches!(
                next_actions.first(),
                Some(RuleNextAction::CollectFact { field }) if field == "case_occurred_at"
            ));
        }
        _ => panic!("expected OutOfScope for missing anchor"),
    }
}

// ───────────────────────────── three-tier warning ─────────────────────────────

#[test]
fn dl_20_warning_tiers() {
    assert_eq!(tier_for(1), Some(WarningTier::T1Day));
    assert_eq!(tier_for(0), Some(WarningTier::T1Day));
    assert_eq!(tier_for(3), Some(WarningTier::T3Days));
    assert_eq!(tier_for(2), Some(WarningTier::T3Days));
    assert_eq!(tier_for(10), Some(WarningTier::T10Days));
    assert_eq!(tier_for(5), Some(WarningTier::T10Days));
    assert_eq!(tier_for(11), None);
    assert_eq!(tier_for(100), None);
}

#[test]
fn dl_21_engine_deadline_general_via_facade() {
    use rule_engine::{RuleEngine, RuleIntent, RuleRequest};
    let engine = RuleEngine::new().unwrap();
    let req = RuleRequest {
        intent: RuleIntent::Deadline(DeadlineKind::ArbitrationGeneral),
        facts: Default::default(),
        deadline_facts: Some(base_facts(d(2024, 6, 1), d(2025, 1, 1))),
        province: "44".to_string(),
        city: String::new(),
        severance_pre_tax: None,
    };
    let o = engine.try_resolve(&req).expect("some outcome");
    let v = deadline_value(&o);
    assert_eq!(v.buffered_remaining_days, 135);
}
