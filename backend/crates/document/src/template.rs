//! Document templates — tera-rendered 6 hard legal documents (compliance/01 §3.3, backend/00 §0.2).
//!
//! `TemplateId` lives here (the document crate is the template authority; api re-exports it,
//! cross-crate reconciliation §C). Each template embeds real legal clauses by D8 LawRef URN (no
//! short codes, GB-05). `tpl_settlement` carries the non-deletable INV-10 disclaimer slot (GB-10).

use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

use crate::gb45438::Gb45438Error;
use crate::render::{BlockKind, ParagraphRef, RenderBlock, RenderDoc};

/// The 6 hard-delivered document templates (compliance/01 §7.1, prd §4.3 M3). Serialised with the
/// frontend `tpl_*_v1` ids so the api / editor JSON contract aligns (reconciliation §C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateId {
    /// 仲裁申请书.
    #[serde(rename = "arb_application")]
    ArbApplication,
    /// 调解申请书.
    #[serde(rename = "mediation")]
    Mediation,
    /// 劳动监察投诉书.
    #[serde(rename = "inspection")]
    Inspection,
    /// 和解协议（必嵌 INV-10 完整免责，slot 不可删）.
    #[serde(rename = "settlement")]
    Settlement,
    /// 起诉状（一审）.
    #[serde(rename = "lawsuit")]
    Lawsuit,
    /// 执行申请书.
    #[serde(rename = "enforcement")]
    Enforcement,
}

impl TemplateId {
    /// All six template ids.
    pub const ALL: [TemplateId; 6] = [
        TemplateId::ArbApplication,
        TemplateId::Mediation,
        TemplateId::Inspection,
        TemplateId::Settlement,
        TemplateId::Lawsuit,
        TemplateId::Enforcement,
    ];

    /// The tera template name registered for this id.
    pub fn template_name(self) -> &'static str {
        match self {
            TemplateId::ArbApplication => "tpl_arb_application.tera",
            TemplateId::Mediation => "tpl_mediation.tera",
            TemplateId::Inspection => "tpl_inspection.tera",
            TemplateId::Settlement => "tpl_settlement.tera",
            TemplateId::Lawsuit => "tpl_lawsuit.tera",
            TemplateId::Enforcement => "tpl_enforcement.tera",
        }
    }

    /// The Chinese document title.
    pub fn title(self) -> &'static str {
        match self {
            TemplateId::ArbApplication => "劳动仲裁申请书",
            TemplateId::Mediation => "劳动争议调解申请书",
            TemplateId::Inspection => "劳动保障监察投诉书",
            TemplateId::Settlement => "劳动争议和解协议",
            TemplateId::Lawsuit => "民事起诉状",
            TemplateId::Enforcement => "强制执行申请书",
        }
    }
}

/// One embedded legal clause referenced by its D8 URN (GB-05: every clause carries a stable
/// `law:…` URN, no short code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawRefSlot {
    /// D8 URN, e.g. `law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1`.
    pub urn: String,
    /// Short human title shown inline.
    pub title: String,
}

/// One claim row rendered in the document body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimItem {
    pub index: u32,
    pub description: String,
    /// Amount as a decimal string (rust_decimal string form; never f64).
    pub amount: Option<String>,
}

/// One fact row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactItem {
    pub fact_id: String,
    pub statement: String,
}

/// The rendering context (slots) for a document (compliance/01 §3.3 slot semantics, shared with the
/// frontend template JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateContext {
    pub applicant: String,
    pub respondent: String,
    pub jurisdiction: String,
    /// data/01 D9 authority: province + city (not the deprecated `region` field).
    pub province: String,
    pub city: String,
    pub claims: Vec<ClaimItem>,
    pub facts: Vec<FactItem>,
    pub law_refs: Vec<LawRefSlot>,
    pub evidence_index: Vec<String>,
    /// Markers for which paragraphs are AI-drafted (the renderer binds these to `paragraph_ref`s).
    pub ai_paragraphs: Vec<AiParagraph>,
    /// Filled for `Settlement` only: the INV-10 full disclaimer text (slot is non-deletable, GB-10).
    pub inv10_disclaimer: Option<String>,
}

/// An AI-drafted body paragraph with its stable segment id (binds the rendered block to its
/// `paragraph_ref` and zero-width frame).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiParagraph {
    pub paragraph_ref: u64,
    pub text: String,
}

impl TemplateContext {
    /// A minimal deterministic context for tests / examples.
    #[doc(hidden)]
    pub fn sample() -> Self {
        Self {
            applicant: "张三".to_string(),
            respondent: "某某科技有限公司".to_string(),
            jurisdiction: "广州市劳动人事争议仲裁委员会".to_string(),
            province: "广东省".to_string(),
            city: "广州市".to_string(),
            claims: vec![ClaimItem {
                index: 1,
                description: "请求裁决被申请人支付未签订书面劳动合同二倍工资差额".to_string(),
                amount: Some("48000.00".to_string()),
            }],
            facts: vec![FactItem {
                fact_id: "F-001".to_string(),
                statement: "申请人于 2025-03-01 入职，被申请人未签订书面劳动合同".to_string(),
            }],
            law_refs: vec![LawRefSlot {
                urn: "law:中华人民共和国劳动合同法/v2012-12-28/§82/¶1".to_string(),
                title: "劳动合同法第八十二条第一款".to_string(),
            }],
            evidence_index: vec!["E-001 工资银行流水".to_string()],
            ai_paragraphs: vec![AiParagraph {
                paragraph_ref: 1,
                text:
                    "经核算，被申请人应支付未签订书面劳动合同二倍工资差额共计人民币 48000.00 元。"
                        .to_string(),
            }],
            inv10_disclaimer: None,
        }
    }
}

/// The INV-10 full disclaimer text embedded into the settlement template (GB-10, non-deletable).
pub const INV10_SETTLEMENT_DISCLAIMER: &str = concat!(
    "【重要风险提示（不可删除）】本和解协议一经签署即对双方产生法律约束力。",
    "若本协议约定的金额低于法定应得数额的 80%，您可能因此放弃部分法定权益；",
    "签署前请务必确认：本协议系您本人在充分知悉法律后果后自愿作出的决定。",
    "如有疑问，请咨询执业律师或当地法律援助机构（全国法援热线：12348）。"
);

/// The template registry: a `tera::Tera` instance preloaded with the 6 built-in templates + slot
/// validators.
pub struct TemplateRegistry {
    tera: Tera,
}

/// Slot validation error (missing / malformed required slot).
#[derive(Debug, thiserror::Error)]
pub enum SlotError {
    #[error("required slot `{0}` is missing or empty")]
    Missing(&'static str),
    #[error("settlement template is missing the non-deletable INV-10 disclaimer slot")]
    MissingInv10Disclaimer,
    #[error("a law ref slot is not a D8 URN (must start with `law:`): {0}")]
    NonUrnLawRef(String),
}

impl From<SlotError> for Gb45438Error {
    fn from(e: SlotError) -> Self {
        Gb45438Error::TemplateRender(e.to_string())
    }
}

// The 6 built-in templates are compiled into the binary so the crate is self-contained (no runtime
// template directory needed).
const TPL_ARB: &str = include_str!("../templates/tpl_arb_application.tera");
const TPL_MEDIATION: &str = include_str!("../templates/tpl_mediation.tera");
const TPL_INSPECTION: &str = include_str!("../templates/tpl_inspection.tera");
const TPL_SETTLEMENT: &str = include_str!("../templates/tpl_settlement.tera");
const TPL_LAWSUIT: &str = include_str!("../templates/tpl_lawsuit.tera");
const TPL_ENFORCEMENT: &str = include_str!("../templates/tpl_enforcement.tera");

impl TemplateRegistry {
    /// Load the 6 built-in templates (compliance/01 §3.3). Compile-time `include_str!` keeps the
    /// crate self-contained.
    pub fn load_builtin() -> Result<Self, Gb45438Error> {
        let mut tera = Tera::default();
        tera.add_raw_templates(vec![
            (TemplateId::ArbApplication.template_name(), TPL_ARB),
            (TemplateId::Mediation.template_name(), TPL_MEDIATION),
            (TemplateId::Inspection.template_name(), TPL_INSPECTION),
            (TemplateId::Settlement.template_name(), TPL_SETTLEMENT),
            (TemplateId::Lawsuit.template_name(), TPL_LAWSUIT),
            (TemplateId::Enforcement.template_name(), TPL_ENFORCEMENT),
        ])
        .map_err(|e| Gb45438Error::TemplateRender(e.to_string()))?;
        tera.autoescape_on(vec![]); // legal docs are plain text, not HTML
        Ok(Self { tera })
    }

    /// Validate the required slots for a template (compliance/01 §3.3; GB-05 URN + GB-10 disclaimer).
    pub fn validate_slots(&self, id: TemplateId, ctx: &TemplateContext) -> Result<(), SlotError> {
        if ctx.applicant.trim().is_empty() {
            return Err(SlotError::Missing("applicant"));
        }
        // Inspection complaint targets a labor authority, not a named respondent; the rest need one.
        if id != TemplateId::Inspection && ctx.respondent.trim().is_empty() {
            return Err(SlotError::Missing("respondent"));
        }
        if ctx.law_refs.is_empty() {
            return Err(SlotError::Missing("law_refs"));
        }
        for lr in &ctx.law_refs {
            if !lr.urn.starts_with("law:") {
                return Err(SlotError::NonUrnLawRef(lr.urn.clone()));
            }
        }
        if id == TemplateId::Settlement {
            match &ctx.inv10_disclaimer {
                Some(t) if t.contains("12348") && t.contains("不可删除") => {}
                _ => return Err(SlotError::MissingInv10Disclaimer),
            }
        }
        Ok(())
    }

    /// Render a template into a [`RenderDoc`] (the body text; the four layers are injected
    /// afterwards by `run_all_layers`). AI paragraphs are bound to `paragraph_ref`s here so the
    /// zero-width / 上标 injection targets them.
    pub fn render(&self, id: TemplateId, ctx: &TemplateContext) -> Result<RenderDoc, Gb45438Error> {
        self.validate_slots(id, ctx)?;

        // For Settlement, force the canonical INV-10 disclaimer if the caller supplied a custom one
        // missing the mandatory anchors — the slot is non-deletable (GB-10).
        let mut ctx = ctx.clone();
        if id == TemplateId::Settlement {
            ctx.inv10_disclaimer = Some(INV10_SETTLEMENT_DISCLAIMER.to_string());
        }

        let mut tctx = Context::new();
        tctx.insert("title", id.title());
        tctx.insert("applicant", &ctx.applicant);
        tctx.insert("respondent", &ctx.respondent);
        tctx.insert("jurisdiction", &ctx.jurisdiction);
        tctx.insert("province", &ctx.province);
        tctx.insert("city", &ctx.city);
        tctx.insert("claims", &ctx.claims);
        tctx.insert("facts", &ctx.facts);
        tctx.insert("law_refs", &ctx.law_refs);
        tctx.insert("evidence_index", &ctx.evidence_index);
        tctx.insert("ai_paragraphs", &ctx.ai_paragraphs);
        tctx.insert("inv10_disclaimer", &ctx.inv10_disclaimer);

        let body = self
            .tera
            .render(id.template_name(), &tctx)
            .map_err(|e| Gb45438Error::TemplateRender(render_err(&e)))?;

        // GB-05 guard: every law ref URN must appear in the rendered output (100% embed rate).
        for lr in &ctx.law_refs {
            if !body.contains(&lr.urn) {
                return Err(Gb45438Error::TemplateRender(format!(
                    "law ref URN not embedded in output: {}",
                    lr.urn
                )));
            }
        }

        Ok(self.body_to_render_doc(id, &body, &ctx))
    }

    /// Split the rendered body into blocks; lines that match an AI paragraph's text become
    /// AI-bound paragraphs (carry `paragraph_ref`), the rest are plain blocks. Title / signature
    /// blocks are recognised by simple markers in the template output.
    fn body_to_render_doc(&self, id: TemplateId, body: &str, ctx: &TemplateContext) -> RenderDoc {
        let mut doc = RenderDoc::default();
        // First block is always the visible document title.
        doc.blocks
            .push(RenderBlock::new(BlockKind::Title, id.title()));

        for raw_line in body.split('\n') {
            let line = raw_line.trim_end();
            if line.trim().is_empty() {
                continue;
            }
            // Is this line an AI-drafted paragraph?
            if let Some(p) = ctx
                .ai_paragraphs
                .iter()
                .find(|p| line.contains(p.text.trim()))
            {
                doc.blocks.push(RenderBlock::ai_paragraph(
                    line.to_string(),
                    ParagraphRef(p.paragraph_ref),
                ));
            } else if line.starts_with("此致") || line.contains("申请人（签名）") {
                doc.blocks
                    .push(RenderBlock::new(BlockKind::Signature, line.to_string()));
            } else if line.starts_with("【") {
                // headings / risk-prompt markers
                doc.blocks
                    .push(RenderBlock::new(BlockKind::Heading, line.to_string()));
            } else {
                doc.blocks
                    .push(RenderBlock::new(BlockKind::Paragraph, line.to_string()));
            }
        }
        doc
    }
}

/// Flatten a tera error chain into a single message (tera nests the real cause in `source`).
fn render_err(e: &tera::Error) -> String {
    let mut msg = e.to_string();
    let mut src = std::error::Error::source(e);
    while let Some(s) = src {
        msg.push_str(" :: ");
        msg.push_str(&s.to_string());
        src = s.source();
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_templates_load_and_render() {
        let reg = TemplateRegistry::load_builtin().unwrap();
        for id in TemplateId::ALL {
            let mut ctx = TemplateContext::sample();
            if id == TemplateId::Settlement {
                ctx.inv10_disclaimer = Some(INV10_SETTLEMENT_DISCLAIMER.to_string());
            }
            let doc = reg.render(id, &ctx).unwrap();
            assert!(!doc.blocks.is_empty(), "{id:?} rendered empty");
            assert_eq!(doc.blocks[0].kind, BlockKind::Title);
            assert_eq!(doc.blocks[0].text, id.title());
        }
    }

    #[test]
    fn law_ref_urn_is_embedded_in_every_template() {
        let reg = TemplateRegistry::load_builtin().unwrap();
        for id in TemplateId::ALL {
            let mut ctx = TemplateContext::sample();
            if id == TemplateId::Settlement {
                ctx.inv10_disclaimer = Some(INV10_SETTLEMENT_DISCLAIMER.to_string());
            }
            let doc = reg.render(id, &ctx).unwrap();
            let full: String = doc.blocks.iter().map(|b| b.text.clone()).collect();
            for lr in &ctx.law_refs {
                assert!(
                    full.contains(&lr.urn),
                    "{id:?}: URN {} not embedded (GB-05)",
                    lr.urn
                );
            }
        }
    }

    #[test]
    fn template_id_serialises_with_frontend_alias() {
        assert_eq!(
            serde_json::to_string(&TemplateId::ArbApplication).unwrap(),
            "\"arb_application\""
        );
        assert_eq!(
            serde_json::to_string(&TemplateId::Settlement).unwrap(),
            "\"settlement\""
        );
        let back: TemplateId = serde_json::from_str("\"lawsuit\"").unwrap();
        assert_eq!(back, TemplateId::Lawsuit);
    }

    #[test]
    fn settlement_requires_inv10_disclaimer_slot() {
        let reg = TemplateRegistry::load_builtin().unwrap();
        let mut ctx = TemplateContext::sample();
        ctx.inv10_disclaimer = None; // missing → slot validation fails
        let err = reg
            .validate_slots(TemplateId::Settlement, &ctx)
            .unwrap_err();
        assert!(matches!(err, SlotError::MissingInv10Disclaimer));
    }

    #[test]
    fn settlement_output_always_contains_inv10_text() {
        let reg = TemplateRegistry::load_builtin().unwrap();
        let mut ctx = TemplateContext::sample();
        ctx.inv10_disclaimer = Some(INV10_SETTLEMENT_DISCLAIMER.to_string());
        let doc = reg.render(TemplateId::Settlement, &ctx).unwrap();
        let full: String = doc.blocks.iter().map(|b| b.text.clone()).collect();
        assert!(full.contains("不可删除"));
        assert!(full.contains("12348"));
        assert!(full.contains("80%"));
    }

    #[test]
    fn non_urn_law_ref_is_rejected() {
        let reg = TemplateRegistry::load_builtin().unwrap();
        let mut ctx = TemplateContext::sample();
        ctx.law_refs[0].urn = "reg_gb45438_2025".to_string(); // deprecated short code
        let err = reg
            .validate_slots(TemplateId::ArbApplication, &ctx)
            .unwrap_err();
        assert!(matches!(err, SlotError::NonUrnLawRef(_)));
    }
}
