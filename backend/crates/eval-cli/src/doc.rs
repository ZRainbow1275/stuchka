//! doc-20 gate (ai/05 §5.4): GB 45438-2025 four-layer completeness == 100% over 20 documents.
//!
//! Drives the REAL `document::export_dossier` (the UNIQUE four-layer injection path, INV-02) for
//! 6 hard templates x N scenarios + fallback = 20 documents, asserting `LayerCompleteness` is fully
//! true on every one. The lawyer-blind usability (>= 70%) is a HUMAN process gate (see the dataset
//! README + blind_rubrics_template.csv); this automated gate verifies only the structural
//! four-layer completeness the law mandates.

use document::render::ParagraphRef;
use document::{
    export_dossier, template, AiSegment, DocMeta, ExportRequest, TemplateContext, TemplateId,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::report::{CaseResult, GateReport};

#[derive(Debug, Deserialize)]
struct DocDataset {
    /// Scenario labels (e.g. ["migrant_wage_arrears","illegal_dismissal","work_injury"]).
    scenarios: Vec<String>,
    /// Extra fallback documents beyond templates x scenarios (spec doc-20 = 18 + 2).
    #[serde(default)]
    fallback: usize,
}

fn build_one(template_id: TemplateId, scenario: &str, seq: u64) -> CaseResult {
    let mut ctx = TemplateContext::sample();
    if template_id == TemplateId::Settlement {
        ctx.inv10_disclaimer = Some(template::INV10_SETTLEMENT_DISCLAIMER.to_string());
    }
    let segs = vec![AiSegment::approved(
        0x1a3f,
        ParagraphRef(1),
        "$.body.section[0].paragraph[0]",
        "deepseek-v3.1-2026Q1",
        "a".repeat(64),
        "b".repeat(64),
        0.83,
    )];
    let mut meta = DocMeta::sample(template_id);
    meta.doc_id = Uuid::now_v7();
    meta.segment_count = segs.len() as u32;

    let id = format!("{}::{scenario}#{seq}", template_id.default_filename());
    let req = ExportRequest::default();
    match export_dossier(template_id, &ctx, &segs, &meta, &req) {
        Ok(bundle) => {
            let c = &bundle.completeness;
            match c.assert_all_true() {
                Ok(()) => CaseResult::new(
                    id,
                    true,
                    "four_layer_complete",
                    "L1+L2+L3zw+L3lsb+L4 all true",
                ),
                Err(e) => CaseResult::new(
                    id,
                    false,
                    "four_layer_complete",
                    format!("incomplete: {e}"),
                ),
            }
        }
        Err(e) => CaseResult::new(id, false, "four_layer_complete", format!("export error: {e}")),
    }
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: DocDataset = crate::common::read_json(dataset)?;
    if data.scenarios.is_empty() {
        return Err("doc dataset has no scenarios".to_string());
    }
    let mut cases = Vec::new();
    let mut seq = 0u64;
    for &template_id in TemplateId::ALL.iter() {
        for scenario in &data.scenarios {
            cases.push(build_one(template_id, scenario, seq));
            seq += 1;
        }
    }
    // Fallback documents: reuse the first templates under a "fallback" label.
    for &template_id in TemplateId::ALL.iter().take(data.fallback) {
        cases.push(build_one(template_id, "fallback", seq));
        seq += 1;
    }
    Ok(GateReport::from_cases("doc", cases))
}
