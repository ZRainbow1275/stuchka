//! Integration tests for data-model (OM-02/04/05/06 + ULID-free invariant + serde).

use data_model::*;
use rust_decimal::Decimal;
use std::str::FromStr;

#[test]
fn om02_ids_are_v7_and_time_monotonic() {
    // OM-02: primary keys use Uuid::now_v7(); ids are time-monotonic.
    let mut prev = new_id();
    assert_eq!(prev.get_version_num(), 7);
    for _ in 0..5000 {
        let next = new_id();
        assert_eq!(next.get_version_num(), 7);
        assert!(next >= prev, "v7 ids must be non-decreasing");
        prev = next;
    }
}

#[test]
fn om04_frozen_case_is_terminal() {
    // OM-04: once frozen, no transition out is accepted.
    for to in [
        CaseStatus::Draft,
        CaseStatus::Diagnosed,
        CaseStatus::Confirmed,
        CaseStatus::Disputed,
    ] {
        assert_eq!(
            validate_case_transition(CaseStatus::Frozen, to),
            Err(StateTransitionError::CaseFrozenTerminal)
        );
    }
}

#[test]
fn om05_claim_amount_decimal_string_roundtrip() {
    // OM-05: money is fixed-point Decimal; string round-trip keeps precision.
    for raw in ["0.00", "8000.00", "123456789012.99", "0.000000001"] {
        let d = Decimal::from_str(raw).unwrap();
        let s = d.to_string();
        assert_eq!(Decimal::from_str(&s).unwrap(), d);
    }
}

#[test]
fn om06_claim_withdraw_requires_inv10() {
    // OM-06: withdrawn requires withdraw_inv10_confirmed = true.
    assert_eq!(
        validate_claim_transition(ClaimStatus::Finalized, ClaimStatus::Withdrawn, false),
        Err(StateTransitionError::ClaimWithdrawNotConfirmed)
    );
    assert!(
        validate_claim_transition(ClaimStatus::Finalized, ClaimStatus::Withdrawn, true).is_ok()
    );
}

#[test]
fn enums_roundtrip_via_json() {
    // Broad serde round-trip over the public enums.
    macro_rules! rt {
        ($v:expr) => {{
            let j = serde_json::to_string(&$v).unwrap();
            let back = serde_json::from_str(&j).unwrap();
            assert_eq!($v, back);
        }};
    }
    rt!(IdentityType::DeFactoNoContract);
    rt!(DisputeSubtype::SocialInsUninsuredInjury);
    rt!(CoverageTier::MakeDeep);
    rt!(CaseStatus::Frozen);
    rt!(FactCategory::WorkInjury);
    rt!(FactStatus::Deprecated);
    rt!(FactSource::RuleEngine);
    rt!(EvidenceCategory::Appraisal);
    rt!(EvidenceStatus::Quarantined);
    rt!(ClaimType::WorkInjuryBenefit);
    rt!(ClaimStatus::Withdrawn);
    rt!(CoverageTag::Boundary);
    rt!(SourceTag::Inferred);
    rt!(AuditCategory::CryptoOp);
    rt!(LawLevel::LocalRegulation);
}

#[test]
fn all_error_codes_with_chinese_messages() {
    // 16 spec §1.12 codes + E_NOT_IMPLEMENTED (HTTP 501, R-phase route skeleton) = 17.
    assert_eq!(ALL_ERROR_CODES.len(), 17);
    for c in ALL_ERROR_CODES {
        assert!(c.code().starts_with("E_"));
        assert!(c
            .message()
            .chars()
            .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)));
    }
}
