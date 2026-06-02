//! GB 45438-2025 acceptance integration tests (compliance/01 §7.3 + prd §4.3.5).
//!
//! Covers the CI-forced assertions GB-01..GB-13 that are machine-checkable in Rust:
//!   GB-01  four-layer completeness across 6 templates × 3 scenarios + 2 fallback = 20 docs × 5 = 100
//!   GB-02  decoded zero-width segment_id set == manifest ai_generated_segments[].segment_id set
//!   GB-03  gb45438_layer_completeness all five true
//!   GB-04  a frontend draft (no four layers) is_rejected_as_final == true
//!   GB-05  6 templates embed the law-ref URN (100%)
//!   GB-06  encode_frame length == 83, decode round-trips, corrupt CRC -> None (in lib unit tests)
//!   GB-07  Layer-2 metadata readable (lopdf-level assertion fallback when exiftool absent)
//!   GB-08  run_all_layers fail-closed per layer, nothing written
//!   GB-09  export audit.sqlite has one Export record + verify_chain ok
//!   GB-10  tpl_settlement contains the non-deletable INV-10 disclaimer
//!   GB-12  Markdown emits [fact:ID] and [law:ID]

use document::gb45438::manifest::AiSegment;
use document::gb45438::metadata::DocMeta;
use document::gb45438::{self, Gb45438Error};
use document::render::ParagraphRef;
use document::template::{TemplateContext, TemplateId, INV10_SETTLEMENT_DISCLAIMER};
use document::{export_dossier, is_rejected_as_final, pdf, ExportRequest};
use lopdf::dictionary;
use uuid::Uuid;

/// Build a (context, segments, meta) triple for one (template, scenario) pair. Scenario `s` varies
/// the applicant / claims so the 20-document matrix is genuinely distinct.
fn fixture(id: TemplateId, scenario: usize) -> (TemplateContext, Vec<AiSegment>, DocMeta) {
    let mut ctx = TemplateContext::sample();
    ctx.applicant = format!("申请人{scenario}号");
    ctx.claims[0].index = 1;
    ctx.claims[0].description =
        format!("场景 {scenario}：请求裁决被申请人支付欠付工资及二倍工资差额");
    if id == TemplateId::Settlement {
        ctx.inv10_disclaimer = Some(INV10_SETTLEMENT_DISCLAIMER.to_string());
    }
    // two AI segments per doc to exercise the 1:1 set assertion with >1 element
    ctx.ai_paragraphs = vec![
        document::template::AiParagraph {
            paragraph_ref: 1,
            text: format!("AI 分析一（场景{scenario}）：依据在案证据，欠薪事实基本成立。"),
        },
        document::template::AiParagraph {
            paragraph_ref: 2,
            text: format!("AI 分析二（场景{scenario}）：二倍工资差额计算区间为入职后次月起。"),
        },
    ];
    let segs = vec![
        AiSegment::approved(
            0x1000 + scenario as u64 * 2,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek-v3.1-2026Q1",
            "a".repeat(64),
            "b".repeat(64),
            0.83,
        ),
        AiSegment::approved(
            0x1001 + scenario as u64 * 2,
            ParagraphRef(2),
            "$.body.section[0].paragraph[1]",
            "deepseek-v3.1-2026Q1",
            "c".repeat(64),
            "d".repeat(64),
            0.77,
        ),
    ];
    let mut meta = DocMeta::sample(id);
    meta.doc_id = Uuid::now_v7();
    meta.segment_count = segs.len() as u32;
    (ctx, segs, meta)
}

/// The 20-document corpus: 6 templates × 3 scenarios = 18 + 2 fallback (arb / lawsuit scenario 4).
fn corpus() -> Vec<(TemplateId, usize)> {
    let mut v = Vec::new();
    for id in TemplateId::ALL {
        for scenario in 0..3 {
            v.push((id, scenario));
        }
    }
    // 2 fallback docs
    v.push((TemplateId::ArbApplication, 3));
    v.push((TemplateId::Lawsuit, 3));
    v
}

#[test]
fn gb01_03_05_four_layer_completeness_100_percent_over_20_docs() {
    let docs = corpus();
    assert_eq!(docs.len(), 20, "GB-01 corpus must be 20 documents");

    let mut checkpoints = 0usize;
    let mut passed = 0usize;
    for (id, scenario) in docs {
        let (ctx, segs, meta) = fixture(id, scenario);
        let bundle = export_dossier(id, &ctx, &segs, &meta, &ExportRequest::default())
            .unwrap_or_else(|e| panic!("{id:?}/{scenario} export failed: {e}"));

        let c = bundle.completeness;
        // GB-03: five booleans all true.
        for ok in [
            c.layer1_explicit,
            c.layer2_metadata,
            c.layer3_zerowidth,
            c.layer3_lsb,
            c.layer4_manifest,
        ] {
            checkpoints += 1;
            if ok {
                passed += 1;
            }
        }
        c.assert_all_true().unwrap();

        // GB-05: every law-ref URN embedded.
        let full: String = bundle.markdown.clone();
        for lr in &ctx.law_refs {
            assert!(
                full.contains(&lr.urn),
                "{id:?}/{scenario}: URN missing (GB-05)"
            );
        }
    }
    // GB-01: 20 × 5 = 100 checkpoints, 100% pass.
    assert_eq!(checkpoints, 100, "GB-01 expects 100 checkpoints");
    assert_eq!(passed, 100, "GB-01 four-layer completeness must be 100%");
}

#[test]
fn gb02_zerowidth_id_set_equals_manifest_id_set() {
    for (id, scenario) in corpus() {
        let (ctx, segs, meta) = fixture(id, scenario);
        // render + inject to inspect the doc directly
        let reg = document::TemplateRegistry::load_builtin().unwrap();
        let mut doc = reg.render(id, &ctx).unwrap();
        let c = gb45438::run_all_layers(&mut doc, &segs, &meta).unwrap();
        let manifest = gb45438::build_manifest(&segs, &meta, c).unwrap();

        let zw = gb45438::zerowidth::decode_all(&doc);
        assert_eq!(
            zw,
            manifest.segment_ids(),
            "{id:?}/{scenario}: zero-width set != manifest set (GB-02)"
        );
        assert_eq!(zw.len(), 2, "two AI segments expected");
    }
}

#[test]
fn gb04_frontend_draft_is_rejected_as_final() {
    // a bare PDF lacking the four layers must be rejected.
    let mut bare = lopdf::Document::with_version("1.7");
    let pages_id = bare.new_object_id();
    let page_id = bare.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    bare.set_object(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 },
    );
    let cat = bare.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    bare.trailer.set("Root", cat);
    let mut bytes = Vec::new();
    bare.save_to(&mut bytes).unwrap();
    assert!(
        is_rejected_as_final(&bytes),
        "GB-04: draft must be rejected"
    );

    // and a real final件 must NOT be rejected
    let (ctx, segs, meta) = fixture(TemplateId::ArbApplication, 0);
    let bundle = export_dossier(
        TemplateId::ArbApplication,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .unwrap();
    assert!(!is_rejected_as_final(&bundle.pdf_bytes));
}

#[test]
fn gb07_layer2_metadata_present_in_pdf() {
    let (ctx, segs, meta) = fixture(TemplateId::Lawsuit, 1);
    let bundle = export_dossier(
        TemplateId::Lawsuit,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .unwrap();
    // GB-07 fallback: lopdf-level assertion that Info Dict + XMP markers are present (used when
    // exiftool is not on the CI host; the offline verify_pdf_metadata.py covers the exiftool path).
    assert!(pdf::pdf_has_metadata_markers(&bundle.pdf_bytes));
}

#[test]
fn gb08_fail_closed_layer1_blocks_without_cover_path() {
    // a segment with no matching paragraph -> Layer 1 blocks, nothing produced.
    let mut doc = document::render::RenderDoc::default();
    let segs = vec![AiSegment::approved(
        1,
        ParagraphRef(99),
        "p",
        "m",
        "k",
        "p",
        0.5,
    )];
    let meta = DocMeta::sample(TemplateId::ArbApplication);
    let err = gb45438::run_all_layers(&mut doc, &segs, &meta).unwrap_err();
    assert!(matches!(err, Gb45438Error::Layer1Missing));
    assert!(doc.cover_png.is_none(), "GB-08: nothing written on block");
}

#[test]
fn gb08_fail_closed_layer4_blocks_on_bad_hash() {
    let (ctx, mut segs, meta) = fixture(TemplateId::Mediation, 0);
    segs[0].kb_hash = "not-sha256".into();
    let err = export_dossier(
        TemplateId::Mediation,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .unwrap_err();
    assert!(matches!(err, Gb45438Error::ManifestSchemaInvalid));
}

#[test]
fn gb10_settlement_carries_non_deletable_inv10_disclaimer() {
    let (ctx, segs, meta) = fixture(TemplateId::Settlement, 0);
    let bundle = export_dossier(
        TemplateId::Settlement,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .unwrap();
    assert!(bundle.markdown.contains("不可删除"));
    assert!(bundle.markdown.contains("12348"));
    assert!(bundle.markdown.contains("80%"));
}

#[test]
fn gb12_markdown_has_fact_and_law_bracket_nodes() {
    let (ctx, segs, meta) = fixture(TemplateId::ArbApplication, 0);
    let bundle = export_dossier(
        TemplateId::ArbApplication,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .unwrap();
    assert!(bundle.markdown.contains("[fact:F-001]"), "GB-12 fact node");
    assert!(
        bundle.markdown.contains("law:中华人民共和国劳动合同法"),
        "GB-12 law node"
    );
}

#[tokio::test]
async fn gb09_export_writes_one_export_audit_record_and_chain_verifies() {
    let key = [3u8; 32];
    let log = audit::open_in_memory(&key).await.unwrap();
    let case_id = Uuid::now_v7();
    let (ctx, segs, meta) = fixture(TemplateId::Enforcement, 2);

    let bundle = document::export_dossier_with_audit(
        &log,
        audit::Subject::System {
            component: "document".into(),
        },
        case_id,
        TemplateId::Enforcement,
        &ctx,
        &segs,
        &meta,
        &ExportRequest::default(),
    )
    .await
    .unwrap();
    bundle.completeness.assert_all_true().unwrap();

    let records = log.query_by_case(case_id).await.unwrap();
    assert_eq!(records.len(), 1, "GB-09: exactly one Export record");
    assert_eq!(records[0].why, audit::AuditReason::DocumentExported);
    assert_eq!(records[0].why.category(), data_model::AuditCategory::Export);
    let v = log.verify_chain().await.unwrap();
    assert!(v.ok, "GB-09: audit chain must verify");
}

/// PDF rasterisation integration test via pdfium-render — needs the runtime pdfium native lib,
/// which is not bundled in this environment, so it is `#[ignore]`d (run explicitly with the
/// `pdfium` feature + a discoverable pdfium.dll). The structural PDF path is fully covered above.
#[test]
#[ignore = "needs runtime pdfium native lib (pdfium.dll); structural PDF is covered by lopdf tests"]
fn pdfium_rasterisation_smoke() {
    // Intentionally a no-op placeholder for the gated render path; enabling it requires building
    // with --features pdfium and a locatable pdfium dynamic library.
}
