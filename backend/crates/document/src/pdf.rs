//! Final PDF generation (D5) — real PDF bytes via `lopdf` (pure Rust, no native lib at build time).
//!
//! `lopdf` builds the document structure, content streams, the Layer-2 Info Dict (trailer) + XMP
//! metadata stream, and embeds the Layer-3 LSB cover image as a page-1 image XObject. The
//! zero-width frames (Layer 3-zw) are written into the text content stream so a PDF copy-paste
//! preserves them (compliance/01 §5.1.4 grade A).
//!
//! `pdfium-render` (the `pdfium` feature) is the optional rasterisation / verification seam; it
//! needs the runtime pdfium native lib, so that integration test is `#[ignore]` when the lib is
//! absent. The structural PDF here is fully real and does not require it.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

use crate::gb45438::Gb45438Error;
use crate::render::{BlockKind, RenderDoc};

/// Render the `RenderDoc` (after `run_all_layers`) into final PDF bytes (D5). Produces a valid
/// single-stream PDF carrying the cover declaration, per-page header, body (with zero-width frames
/// embedded in the text operators), the Layer-2 Info Dict + XMP, and the Layer-3 LSB cover image.
pub fn render_to_pdf(doc: &RenderDoc) -> Result<Vec<u8>, Gb45438Error> {
    let mut pdf = Document::with_version("1.7");

    // ---- Base Helvetica font (Type1 standard 14 font; no embedding needed). ----
    let font_id = pdf.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // ---- Build the page content stream. ----
    let mut ops: Vec<Operation> = Vec::new();
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![Object::Name(b"F1".to_vec()), 12.into()],
    ));
    // start near the top of the A4 page
    let mut y = (doc.page_meta.height_pt as i64) - 60;
    ops.push(Operation::new("Td", vec![50.into(), Object::Integer(y)]));

    for block in &doc.blocks {
        // The visible line + (for AI paragraphs) the invisible zero-width frame prefix. Both go
        // into the text stream so PDF copy-paste recovers the frame (§5.1.4).
        let line = match block.kind {
            BlockKind::CoverDeclaration => format!("[GB45438 COVER] {}", oneline(&block.text)),
            BlockKind::PageHeader => format!("[HEADER] {}", oneline(&block.text)),
            _ => block.text.clone(),
        };
        let shown = if block.zerowidth_frame.is_empty() {
            line
        } else {
            format!("{}{line}", block.zerowidth_frame)
        };
        ops.push(Operation::new(
            "Tj",
            vec![Object::string_literal(shown.into_bytes())],
        ));
        // advance the line; coarse single-line-per-block layout for R1a
        y -= 16;
        ops.push(Operation::new("Td", vec![0.into(), (-16).into()]));
        if y < 60 {
            // keep within the page for the structural R1a output
            y = (doc.page_meta.height_pt as i64) - 60;
        }
    }
    ops.push(Operation::new("ET", vec![]));

    let content = Content { operations: ops };
    let content_bytes = content
        .encode()
        .map_err(|e| Gb45438Error::PdfBackend(format!("content encode: {e}")))?;
    let content_id = pdf.add_object(Stream::new(dictionary! {}, content_bytes));

    // ---- Optional Layer-3 LSB cover image XObject (page-1 image). ----
    let mut resources = dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    };
    if let Some(png) = &doc.cover_png {
        let img_id = embed_cover_image(&mut pdf, png)?;
        resources.set("XObject", dictionary! { "CoverImg" => img_id });
    }
    let resources_id = pdf.add_object(resources);

    // ---- Page + Pages tree. ----
    let pages_id = pdf.new_object_id();
    let page_id = pdf.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![
            0.into(),
            0.into(),
            (doc.page_meta.width_pt as i64).into(),
            (doc.page_meta.height_pt as i64).into(),
        ],
    });
    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    pdf.set_object(pages_id, pages);

    // ---- XMP metadata stream (Layer 2) attached to the catalog. ----
    let metadata_id = pdf.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        doc.xmp.xml.clone().into_bytes(),
    ));

    let catalog_id = pdf.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    });
    pdf.trailer.set("Root", catalog_id);

    // ---- Layer-2 Info Dict (trailer /Info). ----
    let mut info = lopdf::Dictionary::new();
    for (key, value) in &doc.info_dict.entries {
        // strip the leading '/' so the Dictionary key is the bare name
        let name = key.strip_prefix('/').unwrap_or(key);
        info.set(name, Object::string_literal(value.clone()));
    }
    let info_id = pdf.add_object(info);
    pdf.trailer.set("Info", info_id);

    let mut out = Vec::new();
    pdf.save_to(&mut out)
        .map_err(|e| Gb45438Error::PdfBackend(format!("save: {e}")))?;
    Ok(out)
}

/// Embed the PNG cover as an RGB image XObject (Layer 3-lsb carrier). The PNG is decoded to raw RGB
/// samples so the image is a self-contained PDF object (no PNG filter dependency in the reader).
fn embed_cover_image(pdf: &mut Document, png: &[u8]) -> Result<lopdf::ObjectId, Gb45438Error> {
    let img = image::load_from_memory(png)
        .map_err(|e| Gb45438Error::PdfBackend(format!("cover decode: {e}")))?
        .to_rgb8();
    let (w, h) = (img.width(), img.height());
    let raw = img.into_raw(); // RGB8 interleaved

    let stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => w as i64,
            "Height" => h as i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        raw,
    );
    Ok(pdf.add_object(stream))
}

/// Flatten a multi-line block into a single line for the coarse R1a PDF text layout.
fn oneline(s: &str) -> String {
    s.replace('\n', " ")
}

/// Whether the structural PDF carries the Layer-2 Info Dict + XMP markers (a lopdf-level assertion
/// usable when `exiftool` is not available on the CI host — GB-07 fallback).
pub fn pdf_has_metadata_markers(pdf_bytes: &[u8]) -> bool {
    let doc = match Document::load_mem(pdf_bytes) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let info_ok = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .and_then(|id| doc.get_dictionary(id).ok())
        .map(|info| {
            info.get(b"AIGC.Standard").is_ok()
                && info.get(b"Producer").is_ok()
                && info.get(b"Keywords").is_ok()
        })
        .unwrap_or(false);
    // XMP stream present in the catalog
    let xmp_ok = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Metadata").ok())
        .is_some();
    info_ok && xmp_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb45438::{self, manifest::AiSegment, metadata::DocMeta};
    use crate::render::{ParagraphRef, RenderBlock};
    use crate::template::TemplateId;

    fn marked_doc() -> RenderDoc {
        let mut doc = RenderDoc::default();
        doc.blocks
            .push(RenderBlock::new(BlockKind::Title, "劳动仲裁申请书"));
        doc.blocks.push(RenderBlock::ai_paragraph(
            "AI 草拟的正文段落",
            ParagraphRef(1),
        ));
        let segs = vec![AiSegment::approved(
            0x42,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek",
            "a".repeat(64),
            "b".repeat(64),
            0.9,
        )];
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        gb45438::run_all_layers(&mut doc, &segs, &meta).unwrap();
        doc
    }

    #[test]
    fn render_produces_loadable_pdf_with_metadata() {
        let doc = marked_doc();
        let bytes = render_to_pdf(&doc).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "must be a PDF header");
        assert!(pdf_has_metadata_markers(&bytes), "Info+XMP must be present");
    }

    #[test]
    fn pdf_text_stream_carries_zero_width_frame() {
        let doc = marked_doc();
        let bytes = render_to_pdf(&doc).unwrap();
        // the zero-width frame bytes (U+200B etc.) must survive into the saved PDF bytes
        let frame = &doc
            .blocks
            .iter()
            .find(|b| b.paragraph_ref.is_some())
            .unwrap()
            .zerowidth_frame;
        assert!(!frame.is_empty());
        // U+2060 encodes to E2 81 A0 in UTF-8 — assert that byte sequence is in the PDF.
        let needle = "\u{2060}".as_bytes();
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle),
            "zero-width SEG_HEAD bytes missing from PDF"
        );
    }
}
