//! `/llm` routes (backend/01 §1.2 / §1.10, module M7).
//!
//! Unified AI entry with high-sensitivity interception (`crates/hsd` force-local gate) + the
//! three-stage / 4-layer fallback chain (`crates/ai-dispatcher`). The handler drives the REAL
//! [`ai_dispatcher::Dispatcher::answer`] A→F pipeline over the live hsd / kb / rule-engine
//! subsystems (abstention-first INV-01, INV-08 compose, source tags), and `/llm/provider/test`
//! probes the real SiliconFlow provider health endpoint. No stubs.

use ai_dispatcher::wiring::{HsdAdapter, KbAdapter, RuleAdapter};
use ai_dispatcher::{Provider, RoutingAuditSink, RoutingGuardCtx, UserQuery};
use axum::{extract::State, response::Response, routing::post, Json, Router};
use data_model::{CoverageTag, SourceTag};
use serde::{Deserialize, Serialize};

use crate::routing_audit_sink::AuditLogRoutingSink;
use crate::{responses, state::AppState};

/// Structured query context (backend/01 §1.10 `LlmContext`). Replaces the prior free
/// `serde_json::Value` with a typed, tolerant shape: every field is `#[serde(default)]` so an empty
/// `{}` (or omitted keys) still deserialises, while a known field is parsed into its type. Unknown
/// keys are ignored (forward-compatible). The context conditions the three-stage pipeline (the
/// dispute / region hints feed the KB stage; the dialogue/history is reserved for R1b multi-turn).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmContext {
    /// Dispute subtype / category hint (e.g. the case's `dispute_subtype`).
    #[serde(default)]
    pub dispute_subtype: Option<String>,
    /// Region hint — province (GB/T 2260 first 2 digits, D9).
    #[serde(default)]
    pub province: Option<String>,
    /// Region hint — city (GB/T 2260 first 4 digits, D9).
    #[serde(default)]
    pub city: Option<String>,
    /// Document id when the query is editor-scoped.
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Prior turns / extra hints (reserved for R1b multi-turn; tolerated now).
    #[serde(default)]
    pub history: Vec<String>,
}

/// `POST /llm/query` request (backend/01 §1.10).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmQueryReq {
    pub case_id: Option<String>,
    pub prompt: String,
    /// Structured query context (§1.10). `#[serde(default)]` keeps an empty `{}` / omitted body
    /// tolerant.
    #[serde(default)]
    pub context: LlmContext,
    /// User explicitly forces local processing.
    #[serde(default)]
    pub force_local: bool,
    /// Cross-border provider second confirmation (§5.3.2).
    #[serde(default)]
    pub allow_cross_border: bool,
}

/// `POST /llm/query` response (backend/01 §1.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmQueryResp {
    pub content: String,
    pub source_tag: SourceTag,
    pub confidence: f32,
    pub coverage_tag: CoverageTag,
    pub evidence_chain: Vec<EvidenceLink>,
    /// 0-4 (§4.5.3).
    pub fallback_level: u8,
    pub heuristic_followups: Vec<String>,
    /// High-sensitivity interception fired.
    pub pii_blocked: bool,
}

/// A cited law-ref id + KB fragment hash (INV-08 evidence chain).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLink {
    /// D8 URN law-ref id or KB stable id.
    pub law_ref_id: String,
    pub kb_fragment_hash: String,
}

/// `POST /llm/provider/test` response (real provider health probe).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthResp {
    pub provider: String,
    pub healthy: bool,
    pub latency_ms: u64,
    pub model: Option<String>,
}

/// Register `/llm/query` and `/llm/provider/test`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/llm/query", post(llm_query))
        .route("/llm/provider/test", post(provider_test))
}

async fn llm_query(State(s): State<AppState>, Json(req): Json<LlmQueryReq>) -> Response {
    let trace = s.new_trace_id();

    // Live subsystems for the three-stage pipeline (Stage A hsd, Stage B/C kb, Stage D rule).
    let (Some(hsd), Some(kb_index), Some(engine)) = (s.hsd(), s.kb_index(), s.rule_engine()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Some(dispatcher) = s.dispatcher() else {
        // No provider configured (secrets absent) → the cloud leg is down for this round.
        return responses::error(data_model::ErrorCode::LlmProviderDown, trace);
    };

    // Stage A pre-check: an hsd strong-signal hit forces local; if the user did not opt into local
    // (force_local) and no local model is available, surface E_PII_BLOCKED (backend/01 §1.12) before
    // any cloud call (INV-05). Otherwise the pipeline's own force-local gate handles routing.
    let report = hsd.scan(&req.prompt);
    if report.is_high_sensitive && !req.force_local && dispatcher.local.is_none() {
        return responses::error(data_model::ErrorCode::PiiBlocked, trace);
    }

    let hsd_ctx = HsdAdapter { detector: hsd };
    let kb_ctx = KbAdapter {
        index: kb_index,
        version: s.kb_version(),
        age_days: s.kb_age_days(),
    };
    let rule_ctx = RuleAdapter { engine };

    // §4.5 forced-local scene: a high-sensitivity case subtype (medical leave / three-periods /
    // minor / sexual harassment / work-injury appraisal / criminal report) forces local routing
    // regardless of the user's channel choice (INV-05) — derived from the structured context hint,
    // never overridable by the request body.
    let scene_forced_local = forced_local_scene(req.context.dispute_subtype.as_deref());

    let query = UserQuery {
        text: req.prompt.clone(),
        case_id: req.case_id.clone(),
        force_local: req.force_local,
        allow_cross_border: req.allow_cross_border,
        // R1a `/llm/query` carries free text + context hints, not a named structured-field payload;
        // the data-export guard grades the empty set conservatively as L2 and relies on the live
        // hsd layer (Stage A) to catch incidental PII (which then drives R2 force-local). The seam
        // is wired so a future structured-field channel feeds this directly (compliance/02 §3.1).
        structured_fields: Vec::new(),
        scene_forced_local,
    };

    // The LIVE data-export-guard audit sink (compliance/02 §6, INV-06): EVERY routing decision the
    // guard makes on this dispatch is written into the independent audit.sqlite chain. Present only
    // when the chain verified clean on startup — a broken chain must not be extended (D3); the guard
    // still runs and honours its local/cloud verdict either way (an audit gap never routes around it).
    let routing_sink = match (s.audit_chain_ok(), s.audit()) {
        (true, Some(audit)) => Some(AuditLogRoutingSink::new(audit.clone())),
        _ => None,
    };
    let audit_sink: Option<&dyn RoutingAuditSink> =
        routing_sink.as_ref().map(|s| s as &dyn RoutingAuditSink);

    // `overseas_enabled` mirrors the per-request cross-border consent: an overseas route (R5) needs
    // BOTH the channel enabled AND chosen, so without consent the guard can never select overseas.
    let overseas_enabled = req.allow_cross_border;

    // The api layer does not derive a determinate rule intent from free text in R1a; Stage D is fed
    // None (AI answers directly, abstention-first still capping confidence when uncertain). The
    // data-export-guard inputs (compliance/02 §3.1 `overseas_enabled` + §6 INV-06 audit sink) are
    // bundled into the `RoutingGuardCtx` so EVERY decision runs the guard and is audited.
    let answer = dispatcher
        .answer(
            &query,
            &hsd_ctx,
            &kb_ctx,
            &rule_ctx,
            None,
            RoutingGuardCtx {
                overseas_enabled,
                audit: audit_sink,
            },
        )
        .await;

    // Map the dispatcher Answer onto the api response (INV-08 surfaced verbatim).
    let resp = LlmQueryResp {
        content: answer.content,
        source_tag: answer.source_tag,
        confidence: answer.confidence,
        coverage_tag: answer.coverage_tag,
        evidence_chain: answer
            .evidence_chain
            .into_iter()
            .map(|l| EvidenceLink {
                law_ref_id: l.law_ref,
                kb_fragment_hash: l.kb_hash.unwrap_or_default(),
            })
            .collect(),
        fallback_level: answer.fallback_level,
        heuristic_followups: answer.heuristic_followups,
        pii_blocked: answer.pii_blocked,
    };
    responses::ok_200(resp, trace)
}

/// Map a case `dispute_subtype` hint onto the §4.5 forced-local scene flag (`compliance/02` §4.5):
/// medical leave / three-periods / minor / sexual harassment / work-injury appraisal material /
/// criminal report (拒不支付劳动报酬罪). Matched on substrings so a richer subtype label (e.g.
/// `"work_injury_appraisal"` or 中文 `"工伤鉴定"`) still trips the gate. The flag is the highest-
/// priority guard input (R0) and overrides every user channel choice (INV-05).
fn forced_local_scene(dispute_subtype: Option<&str>) -> bool {
    const SCENE_MARKERS: &[&str] = &[
        // English / snake_case subtype labels.
        "medical_leave",
        "three_period",
        "minor",
        "sexual_harassment",
        "work_injury_appraisal",
        "criminal_report",
        // 中文 case-subtype labels (§4.5 verbatim cues).
        "医疗期",
        "三期",
        "未成年",
        "性骚扰",
        "工伤鉴定",
        "刑事报案",
        "拒不支付劳动报酬",
    ];
    match dispute_subtype {
        Some(s) => SCENE_MARKERS.iter().any(|m| s.contains(m)),
        None => false,
    }
}

async fn provider_test(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(dispatcher) = s.dispatcher() else {
        return responses::error(data_model::ErrorCode::LlmProviderDown, trace);
    };
    // Probe the configured primary (DeepSeek via SiliconFlow); fall back to the secondary if the
    // primary is absent. This is a REAL network probe of the gateway `/models` endpoint.
    let probe = if let Some(p) = dispatcher.primary.as_ref() {
        p.health().await
    } else if let Some(p) = dispatcher.secondary.as_ref() {
        p.health().await
    } else {
        return responses::error(data_model::ErrorCode::LlmProviderDown, trace);
    };

    match probe {
        Ok(h) => responses::ok_200(
            ProviderHealthResp {
                provider: h.provider.as_str().to_string(),
                healthy: h.healthy,
                latency_ms: h.latency_ms,
                model: h.model,
            },
            trace,
        ),
        Err(e) => {
            tracing::warn!(error = %e, "provider health probe failed");
            responses::error(data_model::ErrorCode::LlmProviderDown, trace)
        }
    }
}
