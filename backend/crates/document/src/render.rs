//! Backend-agnostic document intermediate representation (`RenderDoc`).
//!
//! `run_all_layers` injects the four GB 45438 marking layers onto a `RenderDoc`, then `pdf.rs`
//! turns it into the final PDF bytes. Decoupling the IR from the concrete PDF backend keeps the
//! four-layer logic unit-testable without any native renderer (compliance/01 §6.4).

use serde::{Deserialize, Serialize};

/// Stable reference to a rendered paragraph; mirrors `AiSegment.paragraph_ref` so the zero-width
/// frame (Layer 3) is injected before the right paragraph (compliance/01 §5.1.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParagraphRef(pub u64);

/// A single block in the ordered document flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    /// Document title (centred, gongwen title font chain).
    Title,
    /// A heading / section title.
    Heading,
    /// A normal body paragraph.
    Paragraph,
    /// A signature / closing block (落款, KaiTi font chain).
    Signature,
    /// The hard-coded Layer 1 cover declaration page (§3.2). Not user-editable.
    CoverDeclaration,
    /// A per-page running header line (§3.1 "本文部分内容由 AI 辅助生成 …").
    PageHeader,
}

/// One block of the document flow. `paragraph_ref` is `Some` only for AI-drafted body paragraphs
/// that carry a zero-width frame + segment上标 (Layer 1 / Layer 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderBlock {
    pub kind: BlockKind,
    /// Visible text of the block. Layer 1 上标 markers and Layer 3 zero-width frames are prepended
    /// in place (the frame chars are invisible; the 上标 markers are short visible literals).
    pub text: String,
    pub paragraph_ref: Option<ParagraphRef>,
    /// True once the Layer-1 `[AI 草拟]` / `[AI 草拟止]` 上标 have been applied to this block.
    pub ai_marked: bool,
    /// The Layer-3 zero-width frame prepended to this block (empty until injected).
    pub zerowidth_frame: String,
}

impl RenderBlock {
    /// Construct a plain block with no AI marking.
    pub fn new(kind: BlockKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            paragraph_ref: None,
            ai_marked: false,
            zerowidth_frame: String::new(),
        }
    }

    /// Construct an AI-drafted body paragraph bound to a `paragraph_ref`.
    pub fn ai_paragraph(text: impl Into<String>, paragraph_ref: ParagraphRef) -> Self {
        Self {
            kind: BlockKind::Paragraph,
            text: text.into(),
            paragraph_ref: Some(paragraph_ref),
            ai_marked: false,
            zerowidth_frame: String::new(),
        }
    }
}

/// Page layout metadata (A4 + binding gutter + auto TOC — Prototype/document.html is the typography
/// authority; only the load-bearing fields are modelled here for R1a).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageMeta {
    /// Page width in PDF points (A4 = 595).
    pub width_pt: u32,
    /// Page height in PDF points (A4 = 842).
    pub height_pt: u32,
    /// Whether a per-page running header has been rendered (Layer 1 self-check anchor).
    pub header_present: bool,
}

impl Default for PageMeta {
    fn default() -> Self {
        Self {
            width_pt: 595,
            height_pt: 842,
            header_present: false,
        }
    }
}

/// One PDF Info Dict entry (Layer 2). Order is preserved for deterministic output.
pub type InfoEntry = (String, String);

/// Accumulated PDF Info Dict (Layer 2, §4.1.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoDict {
    pub entries: Vec<InfoEntry>,
}

impl InfoDict {
    /// Set or overwrite a key (last write wins; insertion order kept).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Look up a key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Accumulated XMP packet (Layer 2, §4.1.2). Stored as the serialised RDF/XML body so `pdf.rs`
/// can embed it verbatim and `verify` can substring-match the required fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmpPacket {
    pub xml: String,
}

/// The document intermediate representation. The four layers mutate this in place; `pdf.rs` reads
/// it to produce the final bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderDoc {
    pub blocks: Vec<RenderBlock>,
    pub page_meta: PageMeta,
    pub info_dict: InfoDict,
    pub xmp: XmpPacket,
    /// PNG bytes of the dossier cover image carrying the LSB watermark (Layer 3-lsb). `None` until
    /// `lsb_pdf::embed_cover` runs; `pdf.rs` embeds it as the first-page image XObject.
    pub cover_png: Option<Vec<u8>>,
}

impl RenderDoc {
    /// Prepend an invisible zero-width run to the AI paragraph bound to `paragraph_ref` (Layer 3
    /// injection point, compliance/01 §5.1.2.1). Stores the frame on the block; `pdf.rs` writes it
    /// into the text stream before the visible glyphs.
    pub fn prepend_invisible_run(&mut self, paragraph_ref: ParagraphRef, frame: &str) {
        for block in &mut self.blocks {
            if block.paragraph_ref == Some(paragraph_ref) {
                block.zerowidth_frame = frame.to_string();
            }
        }
    }

    /// All AI-drafted body blocks (those bound to a `paragraph_ref`).
    pub fn ai_blocks(&self) -> impl Iterator<Item = &RenderBlock> {
        self.blocks.iter().filter(|b| b.paragraph_ref.is_some())
    }

    /// Whether the hard-coded Layer-1 cover declaration block is present.
    pub fn has_cover(&self) -> bool {
        self.blocks
            .iter()
            .any(|b| b.kind == BlockKind::CoverDeclaration)
    }

    /// Whether at least one per-page header block is present.
    pub fn has_header(&self) -> bool {
        self.blocks.iter().any(|b| b.kind == BlockKind::PageHeader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_dict_set_overwrites_and_keeps_order() {
        let mut d = InfoDict::default();
        d.set("/Producer", "Stučka 0.1.0");
        d.set("/Creator", "Stučka Document Engine");
        d.set("/Producer", "Stučka 0.2.0");
        assert_eq!(d.get("/Producer"), Some("Stučka 0.2.0"));
        assert_eq!(d.entries.len(), 2);
        assert_eq!(d.entries[0].0, "/Producer");
        assert_eq!(d.entries[1].0, "/Creator");
    }

    #[test]
    fn prepend_invisible_run_targets_paragraph() {
        let mut doc = RenderDoc::default();
        doc.blocks.push(RenderBlock::ai_paragraph(
            "一段 AI 草拟正文",
            ParagraphRef(7),
        ));
        doc.blocks
            .push(RenderBlock::new(BlockKind::Paragraph, "人工正文"));
        doc.prepend_invisible_run(ParagraphRef(7), "ZWFRAME");
        assert_eq!(doc.blocks[0].zerowidth_frame, "ZWFRAME");
        assert_eq!(doc.blocks[1].zerowidth_frame, "");
        assert_eq!(doc.ai_blocks().count(), 1);
    }
}
