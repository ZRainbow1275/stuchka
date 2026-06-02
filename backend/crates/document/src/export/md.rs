//! Markdown serialisation of a rendered document (GB-12: fact nodes emit `[fact:ID]`, law nodes
//! emit `[law:ID]`, matching the frontend `to_markdown.ts` convention).
//!
//! The zero-width frames are NOT emitted into the Markdown (they belong to the PDF text stream);
//! the Markdown is the human-readable companion in the三件套 bundle. The Layer-1 cover / header
//! become Markdown front-matter blocks so the AI-assisted notice survives the conversion.

use crate::render::{BlockKind, RenderDoc};
use crate::template::{LawRefSlot, TemplateContext};

/// Serialise a [`RenderDoc`] into Markdown. The visible 上标 markers ([AI 草拟] / [AI 草拟止]) are
/// preserved so the AI-drafted spans stay identifiable in Markdown form too (Layer 1 parity).
pub fn to_markdown(doc: &RenderDoc) -> String {
    let mut out = String::new();
    for block in &doc.blocks {
        match block.kind {
            BlockKind::Title => {
                out.push_str("# ");
                out.push_str(&block.text);
                out.push_str("\n\n");
            }
            BlockKind::Heading => {
                out.push_str("## ");
                out.push_str(&block.text);
                out.push_str("\n\n");
            }
            BlockKind::CoverDeclaration => {
                // cover becomes a blockquote front-matter so the AIGC notice is preserved
                for line in block.text.split('\n') {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            BlockKind::PageHeader => {
                out.push_str("> _");
                out.push_str(&block.text);
                out.push_str("_\n\n");
            }
            BlockKind::Paragraph | BlockKind::Signature => {
                out.push_str(&block.text);
                out.push_str("\n\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// Emit the `[law:URN]` reference list for a context (GB-12 law-node form). Used by callers that
/// want a stable law-ref appendix independent of the rendered body lines.
pub fn law_refs_markdown(law_refs: &[LawRefSlot]) -> String {
    let mut out = String::from("## 法律依据\n\n");
    for lr in law_refs {
        out.push_str(&format!("- [law:{}] {}\n", lr.urn, lr.title));
    }
    out
}

/// Emit the `[fact:ID]` reference list for a context (GB-12 fact-node form).
pub fn facts_markdown(ctx: &TemplateContext) -> String {
    let mut out = String::from("## 事实\n\n");
    for f in &ctx.facts {
        out.push_str(&format!("- [fact:{}] {}\n", f.fact_id, f.statement));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{BlockKind, RenderBlock, RenderDoc};
    use crate::template::TemplateContext;

    #[test]
    fn markdown_has_title_and_preserves_ai_markers() {
        let mut doc = RenderDoc::default();
        doc.blocks
            .push(RenderBlock::new(BlockKind::Title, "劳动仲裁申请书"));
        doc.blocks.push(RenderBlock::new(
            BlockKind::Paragraph,
            "[AI 草拟]经核算应支付二倍工资[AI 草拟止]",
        ));
        let md = to_markdown(&doc);
        assert!(md.starts_with("# 劳动仲裁申请书"));
        assert!(md.contains("[AI 草拟]"));
        assert!(md.contains("[AI 草拟止]"));
    }

    #[test]
    fn fact_and_law_nodes_use_bracket_form() {
        let ctx = TemplateContext::sample();
        let facts = facts_markdown(&ctx);
        assert!(facts.contains("[fact:F-001]"));
        let laws = law_refs_markdown(&ctx.law_refs);
        assert!(laws.contains("[law:law:中华人民共和国劳动合同法/v2012-12-28/§82/¶1]"));
    }
}
