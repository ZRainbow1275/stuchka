//! §4.4 — `what` validation, keyed by `why`.
//!
//! data/04 §4.4 says each trigger point has a `what` schema under `crates/audit/src/schemas/`
//! that is enforced before insert. R1a implements the load-bearing structural invariants directly
//! (no external JSON-Schema engine pulled in): `what` must be a JSON object, and case-scoped
//! reasons must carry a `case_id` that parses as a UUID (the D9 soft reference into the main
//! store). This is the real append-time guard the chain relies on; richer per-field schemas are a
//! make-usable extension (R1b) layered on the same seam.

use uuid::Uuid;

use crate::error::AuditError;
use crate::reason::AuditReason;

/// Reasons whose `what` must reference a case via a `case_id` UUID (data/04 §4.4 — `why` aligns
/// with the per-object state machines that are all case-scoped).
fn requires_case_id(why: AuditReason) -> bool {
    use AuditReason::*;
    matches!(
        why,
        AiDiagnosisOutput
            | AiCalculationSuggestion
            | AiDocumentDraft
            | AiEvidenceScoring
            | AiAbstention
            | UserConfirmFact
            | UserDisputeFact
            | UserConfirmClaim
            | UserWithdrawClaim
            | FactStateTransition
            | EvidenceUploaded
            | EvidenceParsed
            | EvidenceScored
            | EvidenceQuarantined
            | DocumentFinalized
            | DocumentMerged
            | DocumentExported
            | GbMarkApplied
            | Inv10VoluntaryResign
            | Inv10SettlementBelow80
            | Inv10ClaimWithdraw
            | Inv10GroupRepresentAuth
            | Inv10CriminalReportExport
            | Inv10MedicalLeave
            | Inv10ThreePeriods
            | Inv10MinorProtection
            | CaseFreeze
            | CaseStatusTransition
    )
}

/// Validate `what` against the `why`-bound schema (§4.4); returns the `case_id` soft reference if
/// the `what` carries one (used to index `query_by_case`).
pub fn validate_what(
    why: AuditReason,
    what: &serde_json::Value,
) -> Result<Option<Uuid>, AuditError> {
    if !what.is_object() {
        return Err(AuditError::SchemaValidation {
            why: why.as_str(),
            detail: "`what` must be a JSON object".into(),
        });
    }

    let case_id = match what.get("case_id") {
        Some(serde_json::Value::String(s)) => {
            let id = Uuid::parse_str(s).map_err(|_| AuditError::SchemaValidation {
                why: why.as_str(),
                detail: format!("`case_id` is not a valid UUID: {s}"),
            })?;
            Some(id)
        }
        Some(serde_json::Value::Null) | None => None,
        Some(other) => {
            return Err(AuditError::SchemaValidation {
                why: why.as_str(),
                detail: format!("`case_id` must be a string UUID, got {other}"),
            });
        }
    };

    if requires_case_id(why) && case_id.is_none() {
        return Err(AuditError::SchemaValidation {
            why: why.as_str(),
            detail: "this trigger point requires a `case_id` in `what`".into(),
        });
    }

    Ok(case_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn case_scoped_requires_case_id() {
        let err = validate_what(AuditReason::CaseFreeze, &json!({})).unwrap_err();
        assert!(matches!(err, AuditError::SchemaValidation { .. }));
    }

    #[test]
    fn case_scoped_accepts_valid_uuid() {
        let id = Uuid::now_v7();
        let got = validate_what(
            AuditReason::EvidenceUploaded,
            &json!({"case_id": id.to_string()}),
        )
        .unwrap();
        assert_eq!(got, Some(id));
    }

    #[test]
    fn bad_uuid_is_rejected() {
        let err =
            validate_what(AuditReason::CaseFreeze, &json!({"case_id": "not-a-uuid"})).unwrap_err();
        assert!(matches!(err, AuditError::SchemaValidation { .. }));
    }

    #[test]
    fn non_object_what_is_rejected() {
        let err = validate_what(AuditReason::KbVersionInstalled, &json!("x")).unwrap_err();
        assert!(matches!(err, AuditError::SchemaValidation { .. }));
    }

    #[test]
    fn non_case_scoped_allows_missing_case_id() {
        // KB / crisis / fallback reasons are not case-scoped.
        let got = validate_what(
            AuditReason::KbVersionInstalled,
            &json!({"version_label": "2026.05"}),
        )
        .unwrap();
        assert_eq!(got, None);
    }
}
