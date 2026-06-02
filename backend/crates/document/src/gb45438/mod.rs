//! GB 45438-2025 four-layer marking — the SOLE authority (compliance/01, master-index §0.2 D5).
//!
//! [`run_all_layers`] is the UNIQUE injection entry: there is no legal path that exports with an
//! incomplete set of layers (INV-02, fail-closed §10). The five-boolean [`LayerCompleteness`] must
//! be all-true or the function returns `Err` and nothing is written.

pub mod explicit;
pub mod lsb_image;
pub mod lsb_pdf;
pub mod manifest;
pub mod manifest_validator;
pub mod metadata;
pub mod zerowidth;

use manifest::AiSegment;
use metadata::DocMeta;

use crate::render::RenderDoc;

/// The five-layer completeness self-check result (compliance/01 §6.1 / §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerCompleteness {
    pub layer1_explicit: bool,
    pub layer2_metadata: bool,
    pub layer3_zerowidth: bool,
    pub layer3_lsb: bool,
    pub layer4_manifest: bool,
}

impl LayerCompleteness {
    /// All five layers satisfied → `Ok(())`; otherwise `Err(Gb45438Error::Incomplete)` (fail-closed,
    /// no soft warning — INV-02).
    pub fn assert_all_true(&self) -> Result<(), Gb45438Error> {
        if self.layer1_explicit
            && self.layer2_metadata
            && self.layer3_zerowidth
            && self.layer3_lsb
            && self.layer4_manifest
        {
            Ok(())
        } else {
            Err(Gb45438Error::Incomplete)
        }
    }

    /// The list of layer names that are NOT satisfied (drives the `E_GB45438_INCOMPLETE` detail).
    pub fn missing_layers(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if !self.layer1_explicit {
            m.push("layer1_explicit");
        }
        if !self.layer2_metadata {
            m.push("layer2_metadata");
        }
        if !self.layer3_zerowidth {
            m.push("layer3_zerowidth");
        }
        if !self.layer3_lsb {
            m.push("layer3_lsb");
        }
        if !self.layer4_manifest {
            m.push("layer4_manifest");
        }
        m
    }

    /// Project into the JSON form embedded in the manifest.
    pub fn to_json(self) -> manifest::LayerCompletenessJson {
        manifest::LayerCompletenessJson {
            layer1_explicit: self.layer1_explicit,
            layer2_metadata: self.layer2_metadata,
            layer3_zerowidth: self.layer3_zerowidth,
            layer3_lsb: self.layer3_lsb,
            layer4_manifest: self.layer4_manifest,
        }
    }
}

/// Errors from the four-layer pipeline (compliance/01 §6.4 / §10 fail-closed handling).
#[derive(Debug, thiserror::Error)]
pub enum Gb45438Error {
    /// The five completeness booleans were not all true → export forbidden (INV-02).
    #[error("GB45438 four-layer marking incomplete; export refused")]
    Incomplete,
    /// Layer 1 cover / header / segment 上标 missing → block (§10).
    #[error("GB45438 Layer 1 (explicit) marking missing")]
    Layer1Missing,
    /// Layer 2 metadata could not be (re-)written → block (§10).
    #[error("GB45438 Layer 2 (metadata) unwritable")]
    Layer2Unwritable,
    /// Layer 3 zero-width round-trip failed after one retry → block (§10).
    #[error("GB45438 Layer 3 (zero-width) round-trip failed after retry")]
    Layer3RoundtripFailed,
    /// Layer 4 manifest failed schema validation → refuse generation.
    #[error("GB45438 Layer 4 (manifest) schema invalid")]
    ManifestSchemaInvalid,
    /// Template rendering error (tera).
    #[error("template render error: {0}")]
    TemplateRender(String),
    /// PDF / image backend error.
    #[error("pdf backend error: {0}")]
    PdfBackend(String),
}

/// The UNIQUE four-layer injection entry (compliance/01 §6.4, D5). Runs L1→L4 with the §10
/// fail-closed handling and returns the all-true [`LayerCompleteness`] or an `Err`. On success the
/// caller (`export_dossier`) emits the audit Export event; this function performs no I/O so it
/// stays synchronous and unit-testable.
pub fn run_all_layers(
    doc: &mut RenderDoc,
    segs: &[AiSegment],
    meta: &DocMeta,
) -> Result<LayerCompleteness, Gb45438Error> {
    // ---- Layer 1: explicit (cover + headers + segment 上标) ----
    explicit::render_cover(doc, meta)?;
    explicit::render_headers(doc, meta)?;
    explicit::mark_segments(doc, segs)?;
    let layer1 = explicit::verify(doc)?;
    if !layer1 {
        // §10: Layer 1 missing → block.
        return Err(Gb45438Error::Layer1Missing);
    }

    // ---- Layer 2: metadata (Info Dict + XMP). §10: on miss, auto re-write once, else block. ----
    metadata::write_pdf_info_and_xmp(doc, meta)?;
    let mut layer2 = metadata::verify(doc)?;
    if !layer2 {
        metadata::write_pdf_info_and_xmp(doc, meta)?; // auto补写
        layer2 = metadata::verify(doc)?;
    }
    if !layer2 {
        return Err(Gb45438Error::Layer2Unwritable);
    }

    // ---- Layer 3 (zero-width). §10: on miss, retry once, else block. ----
    zerowidth::inject_into_segments(doc, segs);
    let mut layer3_zw = zerowidth::verify_roundtrip(doc, segs)?;
    if !layer3_zw {
        zerowidth::inject_into_segments(doc, segs); // retry 1
        layer3_zw = zerowidth::verify_roundtrip(doc, segs)?;
    }
    if !layer3_zw {
        return Err(Gb45438Error::Layer3RoundtripFailed);
    }

    // ---- Layer 3 (LSB cover, 1 image). ----
    let layer3_lsb = lsb_pdf::embed_cover(doc, meta)?;

    // ---- Layer 4: manifest (schema-validated; missing → block directly). ----
    let mut built = manifest::build(segs, meta)?;
    let completeness = LayerCompleteness {
        layer1_explicit: layer1,
        layer2_metadata: layer2,
        layer3_zerowidth: layer3_zw,
        layer3_lsb,
        layer4_manifest: true,
    };
    built.gb45438_layer_completeness = completeness.to_json();
    manifest_validator::validate(&built)?;

    // Five-of-five or refuse to write (INV-02, no soft warning).
    completeness.assert_all_true()?;
    Ok(completeness)
}

/// Build the schema-validated manifest for the document (used by `export_dossier` to serialise the
/// Layer-4 JSON after `run_all_layers` has confirmed completeness).
pub fn build_manifest(
    segs: &[AiSegment],
    meta: &DocMeta,
    completeness: LayerCompleteness,
) -> Result<manifest::Manifest, Gb45438Error> {
    let mut m = manifest::build(segs, meta)?;
    m.gb45438_layer_completeness = completeness.to_json();
    manifest_validator::validate(&m)?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ParagraphRef, RenderBlock};
    use crate::template::TemplateId;

    fn doc_and_segs(n: u64) -> (RenderDoc, Vec<AiSegment>, DocMeta) {
        let mut doc = RenderDoc::default();
        let mut segs = Vec::new();
        for i in 0..n {
            doc.blocks.push(RenderBlock::ai_paragraph(
                format!("AI 段 {i}"),
                ParagraphRef(i),
            ));
            segs.push(AiSegment::approved(
                0x1000 + i,
                ParagraphRef(i),
                format!("$.body.section[0].paragraph[{i}]"),
                "deepseek-v3.1-2026Q1",
                "a".repeat(64),
                "b".repeat(64),
                0.9,
            ));
        }
        let mut meta = DocMeta::sample(TemplateId::ArbApplication);
        meta.segment_count = n as u32;
        (doc, segs, meta)
    }

    #[test]
    fn run_all_layers_all_true_on_well_formed_doc() {
        let (mut doc, segs, meta) = doc_and_segs(3);
        let c = run_all_layers(&mut doc, &segs, &meta).unwrap();
        assert!(c.layer1_explicit && c.layer2_metadata && c.layer3_zerowidth);
        assert!(c.layer3_lsb && c.layer4_manifest);
        c.assert_all_true().unwrap();

        // GB-02: decoded zero-width id set == manifest segment id set.
        let m = build_manifest(&segs, &meta, c).unwrap();
        let zw = zerowidth::decode_all(&doc);
        assert_eq!(zw, m.segment_ids());
    }

    #[test]
    fn assert_all_true_missing_lists_layers() {
        let c = LayerCompleteness {
            layer1_explicit: true,
            layer2_metadata: false,
            layer3_zerowidth: true,
            layer3_lsb: false,
            layer4_manifest: true,
        };
        assert!(matches!(c.assert_all_true(), Err(Gb45438Error::Incomplete)));
        assert_eq!(c.missing_layers(), vec!["layer2_metadata", "layer3_lsb"]);
    }

    #[test]
    fn fail_closed_layer1_when_segment_paragraph_absent() {
        // a segment whose paragraph_ref has no matching block → Layer 1 blocks.
        let mut doc = RenderDoc::default();
        let segs = vec![AiSegment::approved(
            1,
            ParagraphRef(7),
            "p",
            "m",
            "k",
            "p",
            0.5,
        )];
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        let err = run_all_layers(&mut doc, &segs, &meta).unwrap_err();
        assert!(matches!(err, Gb45438Error::Layer1Missing));
        // nothing usable was produced (no cover image embedded)
        assert!(doc.cover_png.is_none());
    }

    #[test]
    fn fail_closed_layer4_on_bad_segment_hash() {
        let (mut doc, mut segs, meta) = doc_and_segs(1);
        segs[0].kb_hash = "short".into(); // invalid sha256 hex → Layer 4 schema invalid
        let err = run_all_layers(&mut doc, &segs, &meta).unwrap_err();
        assert!(matches!(err, Gb45438Error::ManifestSchemaInvalid));
    }
}
