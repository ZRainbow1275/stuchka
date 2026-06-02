//! abstention-300 gate (ai/05 §5.6, detail ai/03 §3.7-§3.8): INV-08 三档置信 拒答率 + 桶命中.
//!
//! Drives the REAL `ai_dispatcher` INV-08 path — [`compute_confidence`] -> [`bucket`] ->
//! [`compose`] for normal cases, and the genuine refusal answers [`Answer::level4_warning`] /
//! [`Answer::hsd_blocked_no_local`] for the (rare) hard-refusal cases — over a 300-case dataset, and
//! reports two metrics the Python runner asserts against §5.3:
//!   * `bucket_hit`   = fraction of cases whose REAL bucket matches the spec-derived `expect.bucket`.
//!   * `refusal_rate` = fraction of cases whose REAL answer is an abstention (`abstained == true`).
//!
//! The expected bucket is derived in the dataset generator from the ai/03 §3.3 boundaries (INV-01
//! rule-coverage dominance + the 0.5 / 0.8 cut points), INDEPENDENT of this crate. So a drift in
//! `compute_confidence` (e.g. a broken `RULE_ABSTENTION_CAP`) or a regression that turns a Low
//! heuristic follow-up into a hard refusal surfaces as a real failure — never a silent pass. No
//! answer text is fabricated: the model `content` is irrelevant to bucket/abstention, which are the
//! only things scored.

use ai_dispatcher::{
    compose, compute_confidence, Answer, ConfBucket, ConfidenceInput, DegradeLevel, Inv08Templates,
    ScoredAnswer,
};
use data_model::{CoverageTag, SourceTag};
use serde::Deserialize;

use crate::common::read_json;
use crate::report::{CaseResult, GateReport};

/// The confidence signals for one case (a serde-lenient mirror of [`ConfidenceInput`] so the
/// dataset is decoupled from that struct's exact field set; every field defaults).
#[derive(Debug, Default, Deserialize)]
struct Signals {
    #[serde(default)]
    model_logprobs: Option<Vec<f32>>,
    #[serde(default)]
    model_self_reported: Option<f32>,
    #[serde(default)]
    kb_hit_scores: Vec<f32>,
    #[serde(default)]
    kb_top_score: f32,
    /// INV-01 rule coverage tag (`exact|approximate|boundary|unknown`), or null for a pure-AI path.
    #[serde(default)]
    rule_outcome: Option<CoverageTag>,
    #[serde(default)]
    hsd_route_forced_local: bool,
    /// Degrade level (`level0..level4`); defaults to level0.
    #[serde(default)]
    level: Option<DegradeLevel>,
}

#[derive(Debug, Deserialize)]
struct Expect {
    /// Spec-derived bucket: `high|mid|low`.
    bucket: String,
    /// Whether this case is a genuine abstention (hard refusal). Defaults to false.
    #[serde(default)]
    abstained: bool,
}

#[derive(Debug, Deserialize)]
struct AbstentionCase {
    id: String,
    /// Which REAL production path to drive: `compose` (default) | `level4_stale` |
    /// `hsd_blocked_no_local`.
    #[serde(default = "default_route")]
    route: String,
    #[serde(default)]
    signals: Signals,
    expect: Expect,
}

fn default_route() -> String {
    "compose".to_string()
}

#[derive(Debug, Deserialize)]
struct AbstentionDataset {
    cases: Vec<AbstentionCase>,
}

fn bucket_str(b: ConfBucket) -> &'static str {
    match b {
        ConfBucket::High => "high",
        ConfBucket::Mid => "mid",
        ConfBucket::Low => "low",
    }
}

/// Produce the REAL answer for one case through the production INV-08 code (no mock path).
fn real_answer(case: &AbstentionCase, tpls: &Inv08Templates) -> Answer {
    match case.route.as_str() {
        "level4_stale" => Answer::level4_warning(),
        "hsd_blocked_no_local" => Answer::hsd_blocked_no_local(),
        // "compose" (default): compute_confidence -> bucket -> compose.
        _ => {
            let level = case.signals.level.unwrap_or(DegradeLevel::Level0);
            let ci = ConfidenceInput {
                model_logprobs: case.signals.model_logprobs.clone(),
                model_self_reported: case.signals.model_self_reported,
                kb_hit_scores: case.signals.kb_hit_scores.clone(),
                kb_top_score: case.signals.kb_top_score,
                rule_outcome: case.signals.rule_outcome,
                hsd_route_forced_local: case.signals.hsd_route_forced_local,
                level,
            };
            let conf = compute_confidence(&ci);
            let coverage = case.signals.rule_outcome.unwrap_or(CoverageTag::Unknown);
            // The content is irrelevant to the scored metrics (bucket + abstained).
            let scored = ScoredAnswer::new("评估占位内容", conf, SourceTag::Inferred, coverage, level);
            compose(scored, tpls)
        }
    }
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: AbstentionDataset = read_json(dataset)?;
    let tpls = Inv08Templates::default();
    let total = data.cases.len();
    let mut cases = Vec::with_capacity(total);
    let mut bucket_hits = 0usize;
    let mut refusals = 0usize;

    for case in &data.cases {
        let answer = real_answer(case, &tpls);
        let got_bucket = bucket_str(answer.inv08_bucket);
        let bucket_ok = got_bucket == case.expect.bucket;
        if bucket_ok {
            bucket_hits += 1;
        }
        if answer.abstained {
            refusals += 1;
        }
        let abstain_ok = answer.abstained == case.expect.abstained;
        let pass = bucket_ok && abstain_ok;
        cases.push(CaseResult::new(
            &case.id,
            pass,
            format!(
                "bucket={} abstained={}",
                case.expect.bucket, case.expect.abstained
            ),
            format!("bucket={got_bucket} abstained={}", answer.abstained),
        ));
    }

    let bucket_hit = if total == 0 {
        0.0
    } else {
        bucket_hits as f64 / total as f64
    };
    let refusal_rate = if total == 0 {
        0.0
    } else {
        refusals as f64 / total as f64
    };

    Ok(GateReport::from_cases("abstention", cases)
        .with_metric("bucket_hit", serde_json::json!(bucket_hit))
        .with_metric("refusal_rate", serde_json::json!(refusal_rate)))
}
