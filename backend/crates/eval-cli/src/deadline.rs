//! deadline-30 gate (ai/05 §5.3 + ai/06 §6.9): M5 时效正确率, 100% 错一道不发版.
//!
//! Drives the REAL `rule_engine::RuleEngine::try_resolve` for intent `Deadline(kind)` and compares
//! the produced `DeadlineValue` (raw/buffered remaining days + state) or out-of-scope phase gate
//! against the calendar-derived expectation (mirroring `rule-engine/tests/deadline_resolve.rs`).

use rule_engine::deadline::{DeadlineFacts, DeadlineKind, DeadlineState, DeadlineValue};
use rule_engine::{ComputedValue, RuleEngine, RuleIntent, RuleNextAction, RuleOutcome, RuleRequest};
use serde::Deserialize;

use crate::common::{coverage_tag_str, read_json};
use crate::report::{CaseResult, GateReport};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Expect {
    Deadline {
        #[serde(default)]
        state: Option<String>,
        #[serde(default)]
        raw_remaining_days: Option<i64>,
        #[serde(default)]
        buffered_remaining_days: Option<i64>,
        #[serde(default)]
        coverage_tag: Option<String>,
    },
    OutOfScope {
        #[serde(default)]
        coverage_tag: Option<String>,
        #[serde(default)]
        next_action_field: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct DeadlineCase {
    id: String,
    /// snake_case DeadlineKind, e.g. "arbitration_general".
    kind: DeadlineKind,
    province: String,
    #[serde(default)]
    city: String,
    deadline_facts: DeadlineFacts,
    expect: Expect,
}

#[derive(Debug, Deserialize)]
struct DeadlineDataset {
    cases: Vec<DeadlineCase>,
}

fn state_str(s: &DeadlineState) -> &'static str {
    match s {
        DeadlineState::Running { .. } => "running",
        DeadlineState::Suspended { .. } => "suspended",
        DeadlineState::Expired { .. } => "expired",
    }
}

fn evaluate(case: &DeadlineCase, outcome: &RuleOutcome) -> CaseResult {
    match (&case.expect, outcome) {
        (
            Expect::Deadline {
                state,
                raw_remaining_days,
                buffered_remaining_days,
                coverage_tag,
            },
            RuleOutcome::Ok {
                value: ComputedValue::Deadline(dv),
                coverage_tag: got_tag,
                ..
            },
        ) => {
            let DeadlineValue {
                raw_remaining_days: got_raw,
                buffered_remaining_days: got_buf,
                state: got_state,
                ..
            } = dv;
            let state_ok = state.as_deref().is_none_or(|s| state_str(got_state) == s);
            let raw_ok = raw_remaining_days.is_none_or(|r| *got_raw == r);
            let buf_ok = buffered_remaining_days.is_none_or(|b| *got_buf == b);
            let tag_ok = coverage_tag
                .as_deref()
                .is_none_or(|t| coverage_tag_str(*got_tag) == t);
            let pass = state_ok && raw_ok && buf_ok && tag_ok;
            CaseResult::new(
                &case.id,
                pass,
                format!("deadline {state:?} raw={raw_remaining_days:?} buf={buffered_remaining_days:?}"),
                format!(
                    "deadline {} raw={got_raw} buf={got_buf} ({})",
                    state_str(got_state),
                    coverage_tag_str(*got_tag)
                ),
            )
        }
        (
            Expect::OutOfScope {
                coverage_tag,
                next_action_field,
            },
            RuleOutcome::OutOfScope {
                coverage_tag: got_tag,
                next_actions,
                ..
            },
        ) => {
            let tag_ok = coverage_tag
                .as_deref()
                .is_none_or(|t| coverage_tag_str(*got_tag) == t);
            let field_ok = next_action_field.as_deref().is_none_or(|want| {
                next_actions.iter().any(|a| match a {
                    RuleNextAction::CollectFact { field } => field == want,
                    _ => false,
                })
            });
            let pass = tag_ok && field_ok;
            CaseResult::new(
                &case.id,
                pass,
                format!("out_of_scope tag={coverage_tag:?} field={next_action_field:?}"),
                format!("out_of_scope tag={}", coverage_tag_str(*got_tag)),
            )
        }
        (Expect::Deadline { .. }, other) => {
            CaseResult::new(&case.id, false, "deadline Ok", format!("{other:?}"))
        }
        (Expect::OutOfScope { .. }, other) => {
            CaseResult::new(&case.id, false, "out_of_scope", format!("{other:?}"))
        }
    }
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: DeadlineDataset = read_json(dataset)?;
    let engine = RuleEngine::new().map_err(|e| format!("rule engine init: {e}"))?;
    let mut cases = Vec::with_capacity(data.cases.len());
    for case in &data.cases {
        let req = RuleRequest {
            intent: RuleIntent::Deadline(case.kind),
            facts: Default::default(),
            deadline_facts: Some(case.deadline_facts.clone()),
            province: case.province.clone(),
            city: case.city.clone(),
            severance_pre_tax: None,
        };
        match engine.try_resolve(&req) {
            Some(outcome) => cases.push(evaluate(case, &outcome)),
            None => cases.push(
                CaseResult::new(&case.id, false, "outcome", "None")
                    .with_note(format!("unknown province '{}'", case.province)),
            ),
        }
    }
    Ok(GateReport::from_cases("deadline", cases))
}
