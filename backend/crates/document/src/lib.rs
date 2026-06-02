//! `document` — document generation + GB 45438-2025 four-layer marking + final PDF (D5).
//!
//! This crate is the SOLE authority (master-index §0.2 D5; compliance/01 is the规范 authority) for:
//!   - the 6 hard legal document templates ([`template`], tera, real LawRef URN embedding);
//!   - the four GB 45438 marking layers + the UNIQUE injection entry
//!     [`gb45438::run_all_layers`] (fail-closed, INV-02);
//!   - the final distributable PDF ([`pdf`], lopdf — real, pure-Rust);
//!   - the三件套 dossier bundle (PDF + Markdown + JSON manifest) via [`export_dossier`];
//!   - the `Export` audit event written to the independent `audit.sqlite` (D3, `audit` feature).
//!
//! `crates/api` calls [`export_dossier`] / [`export_dossier_with_audit`]; it may NOT bypass the
//! four layers (D5). `TemplateId` lives here and is re-exported by api.

pub mod export;
pub mod gb45438;
pub mod pdf;
pub mod render;
pub mod template;

#[cfg(feature = "audit")]
pub mod audit_event;

use std::io::Write;

use serde::{Deserialize, Serialize};
// `Uuid` is only referenced by the audit-gated export path and the test module.
#[cfg(any(feature = "audit", test))]
use uuid::Uuid;

pub use gb45438::manifest::{AiSegment, Manifest, ReviewState};
pub use gb45438::metadata::{DocMeta, DocReviewState};
pub use gb45438::{Gb45438Error, LayerCompleteness};
pub use render::RenderDoc;
pub use template::{TemplateContext, TemplateId, TemplateRegistry};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "document";

/// Requested output format for the dossier bundle (backend/01 §1.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Pdf,
    Md,
    Json,
}

/// Export request (backend/01 §1.6.2 `POST /document/:id/export`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExportRequest {
    pub formats: Vec<ExportFormat>,
    pub embed_water_mark: bool,
    /// Always 4 (four layers全开, INV-02); a value < 4 is rejected (no soft path).
    pub gb45438_level: u8,
}

impl Default for ExportRequest {
    fn default() -> Self {
        Self {
            formats: vec![ExportFormat::Pdf, ExportFormat::Md, ExportFormat::Json],
            embed_water_mark: true,
            gb45438_level: 4,
        }
    }
}

/// The produced dossier bundle.
#[derive(Debug, Clone)]
pub struct DossierBundle {
    /// The zip bytes of the三件套 bundle.
    pub zip_bytes: Vec<u8>,
    /// The five-layer completeness self-check (all-true on success).
    pub completeness: LayerCompleteness,
    /// The Layer-4 manifest.
    pub manifest: Manifest,
    /// The final PDF bytes (also inside the zip).
    pub pdf_bytes: Vec<u8>,
    /// The Markdown companion (also inside the zip).
    pub markdown: String,
    /// The dossier base filename stem (e.g. `labor_arbitration_application`).
    pub filename_stem: String,
}

/// The UNIQUE dossier export orchestration (D5; api calls this, never bypasses the four layers):
/// tera render → [`gb45438::run_all_layers`] → PDF (lopdf) → MD/JSON serialise → zip the三件套.
///
/// `gb45438_level` must be 4 (four layers全开). This function performs no audit I/O so it stays
/// synchronous and unit-testable; use [`export_dossier_with_audit`] to also write the D3 event.
pub fn export_dossier(
    template_id: TemplateId,
    ctx: &TemplateContext,
    segs: &[AiSegment],
    meta: &DocMeta,
    req: &ExportRequest,
) -> Result<DossierBundle, Gb45438Error> {
    if req.gb45438_level != 4 {
        // INV-02: no path exports with fewer than four layers.
        return Err(Gb45438Error::Incomplete);
    }

    // 1. Render the document body from the template (real LawRef URN embedding, GB-05).
    let reg = TemplateRegistry::load_builtin()?;
    let mut doc = reg.render(template_id, ctx)?;

    // 2. Inject all four layers (fail-closed; the only legal injection path).
    let completeness = gb45438::run_all_layers(&mut doc, segs, meta)?;

    // 3. Build the schema-validated Layer-4 manifest.
    let manifest = gb45438::build_manifest(segs, meta, completeness)?;

    // 4. Serialise the three files.
    let pdf_bytes = pdf::render_to_pdf(&doc)?;
    let markdown = export::md::to_markdown(&doc);
    let json_bytes = export::json::to_json_bytes(&manifest)?;

    // 5. Zip the三件套 according to the requested formats (always include JSON manifest as the
    //    tamper-evidence anchor; PDF/MD per request).
    let stem = template_id
        .default_filename()
        .trim_end_matches(".pdf")
        .to_string();
    let zip_bytes = zip_bundle(&stem, &pdf_bytes, &markdown, &json_bytes, req)?;

    Ok(DossierBundle {
        zip_bytes,
        completeness,
        manifest,
        pdf_bytes,
        markdown,
        filename_stem: stem,
    })
}

/// Zip the三件套 into an in-memory archive.
fn zip_bundle(
    stem: &str,
    pdf_bytes: &[u8],
    markdown: &str,
    json_bytes: &[u8],
    req: &ExportRequest,
) -> Result<Vec<u8>, Gb45438Error> {
    use zip::write::SimpleFileOptions;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let want = |f: ExportFormat| req.formats.contains(&f);
        if want(ExportFormat::Pdf) {
            zip.start_file(format!("{stem}.pdf"), opts)
                .map_err(zip_err)?;
            zip.write_all(pdf_bytes).map_err(io_err)?;
        }
        if want(ExportFormat::Md) {
            zip.start_file(format!("{stem}.md"), opts)
                .map_err(zip_err)?;
            zip.write_all(markdown.as_bytes()).map_err(io_err)?;
        }
        // The manifest is the Layer-4 tamper-evidence anchor: always present.
        zip.start_file("manifest.json", opts).map_err(zip_err)?;
        zip.write_all(json_bytes).map_err(io_err)?;

        zip.finish().map_err(zip_err)?;
    }
    Ok(cursor.into_inner())
}

fn zip_err(e: zip::result::ZipError) -> Gb45438Error {
    Gb45438Error::PdfBackend(format!("zip: {e}"))
}
fn io_err(e: std::io::Error) -> Gb45438Error {
    Gb45438Error::PdfBackend(format!("zip io: {e}"))
}

/// Export the dossier AND write the `Export` audit event to the independent `audit.sqlite` (D3).
/// `who` is the operating subject and `case_id` is the (plaintext) case the export belongs to
/// (required by the audit schema for `DocumentExported`).
#[cfg(feature = "audit")]
#[allow(clippy::too_many_arguments)]
pub async fn export_dossier_with_audit(
    log: &audit::AuditLog,
    who: audit::Subject,
    case_id: Uuid,
    template_id: TemplateId,
    ctx: &TemplateContext,
    segs: &[AiSegment],
    meta: &DocMeta,
    req: &ExportRequest,
) -> Result<DossierBundle, Gb45438Error> {
    let bundle = export_dossier(template_id, ctx, segs, meta, req)?;
    audit_event::write_export_event(
        log,
        who,
        case_id,
        meta,
        &bundle.completeness,
        &template_id.default_filename(),
    )
    .await
    .map_err(|e| Gb45438Error::PdfBackend(format!("audit export event: {e}")))?;
    Ok(bundle)
}

/// Whether a frontend draft PDF (one lacking the four layers) is correctly rejected as a final
/// distributable file (GB-04, D5: a draft without the四层 must NOT go out). A real final件 carries
/// the Layer-2 Info Dict + XMP markers; a draft does not.
pub fn is_rejected_as_final(pdf_bytes: &[u8]) -> bool {
    !pdf::pdf_has_metadata_markers(pdf_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    use render::ParagraphRef;

    fn ctx_segs_meta(template_id: TemplateId) -> (TemplateContext, Vec<AiSegment>, DocMeta) {
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
        (ctx, segs, meta)
    }

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(CRATE_NAME, "document");
    }

    #[test]
    fn export_dossier_produces_three_file_bundle_for_all_templates() {
        for id in TemplateId::ALL {
            let (ctx, segs, meta) = ctx_segs_meta(id);
            let req = ExportRequest::default();
            let bundle = export_dossier(id, &ctx, &segs, &meta, &req).unwrap();
            bundle.completeness.assert_all_true().unwrap();

            // GB-02: zero-width id set == manifest id set.
            assert_eq!(bundle.manifest.segment_ids().len(), 1);

            // zip contains the three files
            let names = zip_entry_names(&bundle.zip_bytes);
            assert!(names.iter().any(|n| n.ends_with(".pdf")), "{id:?} pdf");
            assert!(names.iter().any(|n| n.ends_with(".md")), "{id:?} md");
            assert!(
                names.iter().any(|n| n == "manifest.json"),
                "{id:?} manifest"
            );

            // PDF is a real final file (has metadata markers) → NOT rejected as final.
            assert!(!is_rejected_as_final(&bundle.pdf_bytes), "{id:?}");

            // MD carries fact / law bracket nodes (GB-12).
            assert!(bundle.markdown.contains("[fact:F-001]"), "{id:?} fact");
            assert!(
                bundle.markdown.contains("law:中华人民共和国劳动合同法"),
                "{id:?} law"
            );
        }
    }

    #[test]
    fn gb45438_level_below_four_is_refused() {
        let (ctx, segs, meta) = ctx_segs_meta(TemplateId::ArbApplication);
        let req = ExportRequest {
            gb45438_level: 3,
            ..Default::default()
        };
        let err = export_dossier(TemplateId::ArbApplication, &ctx, &segs, &meta, &req).unwrap_err();
        assert!(matches!(err, Gb45438Error::Incomplete));
    }

    #[test]
    fn frontend_draft_without_layers_is_rejected_as_final() {
        // a bare PDF with no Info/XMP markers is a draft → must be rejected (GB-04).
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
        assert!(is_rejected_as_final(&bytes));
    }

    fn zip_entry_names(zip_bytes: &[u8]) -> Vec<String> {
        let reader = std::io::Cursor::new(zip_bytes.to_vec());
        let mut archive = zip::ZipArchive::new(reader).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect()
    }
}
