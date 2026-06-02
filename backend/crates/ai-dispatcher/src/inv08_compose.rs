//! Assemble the final [`Answer`] from a [`ScoredAnswer`] + templates (ai/03 §3.4).
//!
//! High → "依据 {source}，结论：{content}". Mid → answer + uncertainty + at least 2 alternatives.
//! Low → at least 3 key facts + the re-ask phrase (NOT a refusal — PM P5). The Low key facts are
//! sourced from the rule engine's next_actions when present (consistent with what the rules need).

use crate::answer::{Answer, ScoredAnswer};
use crate::confidence::{bucket, ConfBucket};
use crate::inv08_template::{Inv08Templates, RE_ASK_PHRASE};

/// Compose the final answer for a scored intermediate (ai/03 §3.4).
pub fn compose(scored: ScoredAnswer, tpls: &Inv08Templates) -> Answer {
    match bucket(scored.confidence) {
        ConfBucket::High => compose_high(scored, tpls),
        ConfBucket::Mid => compose_mid(scored, tpls),
        ConfBucket::Low => compose_low(scored, tpls),
    }
}

fn render_source_label(tpls: &Inv08Templates, scored: &ScoredAnswer) -> String {
    let raw = tpls.source_label(scored.source_tag).to_string();
    let kb_version = scored
        .kb_version
        .clone()
        .unwrap_or_else(|| "本地版本".to_string());
    let provider = scored
        .provider_label
        .clone()
        .unwrap_or_else(|| "云端".to_string());
    let ts = chrono::Utc::now().format("%Y-%m-%d").to_string();
    raw.replace("{kb_version}", &kb_version)
        .replace("{provider}", &provider)
        .replace("{ts}", &ts)
}

fn compose_high(scored: ScoredAnswer, tpls: &Inv08Templates) -> Answer {
    let source = render_source_label(tpls, &scored);
    let content = tpls
        .high
        .format_zh
        .replace("{source}", &source)
        .replace("{content}", &scored.content);
    Answer {
        content,
        confidence: scored.confidence,
        source_tag: scored.source_tag,
        coverage_tag: scored.coverage_tag,
        evidence_chain: scored.evidence_chain,
        fallback_level: scored.level.as_u8(),
        inv08_bucket: ConfBucket::High,
        heuristic_followups: Vec::new(),
        next_actions: scored.next_actions,
        pii_blocked: false,
        abstained: false,
    }
}

fn compose_mid(mut scored: ScoredAnswer, tpls: &Inv08Templates) -> Answer {
    // Ensure >= 2 alternatives (INV-08 Mid anchor). Pad with a generic legal-aid / dual-claim path.
    ensure_min_alternatives(&mut scored.alternatives);
    debug_assert!(
        scored.alternatives.len() >= 2,
        "INV-08 Mid needs >= 2 paths"
    );

    let uncertainty = if scored.uncertainty_reasons.is_empty() {
        "可用信息有限，地方口径可能存在差异".to_string()
    } else {
        scored.uncertainty_reasons.join("、")
    };
    let alt_a = scored.alternatives.first().cloned().unwrap_or_default();
    let alt_b = scored.alternatives.get(1).cloned().unwrap_or_default();

    let content = tpls
        .mid
        .format_zh
        .replace("{content}", &scored.content)
        .replace("{uncertainty_reasons}", &uncertainty)
        .replace("{alt_a}", &alt_a)
        .replace("{alt_b}", &alt_b);

    Answer {
        content,
        confidence: scored.confidence,
        source_tag: scored.source_tag,
        coverage_tag: scored.coverage_tag,
        evidence_chain: scored.evidence_chain,
        fallback_level: scored.level.as_u8(),
        inv08_bucket: ConfBucket::Mid,
        heuristic_followups: scored.alternatives,
        next_actions: scored.next_actions,
        pii_blocked: false,
        abstained: false,
    }
}

fn compose_low(mut scored: ScoredAnswer, tpls: &Inv08Templates) -> Answer {
    // Ensure >= 3 key facts (INV-08 Low anchor). Pad from the generic labour-dispute fact set.
    ensure_min_key_facts(&mut scored.key_facts);
    debug_assert!(scored.key_facts.len() >= 3, "INV-08 Low needs >= 3 facts");

    let numbered = number_facts(&scored.key_facts);
    let content = tpls.low.format_zh.replace("{key_facts}", &numbered);
    // Defensive: the template footer already carries the re-ask phrase, but guarantee it.
    let content = if content.contains(RE_ASK_PHRASE) {
        content
    } else {
        format!("{content}\n建议先做完这几件事{RE_ASK_PHRASE}。")
    };

    Answer {
        content,
        confidence: scored.confidence,
        source_tag: scored.source_tag,
        coverage_tag: scored.coverage_tag,
        evidence_chain: scored.evidence_chain,
        fallback_level: scored.level.as_u8(),
        inv08_bucket: ConfBucket::Low,
        heuristic_followups: scored.key_facts,
        next_actions: scored.next_actions,
        pii_blocked: false,
        // Low is a heuristic follow-up, NOT a refusal (PM P5 / INV-08).
        abstained: false,
    }
}

/// The circled-number prefixes for the Low fact list.
const CIRCLED: [&str; 6] = ["①", "②", "③", "④", "⑤", "⑥"];

fn number_facts(facts: &[String]) -> String {
    facts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let marker = CIRCLED.get(i).copied().unwrap_or("•");
            format!("{marker} {f}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generic labour-dispute fact prompts used to pad a thin Low key-fact list (never legal advice;
/// these are evidence-collection prompts).
const GENERIC_KEY_FACTS: [&str; 3] = [
    "确认入职日期与解除日期（用于工龄计算）",
    "确认解除原因（用人单位主张的解除依据）",
    "整理前 12 个月工资明细（用于月平均工资计算）",
];

fn ensure_min_key_facts(facts: &mut Vec<String>) {
    for extra in GENERIC_KEY_FACTS {
        if facts.len() >= 3 {
            break;
        }
        if !facts.iter().any(|f| f == extra) {
            facts.push(extra.to_string());
        }
    }
    // Absolute floor: if dedup still left < 3, append distinct generic prompts.
    let mut idx = 0;
    while facts.len() < 3 {
        facts.push(format!("补充关键事实 {}", idx + 1));
        idx += 1;
    }
}

const GENERIC_ALTERNATIVES: [&str; 2] = [
    "携带相关材料咨询当地劳动监察大队或法律援助中心",
    "在仲裁请求中同时主张不同口径的金额，由仲裁庭裁定",
];

fn ensure_min_alternatives(alts: &mut Vec<String>) {
    for extra in GENERIC_ALTERNATIVES {
        if alts.len() >= 2 {
            break;
        }
        if !alts.iter().any(|a| a == extra) {
            alts.push(extra.to_string());
        }
    }
    let mut idx = 0;
    while alts.len() < 2 {
        alts.push(format!("备选路径 {}", idx + 1));
        idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::DegradeLevel;
    use data_model::{CoverageTag, SourceTag};

    fn scored(conf: f32) -> ScoredAnswer {
        ScoredAnswer::new(
            "测试内容",
            conf,
            SourceTag::Inferred,
            CoverageTag::Approximate,
            DegradeLevel::Level0,
        )
    }

    #[test]
    fn high_renders_source_and_content() {
        let mut s = scored(0.92);
        s.source_tag = SourceTag::Rule;
        let a = compose(s, &Inv08Templates::default());
        assert_eq!(a.inv08_bucket, ConfBucket::High);
        assert!(a.content.contains("规则引擎"));
        assert!(a.content.contains("测试内容"));
        assert!(!a.abstained);
    }

    #[test]
    fn mid_has_two_paths() {
        let a = compose(scored(0.65), &Inv08Templates::default());
        assert_eq!(a.inv08_bucket, ConfBucket::Mid);
        assert!(a.heuristic_followups.len() >= 2);
        assert!(a.content.contains("次优路径"));
    }

    #[test]
    fn low_has_three_facts_and_re_ask() {
        let a = compose(scored(0.32), &Inv08Templates::default());
        assert_eq!(a.inv08_bucket, ConfBucket::Low);
        assert!(a.heuristic_followups.len() >= 3);
        assert!(a.content.contains(RE_ASK_PHRASE));
        // Low is a follow-up, not a refusal.
        assert!(!a.abstained);
    }

    #[test]
    fn low_keeps_supplied_facts_when_enough() {
        let mut s = scored(0.2);
        s.key_facts = vec!["事实甲".into(), "事实乙".into(), "事实丙".into()];
        let a = compose(s, &Inv08Templates::default());
        assert_eq!(a.heuristic_followups.len(), 3);
        assert!(a.content.contains("事实甲"));
    }
}
