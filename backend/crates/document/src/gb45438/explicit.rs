//! Layer 1 — explicit visual marking: cover declaration + per-page header + paragraph 上标
//! (compliance/01 §3). Hard-coded cover text (§3.2), not user-editable.

use crate::gb45438::manifest::AiSegment;
use crate::gb45438::metadata::DocMeta;
use crate::gb45438::Gb45438Error;
use crate::render::{BlockKind, RenderBlock, RenderDoc};

/// Opening 上标 marker placed before an AI-drafted paragraph (compliance/01 §3.1).
pub const AI_MARK_OPEN: &str = "[AI 草拟]";
/// Closing 上标 marker placed after an AI-drafted paragraph.
pub const AI_MARK_CLOSE: &str = "[AI 草拟止]";

/// Render the hard-coded first-page declaration (compliance/01 §3.2). The text is fixed (legal-aid
/// hotline 12348 + GB/T 45438-2025 + 深度合成规定 §11) and must not be user-editable.
pub fn render_cover(doc: &mut RenderDoc, meta: &DocMeta) -> Result<(), Gb45438Error> {
    let review = match meta.review_state_doc {
        crate::gb45438::metadata::DocReviewState::Unchecked => "未审阅",
        crate::gb45438::metadata::DocReviewState::Partial => "部分审阅",
        crate::gb45438::metadata::DocReviewState::Full => "全部审阅",
    };
    let cover = format!(
        concat!(
            "本文档由 Stučka（劳动纠纷智能体系统）辅助生成。\n",
            "依据 GB/T 45438-2025 与《互联网信息服务深度合成管理规定》第 11 条，\n",
            "本文档存在由人工智能辅助生成的段落（详见尾页“AI 生成段落清单”）。\n\n",
            "本文档的法律效力须经使用者本人审阅、确认后承担。\n",
            "本系统对本文档的具体内容不承担法律责任，\n",
            "具体争议建议咨询执业律师或当地法律援助机构（全国法援热线：12348）。\n\n",
            "文档生成时间：{time}\n",
            "知识库版本：{kb} / {date}\n",
            "用户审阅状态：{review}"
        ),
        time = meta.generated_at.to_rfc3339(),
        kb = meta.kb_hash_short(),
        date = meta.kb_release_date,
        review = review,
    );
    // The cover is a single dedicated block at the top of the flow.
    if doc.has_cover() {
        // already rendered (idempotent) — overwrite text
        if let Some(b) = doc
            .blocks
            .iter_mut()
            .find(|b| b.kind == BlockKind::CoverDeclaration)
        {
            b.text = cover;
        }
    } else {
        doc.blocks
            .insert(0, RenderBlock::new(BlockKind::CoverDeclaration, cover));
    }
    Ok(())
}

/// Render the per-page running header (compliance/01 §3.1: 14pt+, "本文部分内容由 AI 辅助生成" +
/// project name + generation time + review state). Stored as a single header block; `pdf.rs`
/// repeats it on every page.
pub fn render_headers(doc: &mut RenderDoc, meta: &DocMeta) -> Result<(), Gb45438Error> {
    let header = format!(
        "本文部分内容由 AI 辅助生成 · Stučka · {} · 审阅状态：{}",
        meta.generated_at.to_rfc3339(),
        meta.review_state_doc.as_str(),
    );
    if let Some(b) = doc
        .blocks
        .iter_mut()
        .find(|b| b.kind == BlockKind::PageHeader)
    {
        b.text = header;
    } else {
        // header goes right after the cover (index depends on whether cover exists)
        let idx = if doc.has_cover() { 1 } else { 0 };
        doc.blocks
            .insert(idx, RenderBlock::new(BlockKind::PageHeader, header));
    }
    doc.page_meta.header_present = true;
    Ok(())
}

/// Mark each AI paragraph with the opening / closing 上标 (compliance/01 §3.1). Idempotent: a block
/// already marked is left untouched.
pub fn mark_segments(doc: &mut RenderDoc, segs: &[AiSegment]) -> Result<(), Gb45438Error> {
    for seg in segs {
        let block = doc
            .blocks
            .iter_mut()
            .find(|b| b.paragraph_ref == Some(seg.paragraph_ref));
        let Some(block) = block else {
            // a segment with no matching paragraph → Layer 1 cannot mark it → block (§10)
            return Err(Gb45438Error::Layer1Missing);
        };
        if !block.ai_marked {
            block.text = format!("{AI_MARK_OPEN}{}{AI_MARK_CLOSE}", block.text);
            block.ai_marked = true;
        }
    }
    Ok(())
}

/// Layer-1 self-check (compliance/01 §6.4): cover + header present and every AI block marked.
pub fn verify(doc: &RenderDoc) -> Result<bool, Gb45438Error> {
    if !doc.has_cover() || !doc.has_header() {
        return Ok(false);
    }
    for block in doc.ai_blocks() {
        if !block.ai_marked
            || !block.text.contains(AI_MARK_OPEN)
            || !block.text.contains(AI_MARK_CLOSE)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ParagraphRef;
    use crate::template::TemplateId;

    fn doc_with_ai_block() -> (RenderDoc, Vec<AiSegment>) {
        let mut doc = RenderDoc::default();
        doc.blocks.push(RenderBlock::ai_paragraph(
            "AI 草拟的事实陈述",
            ParagraphRef(1),
        ));
        let segs = vec![AiSegment::approved(
            1,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek-v3.1-2026Q1",
            "a".repeat(64),
            "b".repeat(64),
            0.9,
        )];
        (doc, segs)
    }

    #[test]
    fn cover_header_and_marks_pass_verify() {
        let (mut doc, segs) = doc_with_ai_block();
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        render_cover(&mut doc, &meta).unwrap();
        render_headers(&mut doc, &meta).unwrap();
        mark_segments(&mut doc, &segs).unwrap();
        assert!(verify(&doc).unwrap());

        let cover = doc
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::CoverDeclaration)
            .unwrap();
        assert!(cover.text.contains("12348"));
        assert!(cover.text.contains("GB/T 45438-2025"));
        assert!(cover.text.contains("第 11 条"));
    }

    #[test]
    fn missing_cover_fails_verify() {
        let (mut doc, segs) = doc_with_ai_block();
        let meta = DocMeta::sample(TemplateId::Mediation);
        render_headers(&mut doc, &meta).unwrap();
        mark_segments(&mut doc, &segs).unwrap();
        // no cover → fail
        assert!(!verify(&doc).unwrap());
    }

    #[test]
    fn mark_segments_is_idempotent() {
        let (mut doc, segs) = doc_with_ai_block();
        mark_segments(&mut doc, &segs).unwrap();
        let once = doc.blocks[0].text.clone();
        mark_segments(&mut doc, &segs).unwrap();
        assert_eq!(doc.blocks[0].text, once, "double mark must not duplicate");
        assert_eq!(once.matches(AI_MARK_OPEN).count(), 1);
    }

    #[test]
    fn segment_without_paragraph_blocks_layer1() {
        let mut doc = RenderDoc::default();
        let segs = vec![AiSegment::approved(
            9,
            ParagraphRef(99),
            "p",
            "m",
            "k",
            "p",
            0.5,
        )];
        let err = mark_segments(&mut doc, &segs).unwrap_err();
        assert!(matches!(err, Gb45438Error::Layer1Missing));
    }
}
