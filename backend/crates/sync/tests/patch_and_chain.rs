//! SY-07 / SY-08 (R1b-deep, format frozen in R1a): `.stuchka-patch` end-to-end + authorization
//! chain. Builds a real signed patch (ed25519), round-trips it through the codec, verifies the
//! header signature against the contributor key, and applies the payload to a target case doc.
//!
//! Also asserts the convergence invariant a real two-client conflict relies on: both sides' facts
//! survive a CRDT merge (SY-01's underlying "two clients edit -> 1 merged document" property).

use base64::Engine;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use yrs::{Array, Map, Transact};

use sync::{
    decode_patch, encode_patch, verify_chain, verify_patch_signature, AuthLink, CaseDoc,
    PatchHeader, PatchScope,
};

fn keypair(seed: u8) -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let peer_id = bs58::encode(sk.verifying_key().to_bytes()).into_string();
    (sk, peer_id)
}

fn sign_link(sk: &SigningKey, from: &str, to: &str, scope: &str) -> AuthLink {
    let mut link = AuthLink {
        from_user: from.into(),
        to_user: to.into(),
        scope: scope.into(),
        signed_at: Utc::now(),
        signature: String::new(),
    };
    let sig = sk.sign(&link.signing_payload());
    link.signature = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    link
}

#[test]
fn sy07_sy08_signed_patch_round_trips_and_chain_verifies_then_applies() {
    let (owner_sk, owner) = keypair(1);
    let (contrib_sk, contrib) = keypair(2);
    let case_id = Uuid::now_v7();
    let contributor_id = Uuid::now_v7();

    // Contributor builds a doc with one fact and exports a full patch.
    let contrib_doc = CaseDoc::new(&case_id.to_string());
    {
        let mut txn = contrib_doc.doc.transact_mut();
        let m = contrib_doc
            .facts
            .push_back(&mut txn, yrs::MapPrelim::default());
        m.insert(&mut txn, "summary", "未签订书面劳动合同");
    }
    let payload = contrib_doc.encode_full();

    // Header signs case_id|contributor_id|payload_sha256 with the contributor key.
    let payload_sha = {
        let mut h = Sha256::new();
        h.update(&payload);
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let signed = format!("{case_id}|{contributor_id}|{payload_sha}");
    let header_sig = base64::engine::general_purpose::STANDARD
        .encode(contrib_sk.sign(signed.as_bytes()).to_bytes());

    let header = PatchHeader {
        case_id: case_id.to_string(),
        contributor_id: contributor_id.to_string(),
        authorization_chain: vec![sign_link(&owner_sk, &owner, &contrib, "full")],
        from_kb_version: "kb-2026.05".into(),
        scope: PatchScope::Full,
        issued_at: Utc::now().to_rfc3339(),
        signature: header_sig,
        signed_fields: "case_id|contributor_id|payload_sha256".into(),
    };

    let bytes = encode_patch(&contrib_doc, PatchScope::Full, header).unwrap();

    // Importer: decode, verify header signature, verify the authorization chain, then apply.
    let (decoded_header, decoded_payload) = decode_patch(&bytes).unwrap();
    assert_eq!(decoded_header.case_id, case_id.to_string());

    verify_patch_signature(&decoded_header, &decoded_payload, &contrib)
        .expect("header signature must verify against contributor key");

    verify_chain(
        &decoded_header.authorization_chain,
        &owner,
        &contrib,
        decoded_header.scope,
        Utc::now(),
    )
    .expect("authorization chain must verify");

    // Apply into a fresh target case doc (empty) -> the fact transfers.
    let target = CaseDoc::open(&case_id.to_string(), &[]).unwrap();
    target.apply_update(&decoded_payload).unwrap();
    let txn = target.doc.transact();
    assert_eq!(target.facts.len(&txn), 1);
}

#[test]
fn sy08_wrong_contributor_key_rejects_header_signature() {
    let (owner_sk, owner) = keypair(1);
    let (contrib_sk, contrib) = keypair(2);
    let (_attacker_sk, attacker) = keypair(9);
    let case_id = Uuid::now_v7();
    let contributor_id = Uuid::now_v7();

    let doc = CaseDoc::new(&case_id.to_string());
    let payload = doc.encode_full();
    let payload_sha = {
        let mut h = Sha256::new();
        h.update(&payload);
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let signed = format!("{case_id}|{contributor_id}|{payload_sha}");
    let header_sig = base64::engine::general_purpose::STANDARD
        .encode(contrib_sk.sign(signed.as_bytes()).to_bytes());
    let header = PatchHeader {
        case_id: case_id.to_string(),
        contributor_id: contributor_id.to_string(),
        authorization_chain: vec![sign_link(&owner_sk, &owner, &contrib, "full")],
        from_kb_version: "kb".into(),
        scope: PatchScope::Full,
        issued_at: Utc::now().to_rfc3339(),
        signature: header_sig,
        signed_fields: "case_id|contributor_id|payload_sha256".into(),
    };
    let bytes = encode_patch(&doc, PatchScope::Full, header).unwrap();
    let (h, p) = decode_patch(&bytes).unwrap();

    // Verifying against the attacker's key must fail.
    assert!(verify_patch_signature(&h, &p, &attacker).is_err());
}

#[test]
fn sy01_two_client_conflict_merges_both_sides() {
    // The CRDT property SY-01 sits on: two clients editing the same case independently merge into a
    // single document containing both edits (no data loss; deterministic convergence). Each peer
    // uses a DISTINCT Yjs client id (a hard Yjs requirement — colliding ids corrupt the store).
    let a = CaseDoc::with_client_id("case-shared", 1001);
    let b = CaseDoc::with_client_id("case-shared", 2002);
    b.apply_update(&a.encode_full()).unwrap();

    // A and B each add a fact concurrently (same "facts" array — the conflict region).
    {
        let mut txn = a.doc.transact_mut();
        let m = a.facts.push_back(&mut txn, yrs::MapPrelim::default());
        m.insert(&mut txn, "summary", "A: 加班费未付");
    }
    {
        let mut txn = b.doc.transact_mut();
        let m = b.facts.push_back(&mut txn, yrs::MapPrelim::default());
        m.insert(&mut txn, "summary", "B: 违法解除");
    }

    // Exchange deltas both ways.
    let a_to_b = a.diff_update(&b.state_vector()).unwrap();
    let b_to_a = b.diff_update(&a.state_vector()).unwrap();
    b.apply_update(&a_to_b).unwrap();
    a.apply_update(&b_to_a).unwrap();

    // Converged: identical state vectors and both facts present on each side.
    assert_eq!(a.state_vector(), b.state_vector(), "must converge");
    let ta = a.doc.transact();
    let tb = b.doc.transact();
    assert_eq!(a.facts.len(&ta), 2, "both edits survive on A");
    assert_eq!(b.facts.len(&tb), 2, "both edits survive on B");
}
