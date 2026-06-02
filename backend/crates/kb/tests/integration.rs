//! Integration tests for the `kb` crate covering the brief acceptance assertions:
//! LR-01..07 (via data-model), KB-01/02, KBC-02, KBC-03, and the BM25 search end-to-end path.

use std::collections::BTreeMap;

use chrono::{Duration, TimeZone, Utc};

use kb::{
    age_days, changed_law_refs, compute_global_hash, ensure_calculable, freshness, law_ref,
    sample_data, version, Bm25Index, CategoryCatalog, FreshnessLevel, ImpactNotice, KbManifest,
    KbVersion, LawSearch, SearchQuery, CATEGORY_TOTAL, SUBCATEGORY_TOTAL,
};

/// LR-04..07 integration: every real seed URN parses + round-trips through the data-model D8
/// parser that `kb` re-exports.
#[test]
fn lr_seed_urns_parse_and_roundtrip() {
    for sl in sample_data::SAMPLE_LAWS {
        let parts = law_ref::parse(sl.stable_id)
            .unwrap_or_else(|e| panic!("parse failed for {}: {e}", sl.stable_id));
        assert_eq!(
            law_ref::to_stable_id(&parts),
            sl.stable_id,
            "round-trip mismatch for {}",
            sl.stable_id
        );
    }
}

/// LR-01/02 integration: the seed bodies hash stably (whitespace-invariant) and substantive
/// changes flip the hash — exercised through `data-model::content_hash` as `kb` uses it.
#[test]
fn lr_content_hash_invariants_on_seed_data() {
    let body = sample_data::SAMPLE_LAWS[0].body;
    let spaced = format!("  {}  \n", body.replace('，', "， "));
    assert_eq!(
        law_ref::content_hash(body),
        law_ref::content_hash(&spaced),
        "whitespace must not change the hash (LR-01)"
    );
    let changed = body.replacen('一', "二", 1);
    if changed != body {
        assert_ne!(
            law_ref::content_hash(body),
            law_ref::content_hash(&changed),
            "substantive change must flip the hash (LR-02)"
        );
    }
}

/// KB-01: freshness boundary four points (7/8/29/30 days).
#[test]
fn kb01_freshness_boundaries() {
    let gen = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    assert_eq!(
        freshness(gen, gen + Duration::days(7)),
        FreshnessLevel::Fresh
    );
    assert_eq!(
        freshness(gen, gen + Duration::days(8)),
        FreshnessLevel::Stale
    );
    assert_eq!(
        freshness(gen, gen + Duration::days(29)),
        FreshnessLevel::Stale
    );
    assert_eq!(
        freshness(gen, gen + Duration::days(30)),
        FreshnessLevel::Expired
    );
    assert_eq!(age_days(gen, gen + Duration::days(30)), 30);
}

/// KB-02: the M9 calculation gate refuses on an expired KB (the abort hook).
#[test]
fn kb02_calculation_gate_blocks_expired() {
    let gen = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    assert!(ensure_calculable(gen, gen + Duration::days(10)).is_ok());
    let err = ensure_calculable(gen, gen + Duration::days(40)).unwrap_err();
    assert_eq!(err.code(), "E_KB_OUTDATED");
}

/// KBC-03: bundled categories.yaml is exactly 20/85 with well-formed LD codes.
#[test]
fn kbc03_categories_yaml_20_85() {
    let catalog = CategoryCatalog::bundled().expect("bundled categories.yaml valid");
    assert_eq!(catalog.category_count(), CATEGORY_TOTAL as usize);
    assert_eq!(catalog.subcategory_count(), SUBCATEGORY_TOTAL as usize);
    catalog.validate().unwrap();
}

/// KBC-02: the manifest global_hash equals the deterministic aggregate over the seed corpus, so it
/// can serve as `case.kb_version_hash` (INV-04 freeze anchor), and `KbVersion::from_manifest`
/// carries that anchor through activation.
#[test]
fn kbc02_global_hash_anchors_kb_version() {
    let hashes = sample_data::sample_content_hashes();
    let global_hash = compute_global_hash(hashes.iter().map(|(p, h)| (p.as_str(), h.as_str())));

    let manifest = KbManifest {
        version_label: "2026-05-12-r1".to_string(),
        generated_at: Utc.with_ymd_and_hms(2026, 5, 12, 8, 0, 0).unwrap(),
        global_hash: global_hash.clone(),
        schema_version: 1,
        file_count: hashes.len() as i32,
        law_count_national: 20,
        law_count_local: 2,
        category_total: CATEGORY_TOTAL,
        subcategory_total: SUBCATEGORY_TOTAL,
    };
    manifest.validate().unwrap();
    manifest
        .verify_global_hash(hashes.iter().map(|(p, h)| (p.as_str(), h.as_str())))
        .unwrap();

    // The KbVersion built from the manifest carries the same global_hash freeze anchor.
    let mut versions = vec![KbVersion::from_manifest(&manifest, Utc::now())];
    let prev = version::activate(&mut versions, "2026-05-12-r1");
    assert_eq!(prev, None);
    assert_eq!(versions[0].global_hash, global_hash);
    assert!(versions[0].is_active);
}

/// KB-01/02 + LR integration: BM25 search over the real seed corpus genuinely hits across all five
/// deep categories (Chinese tokenization works), and each hit's URN parses.
#[test]
fn bm25_search_hits_all_five_deep_categories() {
    let idx = Bm25Index::with_sample_data().expect("build index");
    assert_eq!(
        idx.doc_count().unwrap() as usize,
        sample_data::SAMPLE_LAWS.len()
    );

    let probes = [
        ("拖欠 工资", "LD-03"),          // wage arrears
        ("违法 解除 赔偿金", "LD-02"),   // illegal termination
        ("书面 劳动合同 二倍", "LD-01"), // no contract
        ("工伤 认定 事故", "LD-05"),     // work injury
        ("加班 工资 报酬", "LD-09"),     // overtime
    ];
    for (q, want_prefix) in probes {
        let hits = idx.search(&SearchQuery::keyword(q)).unwrap();
        assert!(!hits.is_empty(), "query '{q}' returned no hits");
        // top hits' URNs all parse, and at least one belongs to the expected deep category.
        let mut matched = false;
        for h in &hits {
            law_ref::parse(&h.stable_id).expect("hit URN must parse");
            let cat = sample_data::SAMPLE_LAWS
                .iter()
                .find(|l| l.stable_id == h.stable_id)
                .map(|l| l.category)
                .unwrap_or("");
            if cat.starts_with(want_prefix) {
                matched = true;
            }
        }
        assert!(
            matched,
            "query '{q}' did not surface a {want_prefix} clause"
        );
    }
}

/// Impact notice generation between two KB versions over the seed corpus (data/03 §3.7): a
/// modified clause produces an affected-law-ref entry, a frozen-anchor pair produces none.
#[test]
fn impact_notice_generation_on_version_change() {
    let old: BTreeMap<String, String> = sample_data::sample_content_hashes();
    let mut new = old.clone();
    // Simulate one clause body changing in the new version.
    let target = sample_data::SAMPLE_LAWS[0].stable_id.to_string();
    new.insert(target.clone(), "deadbeef".repeat(8));

    let changed = changed_law_refs(&old, &new);
    assert_eq!(changed, vec![target.clone()]);

    let case_id = data_model::new_id();
    let notice = ImpactNotice::new(
        case_id,
        changed,
        "2026-05-01-r1",
        "2026-06-01-r1",
        "第10条正文修订",
    );
    assert_eq!(notice.affected_law_refs, vec![target]);
    assert!(!notice.dismissed);

    // No change → no affected refs.
    assert!(changed_law_refs(&old, &old).is_empty());
}
