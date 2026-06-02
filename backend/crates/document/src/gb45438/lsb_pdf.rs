//! Layer 3 (LSB) — embed the watermarked cover image into the document (compliance/01 §5.2.2).
//!
//! R1a embeds exactly one cover image (the dossier cover, §5.3 "1 only"). The image carries the LSB
//! payload from [`lsb_image`]; `pdf.rs` then writes the PNG as the first-page image XObject. The
//! cover PNG is stored on the `RenderDoc` so `pdf.rs` and the offline `extract_lsb.py` both see the
//! same raster.

use std::io::Cursor;

use crate::gb45438::lsb_image::{self, LsbPayload};
use crate::gb45438::metadata::DocMeta;
use crate::gb45438::Gb45438Error;
use crate::render::RenderDoc;

/// Cover canvas size (points-ish raster; large enough for a 256-bit payload with margin).
const COVER_W: u32 = 256;
const COVER_H: u32 = 256;

/// Embed the dossier cover LSB watermark and store the PNG on the doc (Layer 3-lsb). The payload
/// carries the first AI segment id (or 0 when there are no AI segments — the cover is still marked
/// so the layer self-check passes) plus the low 64 bits of `doc_id`.
pub fn embed_cover(doc: &mut RenderDoc, meta: &DocMeta) -> Result<bool, Gb45438Error> {
    let seg_id = doc
        .ai_blocks()
        .filter_map(|b| b.paragraph_ref)
        .map(|p| p.0)
        .next()
        .unwrap_or(0);
    let doc_id_low = u64::from_be_bytes(meta.doc_id.as_bytes()[8..16].try_into().unwrap());

    let mut img = lsb_image::blank_cover(COVER_W, COVER_H);
    let payload = LsbPayload::new(seg_id, doc_id_low);
    lsb_image::embed(&mut img, &payload).map_err(Gb45438Error::PdfBackend)?;

    // Serialise the cover to PNG bytes (lossless → LSB survives, §5.2.3 grade A on re-compress).
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| Gb45438Error::PdfBackend(e.to_string()))?;
    let png = buf.into_inner();

    // Self-check: decode the just-written PNG and confirm the payload round-trips.
    let reloaded = image::load_from_memory(&png)
        .map_err(|e| Gb45438Error::PdfBackend(e.to_string()))?
        .to_rgba8();
    let recovered = lsb_image::decode(&reloaded).ok_or_else(|| {
        Gb45438Error::PdfBackend("cover LSB decode failed after embed".to_string())
    })?;
    let ok = recovered == payload && recovered.checksum_ok();

    doc.cover_png = Some(png);
    Ok(ok)
}

/// Decode the cover LSB payload back from the stored PNG (offline-verify parity with
/// `extract_lsb.py`).
pub fn decode_cover(doc: &RenderDoc) -> Option<LsbPayload> {
    let png = doc.cover_png.as_ref()?;
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    lsb_image::decode(&img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ParagraphRef, RenderBlock};
    use crate::template::TemplateId;
    use uuid::Uuid;

    #[test]
    fn embed_cover_stores_png_and_round_trips() {
        let mut doc = RenderDoc::default();
        doc.blocks
            .push(RenderBlock::ai_paragraph("AI 段", ParagraphRef(0x55)));
        let mut meta = DocMeta::sample(TemplateId::ArbApplication);
        meta.doc_id = Uuid::now_v7();

        let ok = embed_cover(&mut doc, &meta).unwrap();
        assert!(ok, "cover LSB self-check must pass");
        assert!(doc.cover_png.is_some());

        let payload = decode_cover(&doc).unwrap();
        assert_eq!(payload.segment_id(), 0x55);
        assert!(payload.checksum_ok());
    }

    #[test]
    fn embed_cover_with_no_segments_still_marks() {
        let mut doc = RenderDoc::default();
        let meta = DocMeta::sample(TemplateId::Settlement);
        let ok = embed_cover(&mut doc, &meta).unwrap();
        assert!(ok);
        let payload = decode_cover(&doc).unwrap();
        assert_eq!(payload.segment_id(), 0);
    }
}
