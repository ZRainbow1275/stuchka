//! §4.4 — `AuditReason` (the `why` of the four-tuple), full trigger-point list (~40 variants in 12
//! groups) plus the `AuditReason -> AuditCategory` projection (I2: `category` is derived, never
//! passed by the caller).

use data_model::AuditCategory;
use serde::{Deserialize, Serialize};

/// Every audit trigger point (data/04 §4.4). `snake_case` serde representation is the persisted
/// `why` text and the on-wire DTO `why`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditReason {
    // 1. AI key suggestions (INV-06 mandatory)
    AiDiagnosisOutput,
    AiCalculationSuggestion,
    AiDocumentDraft,
    AiEvidenceScoring,
    AiAbstention,

    // 2. User review
    UserConfirmFact,
    UserDisputeFact,
    UserConfirmClaim,
    UserWithdrawClaim,

    // 3. Fact state transition
    FactStateTransition,

    // 4. Evidence lifecycle
    EvidenceUploaded,
    EvidenceParsed,
    EvidenceScored,
    EvidenceQuarantined,

    // 5. Document generation / export
    DocumentFinalized,
    DocumentMerged,
    DocumentExported,
    GbMarkApplied,

    // 6. INV-10 high-risk decisions (§5.4.5)
    Inv10VoluntaryResign,
    Inv10SettlementBelow80,
    Inv10ClaimWithdraw,
    Inv10GroupRepresentAuth,
    Inv10CriminalReportExport,
    Inv10MedicalLeave,
    Inv10ThreePeriods,
    Inv10MinorProtection,

    // 7. Case freeze / state transition
    CaseFreeze,
    CaseStatusTransition,

    // 8. Knowledge base
    KbVersionInstalled,
    KbExpiredBlock,
    KbImpactNoticeCreated,
    KbImpactNoticeDismissed,

    // 9. AI dispatcher fallback
    AiFallbackLevelChange,

    // 10. High-sensitivity detector (crates/hsd) + data-export guard (crates/ai-dispatcher)
    HighSensitivityDetected,
    CloudRequestDoubleConfirm,
    /// Every AI-dispatch routing decision the data-export guard makes (`compliance/02` §6, INV-06):
    /// the who/when/why/what four-tuple carrying `input_max_grade` / `actual_route` /
    /// `override_reason` / `desensitization_applied`. Written on EVERY decision, not just blocks.
    AiRoutingDecision,

    // 11. Group cases (R1b)
    GroupCreate,
    GroupContributorJoin,
    GroupPatchExport,
    GroupPatchImport,

    // 12. Psychological crisis response (INV-07 neutralised)
    CrisisLevel1Detected,
    CrisisLevel2Detected,
    CrisisLevel3Detected,
    CrisisLevel3CooldownTriggered,
}

impl AuditReason {
    /// The persisted `why` text (serde `snake_case`), produced without allocating through serde.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditReason::AiDiagnosisOutput => "ai_diagnosis_output",
            AuditReason::AiCalculationSuggestion => "ai_calculation_suggestion",
            AuditReason::AiDocumentDraft => "ai_document_draft",
            AuditReason::AiEvidenceScoring => "ai_evidence_scoring",
            AuditReason::AiAbstention => "ai_abstention",
            AuditReason::UserConfirmFact => "user_confirm_fact",
            AuditReason::UserDisputeFact => "user_dispute_fact",
            AuditReason::UserConfirmClaim => "user_confirm_claim",
            AuditReason::UserWithdrawClaim => "user_withdraw_claim",
            AuditReason::FactStateTransition => "fact_state_transition",
            AuditReason::EvidenceUploaded => "evidence_uploaded",
            AuditReason::EvidenceParsed => "evidence_parsed",
            AuditReason::EvidenceScored => "evidence_scored",
            AuditReason::EvidenceQuarantined => "evidence_quarantined",
            AuditReason::DocumentFinalized => "document_finalized",
            AuditReason::DocumentMerged => "document_merged",
            AuditReason::DocumentExported => "document_exported",
            AuditReason::GbMarkApplied => "gb_mark_applied",
            AuditReason::Inv10VoluntaryResign => "inv10_voluntary_resign",
            AuditReason::Inv10SettlementBelow80 => "inv10_settlement_below80",
            AuditReason::Inv10ClaimWithdraw => "inv10_claim_withdraw",
            AuditReason::Inv10GroupRepresentAuth => "inv10_group_represent_auth",
            AuditReason::Inv10CriminalReportExport => "inv10_criminal_report_export",
            AuditReason::Inv10MedicalLeave => "inv10_medical_leave",
            AuditReason::Inv10ThreePeriods => "inv10_three_periods",
            AuditReason::Inv10MinorProtection => "inv10_minor_protection",
            AuditReason::CaseFreeze => "case_freeze",
            AuditReason::CaseStatusTransition => "case_status_transition",
            AuditReason::KbVersionInstalled => "kb_version_installed",
            AuditReason::KbExpiredBlock => "kb_expired_block",
            AuditReason::KbImpactNoticeCreated => "kb_impact_notice_created",
            AuditReason::KbImpactNoticeDismissed => "kb_impact_notice_dismissed",
            AuditReason::AiFallbackLevelChange => "ai_fallback_level_change",
            AuditReason::HighSensitivityDetected => "high_sensitivity_detected",
            AuditReason::CloudRequestDoubleConfirm => "cloud_request_double_confirm",
            AuditReason::AiRoutingDecision => "ai_routing_decision",
            AuditReason::GroupCreate => "group_create",
            AuditReason::GroupContributorJoin => "group_contributor_join",
            AuditReason::GroupPatchExport => "group_patch_export",
            AuditReason::GroupPatchImport => "group_patch_import",
            AuditReason::CrisisLevel1Detected => "crisis_level1_detected",
            AuditReason::CrisisLevel2Detected => "crisis_level2_detected",
            AuditReason::CrisisLevel3Detected => "crisis_level3_detected",
            AuditReason::CrisisLevel3CooldownTriggered => "crisis_level3_cooldown_triggered",
        }
    }

    /// Project the fine-grained reason onto the coarse `AuditCategory` exposed in the DTO (I2:
    /// the category is derived from the reason, not supplied by the caller).
    #[must_use]
    pub fn category(&self) -> AuditCategory {
        use AuditReason::*;
        match self {
            // AI suggestions / outputs
            AiDiagnosisOutput
            | AiCalculationSuggestion
            | AiDocumentDraft
            | AiEvidenceScoring
            | AiAbstention
            | AiFallbackLevelChange => AuditCategory::AiSuggestion,

            // User review of facts / claims (incl. INV-10 human confirmations)
            UserConfirmFact
            | UserDisputeFact
            | UserConfirmClaim
            | UserWithdrawClaim
            | Inv10VoluntaryResign
            | Inv10SettlementBelow80
            | Inv10ClaimWithdraw
            | Inv10GroupRepresentAuth
            | Inv10MedicalLeave
            | Inv10ThreePeriods
            | Inv10MinorProtection => AuditCategory::UserReview,

            // State changes across the case / fact / evidence / claim machines + lifecycle + kb +
            // crisis (all are state mutations, not exports/merges/crypto)
            FactStateTransition
            | EvidenceUploaded
            | EvidenceParsed
            | EvidenceScored
            | EvidenceQuarantined
            | CaseFreeze
            | CaseStatusTransition
            | KbVersionInstalled
            | KbExpiredBlock
            | KbImpactNoticeCreated
            | KbImpactNoticeDismissed
            | CrisisLevel1Detected
            | CrisisLevel2Detected
            | CrisisLevel3Detected
            | CrisisLevel3CooldownTriggered => AuditCategory::StateChange,

            // Three-way merge / group patch application
            DocumentMerged | GroupPatchImport => AuditCategory::MergeDecision,

            // Crypto / high-sensitivity routing operations (incl. the data-export guard verdict)
            HighSensitivityDetected | CloudRequestDoubleConfirm | AiRoutingDecision => {
                AuditCategory::CryptoOp
            }

            // Export-class events (finalized document, exports, GB mark, criminal report export)
            DocumentFinalized
            | DocumentExported
            | GbMarkApplied
            | Inv10CriminalReportExport
            | GroupPatchExport => AuditCategory::Export,

            // Sync / group membership lifecycle
            GroupCreate | GroupContributorJoin => AuditCategory::Sync,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list comes from data/04 §4.4; this guards the "~40 variants" contract.
    const ALL: &[AuditReason] = &[
        AuditReason::AiDiagnosisOutput,
        AuditReason::AiCalculationSuggestion,
        AuditReason::AiDocumentDraft,
        AuditReason::AiEvidenceScoring,
        AuditReason::AiAbstention,
        AuditReason::UserConfirmFact,
        AuditReason::UserDisputeFact,
        AuditReason::UserConfirmClaim,
        AuditReason::UserWithdrawClaim,
        AuditReason::FactStateTransition,
        AuditReason::EvidenceUploaded,
        AuditReason::EvidenceParsed,
        AuditReason::EvidenceScored,
        AuditReason::EvidenceQuarantined,
        AuditReason::DocumentFinalized,
        AuditReason::DocumentMerged,
        AuditReason::DocumentExported,
        AuditReason::GbMarkApplied,
        AuditReason::Inv10VoluntaryResign,
        AuditReason::Inv10SettlementBelow80,
        AuditReason::Inv10ClaimWithdraw,
        AuditReason::Inv10GroupRepresentAuth,
        AuditReason::Inv10CriminalReportExport,
        AuditReason::Inv10MedicalLeave,
        AuditReason::Inv10ThreePeriods,
        AuditReason::Inv10MinorProtection,
        AuditReason::CaseFreeze,
        AuditReason::CaseStatusTransition,
        AuditReason::KbVersionInstalled,
        AuditReason::KbExpiredBlock,
        AuditReason::KbImpactNoticeCreated,
        AuditReason::KbImpactNoticeDismissed,
        AuditReason::AiFallbackLevelChange,
        AuditReason::HighSensitivityDetected,
        AuditReason::CloudRequestDoubleConfirm,
        AuditReason::AiRoutingDecision,
        AuditReason::GroupCreate,
        AuditReason::GroupContributorJoin,
        AuditReason::GroupPatchExport,
        AuditReason::GroupPatchImport,
        AuditReason::CrisisLevel1Detected,
        AuditReason::CrisisLevel2Detected,
        AuditReason::CrisisLevel3Detected,
        AuditReason::CrisisLevel3CooldownTriggered,
    ];

    #[test]
    fn has_about_forty_variants() {
        // 43 data/04 §4.4 trigger points + the data-export-guard routing decision (compliance/02 §6).
        assert_eq!(ALL.len(), 44, "data/04 §4.4 + the ai_routing_decision trigger point");
    }

    #[test]
    fn as_str_matches_serde_snake_case() {
        for r in ALL {
            let via_serde = serde_json::to_value(r).unwrap();
            assert_eq!(via_serde.as_str().unwrap(), r.as_str(), "{r:?}");
        }
    }

    #[test]
    fn as_str_round_trips() {
        for r in ALL {
            let v = serde_json::Value::String(r.as_str().to_string());
            let back: AuditReason = serde_json::from_value(v).unwrap();
            assert_eq!(*r, back);
        }
    }

    #[test]
    fn every_reason_maps_to_a_category() {
        // exhaustiveness is enforced by the match; this asserts a couple of representative anchors
        assert_eq!(
            AuditReason::AiDiagnosisOutput.category(),
            AuditCategory::AiSuggestion
        );
        assert_eq!(
            AuditReason::DocumentMerged.category(),
            AuditCategory::MergeDecision
        );
        assert_eq!(
            AuditReason::EvidenceQuarantined.category(),
            AuditCategory::StateChange
        );
        assert_eq!(
            AuditReason::DocumentExported.category(),
            AuditCategory::Export
        );
        assert_eq!(
            AuditReason::HighSensitivityDetected.category(),
            AuditCategory::CryptoOp
        );
        assert_eq!(AuditReason::GroupCreate.category(), AuditCategory::Sync);
        // and that every variant resolves without panicking
        for r in ALL {
            let _ = r.category();
        }
    }
}
