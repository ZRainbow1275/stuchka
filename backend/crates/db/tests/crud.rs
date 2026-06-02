//! Repository CRUD round-trip tests for every first-class object.

mod common;

use common::*;
use data_model::enums::{ClaimStatus, EvidenceStatus, FactStatus};
use db::Store;

#[tokio::test]
async fn case_crud_roundtrip() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let got = store.case.get(&c.id).await.unwrap().expect("case present");
    assert_eq!(got.id, c.id);
    assert_eq!(got.identity_type, c.identity_type);
    assert_eq!(got.dispute_subtype, c.dispute_subtype);
    assert_eq!(got.dispute_category, c.dispute_category);
    assert_eq!(got.case_occurred_at, c.case_occurred_at);
    assert_eq!(got.region_code, c.region_code);
    assert_eq!(got.kb_version_hash, c.kb_version_hash);
    assert_eq!(got.status, c.status);

    store.case.delete(&c.id).await.unwrap();
    assert!(store.case.get(&c.id).await.unwrap().is_none());
}

#[tokio::test]
async fn fact_crud_roundtrip() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let f = sample_fact(c.id);
    store.fact.insert(&f).await.unwrap();

    let got = store.fact.get(&f.id).await.unwrap().expect("fact present");
    assert_eq!(got.evidence_refs, f.evidence_refs);
    assert_eq!(got.confidence, f.confidence);
    assert_eq!(got.authorization_chain, f.authorization_chain);
    assert_eq!(got.coverage_tag, f.coverage_tag);

    let listed = store.fact.list_by_case(&c.id).await.unwrap();
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn evidence_crud_roundtrip_and_score_order() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let mut low = sample_evidence(c.id);
    low.effective_score = 0.10;
    low.file_hash = "1".repeat(64);
    let mut high = sample_evidence(c.id);
    high.effective_score = 0.90;
    high.file_hash = "2".repeat(64);
    store.evidence.insert(&low).await.unwrap();
    store.evidence.insert(&high).await.unwrap();

    let got = store
        .evidence
        .get(&high.id)
        .await
        .unwrap()
        .expect("evidence present");
    assert_eq!(got.chain_membership, high.chain_membership);
    assert_eq!(got.score_breakdown, high.score_breakdown);
    assert_eq!(got.high_sensitivity, high.high_sensitivity);
    assert_eq!(got.gps_coords, high.gps_coords);

    let listed = store.evidence.list_by_case(&c.id).await.unwrap();
    assert_eq!(listed.len(), 2);
    // ordered effective_score DESC
    assert!(listed[0].effective_score >= listed[1].effective_score);
}

#[tokio::test]
async fn claim_crud_roundtrip() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let cl = sample_claim(c.id, "24000.00");
    store.claim.insert(&cl).await.unwrap();

    let got = store
        .claim
        .get(&cl.id)
        .await
        .unwrap()
        .expect("claim present");
    assert_eq!(got.amount_pre_tax, cl.amount_pre_tax);
    assert_eq!(got.law_refs, cl.law_refs);
    assert_eq!(got.fact_refs, cl.fact_refs);
    assert_eq!(got.calculation_breakdown, cl.calculation_breakdown);
    assert_eq!(got.status, ClaimStatus::Draft);
}

#[tokio::test]
async fn law_ref_crud_and_case_link() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let lr = sample_law_ref();
    store.law_ref.insert(&lr).await.unwrap();

    let got = store
        .law_ref
        .get(&lr.id)
        .await
        .unwrap()
        .expect("law_ref present");
    assert_eq!(got.stable_id, lr.stable_id);
    assert_eq!(got.version_date, lr.version_date);

    store
        .law_ref
        .link_case(&c.id, &lr.id, "diagnosis")
        .await
        .unwrap();
    let linked = store.law_ref.linked_law_refs(&c.id).await.unwrap();
    assert_eq!(linked, vec![lr.id]);
}

#[tokio::test]
async fn group_crud_and_contributors() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();

    let g = sample_group(c.id);
    store.group.insert(&g).await.unwrap();
    let gc = sample_contributor(g.id);
    store.group.add_contributor(&gc).await.unwrap();

    let got = store
        .group
        .get(&g.id)
        .await
        .unwrap()
        .expect("group present");
    assert_eq!(got.initiator_id, g.initiator_id);

    let contributors = store.group.list_contributors(&g.id).await.unwrap();
    assert_eq!(contributors.len(), 1);
    assert_eq!(contributors[0].authorization_chain, gc.authorization_chain);
}

#[tokio::test]
async fn fact_update_changes_persist() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let mut f = sample_fact(c.id);
    store.fact.insert(&f).await.unwrap();

    // legal transition pending -> confirmed
    f.status = FactStatus::Confirmed;
    f.content = "更新后的事实".to_string();
    store.fact.update(&f).await.unwrap();

    let got = store.fact.get(&f.id).await.unwrap().unwrap();
    assert_eq!(got.status, FactStatus::Confirmed);
    assert_eq!(got.content, "更新后的事实");
}

#[tokio::test]
async fn evidence_update_legal_transition_persists() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let mut e = sample_evidence(c.id);
    store.evidence.insert(&e).await.unwrap();

    e.status = EvidenceStatus::Parsed;
    store.evidence.update(&e).await.unwrap();
    assert_eq!(
        store.evidence.get(&e.id).await.unwrap().unwrap().status,
        EvidenceStatus::Parsed
    );
}
