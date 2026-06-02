//! Confidence computation + three-tier bucketing (ai/03 §3.3).
//!
//! INV-01 priority: when the rule engine gives a coverage tag, it dominates the score:
//! `Exact -> 0.95`, `Approximate -> 0.80`, `Boundary -> cap 0.70`, `Unknown -> kb_top.min(0.45) *
//! level_penalty` (always `< 0.5`, forcing the Low bucket). Otherwise a weighted fusion of model
//! logprobs / self-report / KB top score, scaled by the level penalty.

use data_model::CoverageTag;
use serde::{Deserialize, Serialize};

use crate::levels::DegradeLevel;

/// Hard cap applied to AI confidence when the rule engine abstained (`Unknown`) — guarantees the
/// answer enters the Low bucket (INV-01 验收).
pub const RULE_ABSTENTION_CAP: f32 = 0.49;

/// Inputs to [`compute_confidence`] (ai/03 §3.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceInput {
    /// Provider top-1 logprobs (averaged → softmax), when supported.
    pub model_logprobs: Option<Vec<f32>>,
    /// Provider self-reported confidence, when supported.
    pub model_self_reported: Option<f32>,
    /// KB hybrid-search RRF top-8 scores.
    pub kb_hit_scores: Vec<f32>,
    /// KB best score (normalised 0..1).
    pub kb_top_score: f32,
    /// Rule-engine coverage tag (INV-01).
    pub rule_outcome: Option<CoverageTag>,
    /// Whether the HSD forced local routing (quality discounted).
    pub hsd_route_forced_local: bool,
    /// Current degrade level.
    pub level: DegradeLevel,
}

impl ConfidenceInput {
    /// A minimal input for tests / non-rule paths.
    pub fn new(level: DegradeLevel) -> Self {
        Self {
            model_logprobs: None,
            model_self_reported: None,
            kb_hit_scores: Vec::new(),
            kb_top_score: 0.0,
            rule_outcome: None,
            hsd_route_forced_local: false,
            level,
        }
    }
}

/// The level quality penalty (ai/03 §3.3).
pub fn level_penalty(level: DegradeLevel) -> f32 {
    match level {
        DegradeLevel::Level0 | DegradeLevel::Level1 => 1.00,
        DegradeLevel::Level2 => 0.85, // local small model discount
        DegradeLevel::Level3 => 0.60, // rules only
        DegradeLevel::Level4 => 0.40, // KB stale
    }
}

/// Squash an averaged logprob into a 0..1 probability (logistic on the log-odds proxy).
fn softmax_to_prob(avg_logprob: f32) -> f32 {
    // avg_logprob is <= 0; map to (0,1): higher (closer to 0) → higher confidence.
    let p = 1.0 / (1.0 + (-avg_logprob).exp());
    p.clamp(0.0, 1.0)
}

/// Compute the final confidence (ai/03 §3.3). The result is always clamped to `[0.0, 1.0]`.
pub fn compute_confidence(ci: &ConfidenceInput) -> f32 {
    // INV-01: rule coverage dominates.
    match ci.rule_outcome {
        Some(CoverageTag::Exact) => return 0.95,
        Some(CoverageTag::Approximate) => return 0.80,
        Some(CoverageTag::Boundary) => {
            // Boundary caps at 0.70 → Mid bucket (ai/03 §3.5).
            let base = (0.50 + 0.35 * ci.kb_top_score).min(0.70);
            return (base * level_penalty(ci.level)).clamp(0.0, 0.70);
        }
        Some(CoverageTag::Unknown) => {
            // Rule abstention: base must be < 0.5 (kb_top.min(0.45)), then penalised. Never exceeds
            // the cap.
            let kb_part = ci.kb_top_score.min(0.45);
            let raw = kb_part * level_penalty(ci.level);
            return raw.min(RULE_ABSTENTION_CAP).clamp(0.0, RULE_ABSTENTION_CAP);
        }
        None => {}
    }

    // Weighted fusion (no rule coverage).
    let logp = ci
        .model_logprobs
        .as_ref()
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().sum::<f32>() / v.len() as f32)
        .map(softmax_to_prob)
        .unwrap_or(0.6);
    let self_rep = ci.model_self_reported.unwrap_or(logp);
    let kb_top = ci.kb_top_score;

    let raw = 0.45 * logp + 0.20 * self_rep + 0.35 * kb_top;
    let mut conf = raw * level_penalty(ci.level);

    // HSD forced-local further discounts (local-only quality).
    if ci.hsd_route_forced_local {
        conf *= 0.9;
    }
    conf.clamp(0.0, 1.0)
}

/// The three INV-08 buckets (ai/03 §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfBucket {
    /// `>= 0.8` — direct answer + source label.
    High,
    /// `[0.5, 0.8)` — answer + uncertainty + >= 2 alternative paths.
    Mid,
    /// `< 0.5` — NOT a refusal; heuristic follow-up with >= 3 key facts.
    Low,
}

/// Bucket a confidence score (ai/03 §3.3 boundaries: `>=0.8 High`, `>=0.5 Mid`, else `Low`).
pub fn bucket(conf: f32) -> ConfBucket {
    if conf >= 0.8 {
        ConfBucket::High
    } else if conf >= 0.5 {
        ConfBucket::Mid
    } else {
        ConfBucket::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_boundaries() {
        // ai/03 §3.9: 0.49 / 0.50 / 0.79 / 0.80 / 0.81 / 0.95.
        assert_eq!(bucket(0.49), ConfBucket::Low);
        assert_eq!(bucket(0.50), ConfBucket::Mid);
        assert_eq!(bucket(0.79), ConfBucket::Mid);
        assert_eq!(bucket(0.80), ConfBucket::High);
        assert_eq!(bucket(0.81), ConfBucket::High);
        assert_eq!(bucket(0.95), ConfBucket::High);
    }

    #[test]
    fn exact_rule_is_high() {
        let mut ci = ConfidenceInput::new(DegradeLevel::Level0);
        ci.rule_outcome = Some(CoverageTag::Exact);
        let c = compute_confidence(&ci);
        assert_eq!(c, 0.95);
        assert_eq!(bucket(c), ConfBucket::High);
    }

    #[test]
    fn approximate_rule_is_080() {
        let mut ci = ConfidenceInput::new(DegradeLevel::Level0);
        ci.rule_outcome = Some(CoverageTag::Approximate);
        assert_eq!(compute_confidence(&ci), 0.80);
    }

    #[test]
    fn boundary_caps_at_070_mid() {
        let mut ci = ConfidenceInput::new(DegradeLevel::Level0);
        ci.rule_outcome = Some(CoverageTag::Boundary);
        ci.kb_top_score = 1.0;
        let c = compute_confidence(&ci);
        assert!(c <= 0.70, "boundary must cap at 0.70, got {c}");
        assert_eq!(bucket(c), ConfBucket::Mid);
    }

    #[test]
    fn unknown_rule_caps_below_05_low() {
        // Even with a perfect KB score and Level0, Unknown stays < 0.5 → Low.
        let mut ci = ConfidenceInput::new(DegradeLevel::Level0);
        ci.rule_outcome = Some(CoverageTag::Unknown);
        ci.kb_top_score = 1.0;
        let c = compute_confidence(&ci);
        assert!(c <= RULE_ABSTENTION_CAP, "must cap at 0.49, got {c}");
        assert_eq!(bucket(c), ConfBucket::Low);
    }

    #[test]
    fn level_penalty_monotone() {
        assert_eq!(level_penalty(DegradeLevel::Level0), 1.0);
        assert_eq!(level_penalty(DegradeLevel::Level2), 0.85);
        assert_eq!(level_penalty(DegradeLevel::Level3), 0.60);
        assert_eq!(level_penalty(DegradeLevel::Level4), 0.40);
    }

    #[test]
    fn fusion_clamps_to_unit() {
        let mut ci = ConfidenceInput::new(DegradeLevel::Level0);
        ci.model_self_reported = Some(2.0);
        ci.kb_top_score = 2.0;
        let c = compute_confidence(&ci);
        assert!((0.0..=1.0).contains(&c));
    }
}
