//! Application-layer state-machine guard tests (data/01 §1.1.3/§1.2.3/§1.3.4/§1.5.3).
//!
//! The repos call data-model::validate_*_transition; illegal moves must return
//! DbError::InvalidTransition (-> E_INVALID_TRANSITION).

mod common;

use common::*;
use data_model::enums::{CaseStatus, ClaimStatus, EvidenceStatus, FactStatus};
use db::{DbError, Store};

#[tokio::test]
async fn case_illegal_transition_rejected() {
    let store = Store::open_in_memory().await.unwrap();
    let mut c = sample_case();
    store.case.insert(&c).await.unwrap();

    // draft -> confirmed is illegal (must go through diagnosed)
    c.status = CaseStatus::Confirmed;
    let err = store.case.update(&c).await.unwrap_err();
    assert!(matches!(err, DbError::InvalidTransition(_)), "got {err:?}");

    // legal: draft -> diagnosed
    c.status = CaseStatus::Diagnosed;
    store.case.update(&c).await.unwrap();
    assert_eq!(
        store.case.get(&c.id).await.unwrap().unwrap().status,
        CaseStatus::Diagnosed
    );
}

#[tokio::test]
async fn fact_illegal_transition_rejected() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let mut f = sample_fact(c.id);
    store.fact.insert(&f).await.unwrap();

    // pending -> deprecated is illegal
    f.status = FactStatus::Deprecated;
    let err = store.fact.update(&f).await.unwrap_err();
    assert!(matches!(err, DbError::InvalidTransition(_)), "got {err:?}");
}

#[tokio::test]
async fn evidence_illegal_transition_rejected() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let mut e = sample_evidence(c.id);
    store.evidence.insert(&e).await.unwrap();

    // uploaded -> scored skips parsed (illegal)
    e.status = EvidenceStatus::Scored;
    let err = store.evidence.update(&e).await.unwrap_err();
    assert!(matches!(err, DbError::InvalidTransition(_)), "got {err:?}");
}

#[tokio::test]
async fn claim_withdraw_without_inv10_rejected() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let mut cl = sample_claim(c.id, "100.00");
    store.claim.insert(&cl).await.unwrap();

    // draft -> finalized (legal)
    cl.status = ClaimStatus::Finalized;
    store.claim.update(&cl).await.unwrap();

    // finalized -> withdrawn WITHOUT inv10 confirmation must be rejected (INV-10)
    cl.status = ClaimStatus::Withdrawn;
    cl.withdraw_inv10_confirmed = false;
    let err = store.claim.update(&cl).await.unwrap_err();
    assert!(matches!(err, DbError::InvalidTransition(_)), "got {err:?}");

    // with confirmation it succeeds
    cl.withdraw_inv10_confirmed = true;
    store.claim.update(&cl).await.unwrap();
    assert_eq!(
        store.claim.get(&cl.id).await.unwrap().unwrap().status,
        ClaimStatus::Withdrawn
    );
}

#[tokio::test]
async fn case_frozen_is_terminal() {
    let store = Store::open_in_memory().await.unwrap();
    let mut c = sample_case();
    // walk draft -> diagnosed -> confirmed -> frozen
    store.case.insert(&c).await.unwrap();
    c.status = CaseStatus::Diagnosed;
    store.case.update(&c).await.unwrap();
    c.status = CaseStatus::Confirmed;
    store.case.update(&c).await.unwrap();
    c.status = CaseStatus::Frozen;
    c.frozen_at = Some(chrono::Utc::now());
    store.case.update(&c).await.unwrap();

    // any move out of frozen is rejected as terminal
    c.status = CaseStatus::Draft;
    c.frozen_at = None;
    let err = store.case.update(&c).await.unwrap_err();
    assert!(matches!(err, DbError::InvalidTransition(_)), "got {err:?}");
}
