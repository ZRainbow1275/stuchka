//! Three-stage architecture `answer()` — the A→F pipeline (ai/01 §1.5).
//!
//! ```text
//! A  HSD pre-filter   : ctx.scan(text) → ForceLocal pins Stage E to LocalQwen, NO cloud (INV-05)
//! B  KB increment     : ctx.kb_age_days() > 30 → Answer::level4_warning() (refused, no fallback)
//! C  KB hybrid query  : ctx.kb_query(text) → KB hits feed the source / confidence
//! D  Rule abstention  : ctx.rule_resolve(req) → Ok && coverage != Unknown ⇒ Answer::from_rule
//!                       (Stage E AI output discarded, INV-01); Unknown ⇒ AI capped at 0.49 (Low)
//! E  AI inference     : route by level (primary/secondary/local/rule-only/refuse)
//! F  confidence+INV-08 : compute_confidence → compose
//! ```
//!
//! The orchestration depends on three context traits ([`HsdContext`], [`KbContext`],
//! [`RuleContext`]) so it is unit-testable with mocks AND directly composable with the real `hsd` /
//! `kb` / `rule-engine` crates (the [`crate::wiring`] adapters implement them on the real types).

use async_trait::async_trait;
use data_model::{CoverageTag, NextAction, RouteHint, SourceTag};
use rule_engine::{RuleNextAction, RuleOutcome, RuleRequest};

use crate::answer::{Answer, EvidenceLink, ScoredAnswer, UserQuery};
use crate::confidence::{
    bucket, compute_confidence, ConfBucket, ConfidenceInput, RULE_ABSTENTION_CAP,
};
use crate::error::DispatcherError;
use crate::inv08_compose::compose;
use crate::inv08_template::Inv08Templates;
use crate::levels::DegradeLevel;
use crate::local_qwen::LocalQwen;
use crate::provider::{CompleteRequest, CompleteResponse, Provider, ProviderId};
use crate::routing::{
    decide_route, max_grade_of_fields, route as export_route, write_routing_audit, GuardRequest,
    Route, RouteTarget, RoutingAuditSink,
};

/// Stage A — high-sensitivity detector context.
pub trait HsdContext: Send + Sync {
    /// Scan text; returns `(route_hint, is_high_sensitive)`.
    fn scan(&self, text: &str) -> (RouteHint, bool);

    /// Desensitise `text` for a domestic-cloud send (`compliance/02` §3.4): mask every detected PII
    /// span and re-scan, returning `(masked_text, residual_pii)`. `residual_pii == true` means a
    /// strong signal survived masking and the export guard MUST force local. The default reuses
    /// [`HsdContext::scan`] semantics conservatively (no masking capability ⇒ if the text is high
    /// sensitive it is treated as residual so a mock can never silently leak), but the real
    /// [`crate::wiring::HsdAdapter`] overrides it with the genuine `routing::desensitize` pipeline.
    fn desensitize(&self, text: &str) -> (String, bool) {
        let (_, high) = self.scan(text);
        (text.to_string(), high)
    }
}

/// Stage B/C — knowledge base context.
#[async_trait]
pub trait KbContext: Send + Sync {
    /// Pull the incremental KB update if due (best-effort; errors are swallowed by the caller).
    async fn pull_incremental_if_due(&self) -> Result<(), DispatcherError>;
    /// Age of the active KB in days (Stage B Level4 gate).
    fn age_days(&self) -> u32;
    /// Hybrid query → (stable_id, score) hits, ordered by descending score.
    fn hybrid_query(&self, text: &str, top_k: usize) -> Vec<(String, f32)>;
    /// Active KB version label (for the Kb source template).
    fn version_label(&self) -> String;
}

/// Stage D — rule-engine context (synchronous resolve; the engine never links tokio, INC-4).
pub trait RuleContext: Send + Sync {
    /// Resolve the query into a rule outcome (when the query carries a resolvable intent).
    fn try_resolve(&self, req: &RuleRequest) -> Option<RuleOutcome>;
}

/// The data-export-guard inputs the live dispatch must supply (`compliance/02` §3 / §6): the user's
/// overseas-cloud settings flag and the routing-decision audit sink. Bundled into one parameter so
/// [`crate::Dispatcher::answer`] stays within the argument budget while still threading BOTH the
/// §3.1 `overseas_enabled` input and the §6 INV-06 audit sink onto the genuine dispatch.
#[derive(Default)]
pub struct RoutingGuardCtx<'a> {
    /// Whether the user has enabled the overseas-cloud channel at all (settings; default false).
    pub overseas_enabled: bool,
    /// The routing-decision audit sink (`compliance/02` §6, INV-06). The live `Dispatcher` always
    /// supplies a real sink so EVERY guard decision is written as the four-tuple `what`.
    pub audit: Option<&'a dyn RoutingAuditSink>,
}

/// The dispatcher context handed to [`answer`] — bundles the stage contexts + the AI resources.
pub struct StageContext<'a> {
    /// Stage A.
    pub hsd: &'a dyn HsdContext,
    /// Stage B/C.
    pub kb: &'a dyn KbContext,
    /// Stage D.
    pub rules: &'a dyn RuleContext,
    /// The current degrade level.
    pub level: DegradeLevel,
    /// Primary provider id.
    pub primary_id: ProviderId,
    /// Secondary provider id.
    pub secondary_id: ProviderId,
    /// Primary provider (None ⇒ skip cloud).
    pub primary: Option<&'a dyn Provider>,
    /// Secondary provider (None ⇒ skip cloud).
    pub secondary: Option<&'a dyn Provider>,
    /// Local model (None ⇒ no Level2 / force-local target).
    pub local: Option<&'a LocalQwen>,
    /// INV-08 templates.
    pub templates: &'a Inv08Templates,
    /// Optional rule request to feed Stage D (derived from `UserQuery` at the call site).
    pub rule_request: Option<RuleRequest>,
    /// Whether the user has enabled the overseas-cloud channel at all (settings; default false).
    /// Feeds the data-export guard (`compliance/02` §3.1 `overseas_enabled`).
    pub overseas_enabled: bool,
    /// The routing-decision audit sink (`compliance/02` §6, INV-06). EVERY routing decision the
    /// guard makes is written here as the four-tuple `what` — not just blocks. `None` only in unit
    /// fixtures that assert non-audit behaviour; the live `Dispatcher` always supplies a real sink.
    pub audit: Option<&'a dyn RoutingAuditSink>,
}

/// The A→F pipeline (ai/01 §1.5). Any stage hitting abstention returns immediately.
pub async fn answer(req: &UserQuery, ctx: &StageContext<'_>) -> Answer {
    // --- Stage A: HSD pre-filter ---------------------------------------------------------------
    let (route_hint, is_high) = ctx.hsd.scan(&req.text);

    // --- Stage B: KB increment + Level4 gate ---------------------------------------------------
    let _ = ctx.kb.pull_incremental_if_due().await; // best-effort
    if ctx.kb.age_days() > 30 || ctx.level == DegradeLevel::Level4 {
        return Answer::level4_warning();
    }

    // --- Stage C: KB hybrid query --------------------------------------------------------------
    let kb_hits = ctx.kb.hybrid_query(&req.text, 8);
    let kb_top = kb_hits
        .first()
        .map(|(_, s)| normalize_score(*s))
        .unwrap_or(0.0);
    let kb_scores: Vec<f32> = kb_hits.iter().map(|(_, s)| normalize_score(*s)).collect();

    // --- Stage D: rule abstention-first (INV-01) -----------------------------------------------
    let rule_outcome = ctx
        .rule_request
        .as_ref()
        .and_then(|r| ctx.rules.try_resolve(r));

    if let Some(outcome) = &rule_outcome {
        if outcome.coverage_tag() != CoverageTag::Unknown {
            // Determinate rule result: AI output is discarded entirely (INV-01).
            return compose(
                scored_from_rule(outcome, ctx.level, &kb_hits),
                ctx.templates,
            );
        }
    }

    let rule_coverage = rule_outcome.as_ref().map(|o| o.coverage_tag());

    // --- INV-05 data-export guard (compliance/02 §3) -------------------------------------------
    // EVERY dispatch passes through the export guard before any send (§3 性质: 未通过禁止发出).
    // Compute the payload's highest sensitivity grade from the request's structured fields (§2.1),
    // desensitise the free text for the domestic-cloud path (§3.4), then run the §3.1 routing
    // matrix. The verdict (a) may force the route local (overriding the user's channel, INV-05),
    // (b) tells us whether to send the MASKED payload to a domestic cloud, and (c) is ALWAYS
    // audited as the §6 four-tuple — not just on a block.
    let input_max_grade = max_grade_of_fields(req.structured_fields.iter().map(String::as_str));
    let (masked_text, residual_pii) = ctx.hsd.desensitize(&req.text);
    let guard_req = GuardRequest {
        input_max_grade,
        hsd_hit: is_high,
        scene_forced_local: req.scene_forced_local,
        overseas_enabled: ctx.overseas_enabled,
        overseas_chosen: req.allow_cross_border,
        desensitization_clean: !residual_pii,
    };
    let decision = export_route(&guard_req);

    // Write the §6 audit four-tuple on EVERY decision (INV-06; §7 forbids omitting override_reason —
    // write_routing_audit refuses an invariant-violating decision, so a non-compliant record can
    // never reach the chain). The sink is best-effort here: an audit-write failure must not silently
    // route around the guard, so we still honour the (already-computed) local/cloud verdict below.
    if let Some(sink) = ctx.audit {
        let kb_version = ctx.kb.version_label();
        let case_id = req.case_id.as_deref().unwrap_or("");
        let _ = write_routing_audit(
            sink,
            &decision,
            case_id,
            "", // why correlation id: api layer's trace id is threaded once the seam carries it
            None,
            &kb_version,
        )
        .await;
    }

    // --- Stage E: AI inference (routed by level) -----------------------------------------------
    // The guard forces local whenever its verdict is the local small steel cannon (R0/R1/R2/R3-
    // residual). We feed that into the EXISTING force-local gate (decide_route) rather than
    // bypassing it, so the HSD `RouteHint::ForceLocal` / no-local-model BlockedNoLocal handling is
    // preserved (INV-05): a guard-forced-local request with no local model is still BLOCKED, never
    // sent to any cloud.
    let guard_force_local =
        req.force_local || decision.actual_route == Route::LocalSmallSteelCannon;
    let route = decide_route(
        route_hint,
        guard_force_local,
        ctx.level,
        ctx.primary_id,
        ctx.secondary_id,
        ctx.local.is_some(),
    );

    // §3.4: the payload sent to a domestic cloud MUST be the desensitised text. The guard sets
    // `desensitization_applied` exactly on the R3 domestic-cloud-after-mask path; on that path the
    // residual-PII rescan was clean, so `masked_text` carries no surviving strong signal.
    let outbound_user_text: &str = if decision.desensitization_applied && route.is_cloud_target() {
        &masked_text
    } else {
        &req.text
    };
    let complete_req = CompleteRequest::new(SYSTEM_PROMPT, outbound_user_text);

    let ai: Result<Option<CompleteResponse>, DispatcherError> = match route {
        RouteTarget::Cloud(id) => {
            let provider = if id == ctx.primary_id {
                ctx.primary
            } else {
                ctx.secondary
            };
            match provider {
                Some(p) => p.complete(complete_req).await.map(Some),
                None => Ok(None),
            }
        }
        RouteTarget::Local => match ctx.local {
            Some(l) => l.complete(complete_req).await.map(Some),
            None => Ok(None),
        },
        RouteTarget::RuleOnly => {
            // Level3: no AI. Abstention-l3 built from the rule next-actions / KB hits.
            return compose(
                scored_abstention(rule_outcome.as_ref(), ctx.level, &kb_hits),
                ctx.templates,
            );
        }
        RouteTarget::Level4Refuse => return Answer::level4_warning(),
        RouteTarget::BlockedNoLocal => return Answer::hsd_blocked_no_local(),
    };

    let ai_response = match ai {
        Ok(Some(r)) => Some(r),
        // Provider/local unavailable → fall back to a Low heuristic follow-up (never a hard error
        // to the user; the api layer maps E_LLM_PROVIDER_DOWN separately when ALL fail).
        Ok(None) | Err(_) => None,
    };

    // --- Stage F: confidence + INV-08 ----------------------------------------------------------
    let ci = ConfidenceInput {
        model_logprobs: None,
        model_self_reported: ai_response
            .as_ref()
            .and_then(|r| r.self_reported_confidence),
        kb_hit_scores: kb_scores,
        kb_top_score: kb_top,
        rule_outcome: rule_coverage,
        hsd_route_forced_local: matches!(route, RouteTarget::Local)
            && matches!(route_hint, RouteHint::ForceLocal),
        level: ctx.level,
    };
    let mut confidence = compute_confidence(&ci);

    // INV-01 cap: rule abstention (Unknown) forces AI confidence into the Low bucket.
    if rule_coverage == Some(CoverageTag::Unknown) {
        confidence = confidence.min(RULE_ABSTENTION_CAP);
    }
    // No AI content at all → cannot be High/Mid; clamp to Low.
    if ai_response.is_none() {
        confidence = confidence.min(RULE_ABSTENTION_CAP);
    }

    let content = ai_response
        .as_ref()
        .map(|r| r.content.clone())
        .unwrap_or_default();
    let source_tag = source_tag_for(route, &kb_hits);

    let mut scored = ScoredAnswer::new(
        content,
        confidence,
        source_tag,
        CoverageTag::Approximate,
        ctx.level,
    );
    scored.kb_version = Some(ctx.kb.version_label());
    if let RouteTarget::Cloud(id) = route {
        scored.provider_label = Some(id.as_str().to_string());
    }
    scored.evidence_chain = kb_hits
        .iter()
        .take(3)
        .map(|(sid, _)| EvidenceLink::kb(sid.clone(), kb_hash(sid)))
        .collect();

    // Feed Low-bucket key facts from the rule next-actions when present (INV-08 consistency).
    if matches!(bucket(confidence), ConfBucket::Low) {
        if let Some(outcome) = &rule_outcome {
            scored.key_facts = key_facts_from_outcome(outcome);
            scored.next_actions = next_actions_from_outcome(outcome);
            scored.coverage_tag = CoverageTag::Unknown;
        }
    }

    compose(scored, ctx.templates)
}

/// Official default system prompt (0529 D6: owned by `crates/ai-dispatcher`, R1 deliverable, fork-
/// modifiable). The text lives in `crates/ai-dispatcher/prompts/system_prompt.md` and is embedded at
/// compile time via [`include_str!`] (cross-platform, no runtime path dependence) so the REAL prompt
/// sent to SiliconFlow / the local model is exactly that file. It carries the INV-09 anti-HR default
/// segment (spec `04-gplv3-readme-front.md` §6.3): default audience = 中国大陆劳动者, the 3-step
/// HR-stance handling, and the high-risk / legal-aid closing, on top of the evidence-collection /
/// legal-information helper framing.
const SYSTEM_PROMPT: &str = include_str!("../prompts/system_prompt.md");

/// Normalise a (potentially BM25, unbounded) score into 0..1 for the confidence fusion.
fn normalize_score(raw: f32) -> f32 {
    // BM25 scores are unbounded positive; squash with a soft saturation.
    if raw <= 0.0 {
        0.0
    } else {
        (raw / (raw + 5.0)).clamp(0.0, 1.0)
    }
}

fn source_tag_for(route: RouteTarget, kb_hits: &[(String, f32)]) -> SourceTag {
    match route {
        RouteTarget::Cloud(_) => SourceTag::Inferred,
        RouteTarget::Local => SourceTag::Inferred,
        _ if !kb_hits.is_empty() => SourceTag::Kb,
        _ => SourceTag::Inferred,
    }
}

fn kb_hash(stable_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(stable_id.as_bytes());
    let d = h.finalize();
    // 12-hex short hash for the evidence link.
    d.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// Build a determinate (rule) scored answer (INV-01: AI discarded).
fn scored_from_rule(
    outcome: &RuleOutcome,
    level: DegradeLevel,
    kb_hits: &[(String, f32)],
) -> ScoredAnswer {
    let coverage = outcome.coverage_tag();
    let kb_top = kb_hits
        .first()
        .map(|(_, s)| normalize_score(*s))
        .unwrap_or(0.0);
    let ci = ConfidenceInput {
        model_logprobs: None,
        model_self_reported: None,
        kb_hit_scores: kb_hits.iter().map(|(_, s)| normalize_score(*s)).collect(),
        kb_top_score: kb_top,
        rule_outcome: Some(coverage),
        hsd_route_forced_local: false,
        level,
    };
    let confidence = compute_confidence(&ci);

    let (content, evidence) = match outcome {
        RuleOutcome::Ok {
            law_refs,
            derivation,
            ..
        } => {
            let body = derivation
                .iter()
                .map(|d| format!("{}：{}", d.label_zh, d.expression))
                .collect::<Vec<_>>()
                .join("；");
            let ev = law_refs
                .iter()
                .map(|r| EvidenceLink::rule(r.urn.clone()))
                .collect();
            (body, ev)
        }
        RuleOutcome::OutOfScope { reasons, .. } => (reasons.join("；"), Vec::new()),
    };

    let mut scored = ScoredAnswer::new(content, confidence, SourceTag::Rule, coverage, level);
    scored.evidence_chain = evidence;
    scored.next_actions = next_actions_from_outcome(outcome);
    scored
}

/// Build a Level3 / no-AI abstention scored answer (Low bucket; heuristic follow-up).
fn scored_abstention(
    outcome: Option<&RuleOutcome>,
    level: DegradeLevel,
    kb_hits: &[(String, f32)],
) -> ScoredAnswer {
    let mut scored = ScoredAnswer::new(
        String::new(),
        0.2,
        if kb_hits.is_empty() {
            SourceTag::Inferred
        } else {
            SourceTag::Kb
        },
        CoverageTag::Unknown,
        level,
    );
    if let Some(o) = outcome {
        scored.key_facts = key_facts_from_outcome(o);
        scored.next_actions = next_actions_from_outcome(o);
    }
    scored
}

/// Extract Low-bucket key facts from a rule outcome's next-actions (ai/03 §3.5).
fn key_facts_from_outcome(outcome: &RuleOutcome) -> Vec<String> {
    let actions = match outcome {
        RuleOutcome::OutOfScope { next_actions, .. } => next_actions,
        RuleOutcome::Ok { .. } => return Vec::new(),
    };
    actions
        .iter()
        .map(|a| match a {
            RuleNextAction::CollectFact { field } => format!("补充事实：{field}"),
            RuleNextAction::CollectEvidence { class } => {
                format!("准备证据：{}", evidence_label(*class))
            }
            RuleNextAction::ShowUiHint { text } => text.clone(),
            RuleNextAction::ManualConfirm { reason } => format!("人工二次确认：{reason}"),
            RuleNextAction::ConsultLegalAid { hotline } => {
                format!("可拨打法律援助热线：{}", hotline.join(" / "))
            }
        })
        .collect()
}

fn evidence_label(class: data_model::EvidenceCategory) -> &'static str {
    use data_model::EvidenceCategory::*;
    match class {
        DocumentaryContract => "书证/合同",
        AudioVideo => "录音/录像",
        DigitalCommunication => "电子通讯记录",
        WitnessStatement => "证人证言",
        ScenePhotoVideo => "现场照片/视频",
        ThirdPartyData => "银行流水/税单/社保",
        Appraisal => "鉴定/评估",
    }
}

/// Project the rule next-actions onto the shared data-model discriminants.
fn next_actions_from_outcome(outcome: &RuleOutcome) -> Vec<NextAction> {
    match outcome {
        RuleOutcome::OutOfScope { next_actions, .. } => {
            next_actions.iter().map(|a| a.discriminant()).collect()
        }
        RuleOutcome::Ok { value, .. } => {
            // M5 deadline outputs always carry a ManualConfirm (INV-08 anchor, ai/06 §6.7).
            if let rule_engine::ComputedValue::Deadline(d) = value {
                if d.manual_confirm_required {
                    return vec![NextAction::ManualConfirm];
                }
            }
            Vec::new()
        }
    }
}

#[cfg(test)]
mod system_prompt_tests {
    use super::SYSTEM_PROMPT;

    /// The compile-time embed must equal the on-disk official prompt byte-for-byte: this pins the
    /// REAL prompt sent to SiliconFlow to `prompts/system_prompt.md` (spec §6.1 Prompt 层 / D6).
    #[test]
    fn embed_matches_official_prompt_file() {
        let on_disk = include_str!("../prompts/system_prompt.md");
        assert_eq!(
            SYSTEM_PROMPT, on_disk,
            "SYSTEM_PROMPT must be the include_str! embed of prompts/system_prompt.md"
        );
        assert!(
            !SYSTEM_PROMPT.is_empty(),
            "embedded system prompt must not be empty"
        );
    }

    /// The embedded prompt MUST carry the INV-09 anti-HR default segment verbatim (spec
    /// `04-gplv3-readme-front.md` §6.3): default audience, the 3-step HR-stance handling, and the
    /// high-risk / legal-aid closing. These assertions are the engineering contract for INV-09 —
    /// do not weaken them.
    #[test]
    fn embed_contains_anti_hr_default_segment() {
        // Default-audience block (中国大陆劳动者, low-income / low-digital-skill workers).
        assert!(
            SYSTEM_PROMPT.contains("你的默认服务对象是中国大陆的劳动者"),
            "missing default-audience declaration"
        );
        assert!(
            SYSTEM_PROMPT.contains("特别是低收入、低数字技能的工人群体"),
            "missing low-income / low-digital-skill audience"
        );
        // HR-stance detection cues.
        assert!(
            SYSTEM_PROMPT.contains("如果用户的请求显示其立场为雇主或人力资源代表"),
            "missing HR-stance detection clause"
        );
        // The 3-step handling.
        assert!(
            SYSTEM_PROMPT.contains("1. 不提供雇主视角的策略性建议。"),
            "missing step 1 (refuse employer-side strategy)"
        );
        assert!(
            SYSTEM_PROMPT.contains("2. 礼貌说明本系统的默认服务对象，并建议其使用雇主侧专业法律服务。"),
            "missing step 2 (redirect to employer-side counsel)"
        );
        assert!(
            SYSTEM_PROMPT.contains("3. 若用户坚持要求，可提供\"中性的程序性信息\""),
            "missing step 3 (neutral procedural info only)"
        );
        // High-risk / legal-aid closing.
        assert!(
            SYSTEM_PROMPT.contains("高风险决策必须经用户审阅，并在必要时联系执业律师或法援。"),
            "missing high-risk / legal-aid closing"
        );
        // Existing legal-information-helper framing must NOT be deleted (no functionality removed).
        assert!(
            SYSTEM_PROMPT.contains("你只提供法律信息与事实梳理"),
            "must retain the legal-information-helper framing"
        );
    }

    /// ZERO Emoji constraint: the official prompt must contain no emoji glyphs.
    #[test]
    fn embed_has_no_emoji() {
        for ch in SYSTEM_PROMPT.chars() {
            let c = ch as u32;
            let is_emoji = (0x1F000..=0x1FAFF).contains(&c)
                || (0x2600..=0x27BF).contains(&c)
                || (0x1F1E6..=0x1F1FF).contains(&c)
                || c == 0xFE0F;
            assert!(!is_emoji, "system prompt must contain no emoji (found U+{c:04X})");
        }
    }
}
