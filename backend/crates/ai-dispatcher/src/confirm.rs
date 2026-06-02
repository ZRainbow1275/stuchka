//! Cloud upload second-confirmation UI protocol (ai/01 §1.7 + 安全 3 / C-C-8 / C-B-5).
//!
//! - 境内 (domestic): light confirm each request, may be remembered for 7 days.
//! - 境外 (overseas): full dialog + typed signature "我同意数据出境" **every** request, never
//!   remembered (C-B-5 locked).
//! - HSD strong hit: the confirm button is disabled — only "force local / cancel".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::provider::ProviderId;

/// The jurisdiction of the target provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Jurisdiction {
    /// CN (light confirm, memorable).
    Cn,
    /// Overseas (full dialog + signature, not memorable).
    Overseas,
}

/// AI purpose surfaced in the confirm preview (ai/01 §1.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPurpose {
    /// Case understanding / triage.
    Diagnosis,
    /// Document drafting.
    Document,
    /// Evidence scoring.
    EvidenceScoring,
    /// Knowledge-base query.
    KnowledgeQuery,
}

/// A summary of one HSD hit (ai/01 §1.7). Should be empty when the confirm dialog is offered; a
/// non-empty list forces local.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HsdHitSummary {
    /// PII kind label (e.g. "phone").
    pub kind: String,
    /// Detector layer (regex / ner).
    pub layer: String,
}

/// The cloud-confirm request handed to the UI (ai/01 §1.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConfirmRequest {
    /// Target provider.
    pub provider_id: ProviderId,
    /// Jurisdiction.
    pub jurisdiction: Jurisdiction,
    /// Masked preview (first 200 chars after PII masking).
    pub data_sample: String,
    /// HSD hits (empty unless forcing local).
    pub hsd_hits: Vec<HsdHitSummary>,
    /// Estimated token count for the preview.
    pub estimated_token_count: u32,
    /// AI purpose.
    pub purpose: AiPurpose,
    /// Session id.
    pub session_id: Uuid,
    /// Whether a signature is required (overseas = true, CN = false).
    pub require_signature: bool,
}

impl CloudConfirmRequest {
    /// Build a confirm request for a provider, deriving `jurisdiction` / `require_signature` from
    /// the provider id and forcing them when HSD hits are present.
    pub fn build(
        provider_id: ProviderId,
        data_sample: String,
        hsd_hits: Vec<HsdHitSummary>,
        estimated_token_count: u32,
        purpose: AiPurpose,
        session_id: Uuid,
    ) -> Self {
        let overseas = provider_id.is_overseas();
        Self {
            provider_id,
            jurisdiction: if overseas {
                Jurisdiction::Overseas
            } else {
                Jurisdiction::Cn
            },
            data_sample,
            hsd_hits,
            estimated_token_count,
            purpose,
            session_id,
            require_signature: overseas,
        }
    }

    /// Whether the confirm button must be disabled (HSD hit present → only force-local / cancel).
    pub fn confirm_button_disabled(&self) -> bool {
        !self.hsd_hits.is_empty()
    }

    /// Whether the user's "remember 7 days" choice is allowed (CN only, and only when not HSD-blocked).
    pub fn memory_allowed(&self) -> bool {
        matches!(self.jurisdiction, Jurisdiction::Cn) && self.hsd_hits.is_empty()
    }
}

/// The user's response to a confirm request (ai/01 §1.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum CloudConfirmResponse {
    /// Accepted (overseas carries a signature).
    Accepted {
        /// Acceptance timestamp.
        ts: chrono::DateTime<chrono::Utc>,
        /// Typed signature (overseas only).
        signature: Option<String>,
    },
    /// Declined with a reason.
    Declined {
        /// Decline reason.
        reason: String,
    },
    /// Declined and the user chose to force local processing.
    DeclinedAndForceLocal,
}

/// The outcome of evaluating a confirm response against the request's policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// Cloud request authorised.
    ProceedCloud,
    /// Force local processing (declined-and-force-local or HSD-blocked).
    ForceLocal,
    /// Cancelled / declined without forcing local.
    Cancelled,
    /// Overseas accepted but no/empty signature — rejected (C-B-5).
    SignatureMissing,
}

/// Evaluate a confirm response against the request's signature / HSD policy.
pub fn evaluate(req: &CloudConfirmRequest, resp: &CloudConfirmResponse) -> ConfirmOutcome {
    // HSD hit present: cloud is never allowed regardless of the response.
    if req.confirm_button_disabled() {
        return match resp {
            CloudConfirmResponse::DeclinedAndForceLocal => ConfirmOutcome::ForceLocal,
            _ => ConfirmOutcome::ForceLocal,
        };
    }
    match resp {
        CloudConfirmResponse::Accepted { signature, .. } => {
            if req.require_signature {
                match signature {
                    Some(s) if !s.trim().is_empty() => ConfirmOutcome::ProceedCloud,
                    _ => ConfirmOutcome::SignatureMissing,
                }
            } else {
                ConfirmOutcome::ProceedCloud
            }
        }
        CloudConfirmResponse::Declined { .. } => ConfirmOutcome::Cancelled,
        CloudConfirmResponse::DeclinedAndForceLocal => ConfirmOutcome::ForceLocal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cn_req() -> CloudConfirmRequest {
        CloudConfirmRequest::build(
            ProviderId::DeepSeek,
            "脱敏预览".into(),
            vec![],
            1200,
            AiPurpose::Diagnosis,
            Uuid::now_v7(),
        )
    }

    fn overseas_req() -> CloudConfirmRequest {
        CloudConfirmRequest::build(
            ProviderId::OpenAI,
            "脱敏预览".into(),
            vec![],
            1200,
            AiPurpose::Diagnosis,
            Uuid::now_v7(),
        )
    }

    fn hsd_req() -> CloudConfirmRequest {
        CloudConfirmRequest::build(
            ProviderId::DeepSeek,
            "脱敏预览".into(),
            vec![HsdHitSummary {
                kind: "phone".into(),
                layer: "regex".into(),
            }],
            1200,
            AiPurpose::Diagnosis,
            Uuid::now_v7(),
        )
    }

    // Scenario 1: 境内单次确认 → proceed.
    #[test]
    fn cn_single_confirm_proceeds() {
        let req = cn_req();
        assert!(!req.require_signature);
        let resp = CloudConfirmResponse::Accepted {
            ts: chrono::Utc::now(),
            signature: None,
        };
        assert_eq!(evaluate(&req, &resp), ConfirmOutcome::ProceedCloud);
    }

    // Scenario 2: 境内记忆 7 天允许.
    #[test]
    fn cn_memory_allowed() {
        assert!(cn_req().memory_allowed());
    }

    // Scenario 3: 境外签名 → proceed.
    #[test]
    fn overseas_signed_proceeds() {
        let req = overseas_req();
        assert!(req.require_signature);
        assert!(!req.memory_allowed()); // overseas never memorable
        let resp = CloudConfirmResponse::Accepted {
            ts: chrono::Utc::now(),
            signature: Some("我同意数据出境".into()),
        };
        assert_eq!(evaluate(&req, &resp), ConfirmOutcome::ProceedCloud);
    }

    // Scenario 4: 境外拒签 (accepted but empty signature) → rejected.
    #[test]
    fn overseas_unsigned_rejected() {
        let req = overseas_req();
        let resp = CloudConfirmResponse::Accepted {
            ts: chrono::Utc::now(),
            signature: None,
        };
        assert_eq!(evaluate(&req, &resp), ConfirmOutcome::SignatureMissing);
    }

    // Scenario 5: 高敏阻断 → confirm disabled, force local.
    #[test]
    fn hsd_blocked_forces_local() {
        let req = hsd_req();
        assert!(req.confirm_button_disabled());
        assert!(!req.memory_allowed());
        let resp = CloudConfirmResponse::DeclinedAndForceLocal;
        assert_eq!(evaluate(&req, &resp), ConfirmOutcome::ForceLocal);
    }

    // Scenario 6: 取消 → cancelled.
    #[test]
    fn cancel_is_cancelled() {
        let req = cn_req();
        let resp = CloudConfirmResponse::Declined {
            reason: "用户取消".into(),
        };
        assert_eq!(evaluate(&req, &resp), ConfirmOutcome::Cancelled);
    }
}
