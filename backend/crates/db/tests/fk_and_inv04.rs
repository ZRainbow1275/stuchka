//! Foreign-key enforcement (RESTRICT / CASCADE) and INV-04 KB-freeze tests.

mod common;

use common::*;
use db::{DbError, Store};

#[tokio::test]
async fn fk_restrict_blocks_orphan_fact() {
    let store = Store::open_in_memory().await.unwrap();
    // fact referencing a non-existent case must fail the FK (foreign_keys=ON).
    let orphan = sample_fact(uuid::Uuid::now_v7());
    let err = store.fact.insert(&orphan).await;
    assert!(
        err.is_err(),
        "inserting a fact with no parent case must violate the FK"
    );
}

#[tokio::test]
async fn fk_restrict_blocks_case_delete_with_children() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let f = sample_fact(c.id);
    store.fact.insert(&f).await.unwrap();

    // ON DELETE RESTRICT: deleting the parent while a fact references it must fail.
    let err = store.case.delete(&c.id).await;
    assert!(
        err.is_err(),
        "RESTRICT must block deleting a case with child facts"
    );

    // after removing the child, delete succeeds.
    store.fact.delete(&f.id).await.unwrap();
    store.case.delete(&c.id).await.unwrap();
    assert!(store.case.get(&c.id).await.unwrap().is_none());
}

#[tokio::test]
async fn fk_cascade_removes_group_contributors() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let g = sample_group(c.id);
    store.group.insert(&g).await.unwrap();
    let gc = sample_contributor(g.id);
    store.group.add_contributor(&gc).await.unwrap();
    assert_eq!(store.group.list_contributors(&g.id).await.unwrap().len(), 1);

    // ON DELETE CASCADE: deleting the group removes its contributors.
    store.group.delete(&g.id).await.unwrap();
    assert!(store
        .group
        .list_contributors(&g.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn fk_restrict_blocks_law_ref_delete_when_linked() {
    let store = Store::open_in_memory().await.unwrap();
    let c = sample_case();
    store.case.insert(&c).await.unwrap();
    let lr = sample_law_ref();
    store.law_ref.insert(&lr).await.unwrap();
    store
        .law_ref
        .link_case(&c.id, &lr.id, "calculation")
        .await
        .unwrap();

    // RESTRICT: cannot delete a law_ref that a case_law_ref references.
    let err = store.law_ref.delete(&lr.id).await;
    assert!(
        err.is_err(),
        "RESTRICT must block deleting a linked law_ref"
    );
}

#[tokio::test]
async fn inv04_kb_version_hash_is_frozen() {
    // backend/02 §2.8: updating kb_version_hash to a different value -> E_INVALID_TRANSITION.
    let store = Store::open_in_memory().await.unwrap();
    let mut c = sample_case();
    store.case.insert(&c).await.unwrap();

    c.kb_version_hash = "d".repeat(64); // attempt to change frozen hash
    let err = store.case.update(&c).await.unwrap_err();
    assert!(matches!(err, DbError::KbVersionFrozen), "got {err:?}");

    // same hash UPDATE (other field change) passes.
    c.kb_version_hash = "a".repeat(64); // original value
    c.kb_version_label = "2026-06-01-r1".to_string();
    store.case.update(&c).await.unwrap();
    assert_eq!(
        store
            .case
            .get(&c.id)
            .await
            .unwrap()
            .unwrap()
            .kb_version_label,
        "2026-06-01-r1"
    );
}
