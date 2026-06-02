//! Layer 2 — file metadata marking: PDF Info Dict + XMP packet (compliance/01 §4).
//!
//! Both stores are written so "any tool can read a hit". The Info Dict / XMP are accumulated onto
//! the `RenderDoc`; `pdf.rs` then writes them into the real PDF trailer + metadata stream.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::gb45438::Gb45438Error;
use crate::render::RenderDoc;
use crate::template::TemplateId;

/// Per-document review state surfaced in metadata (compliance/01 §4.1.1 `/AIGC.ReviewState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocReviewState {
    Unchecked,
    Partial,
    Full,
}

impl DocReviewState {
    /// The literal written into metadata (`unchecked | partial | full`).
    pub fn as_str(self) -> &'static str {
        match self {
            DocReviewState::Unchecked => "unchecked",
            DocReviewState::Partial => "partial",
            DocReviewState::Full => "full",
        }
    }
}

/// Document metadata shared across all four layers (compliance/01 §4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocMeta {
    /// UUID v7 (D9).
    pub doc_id: Uuid,
    /// blake3 hex 32 of the case id (the plaintext case id never leaves the box).
    pub case_id_hashed: String,
    pub template_id: TemplateId,
    /// `stuchka-v{version}` generator version string.
    pub version: String,
    /// Full kb hash (sha256 hex 64); the metadata shows `kb_hash[:12]`.
    pub kb_hash: String,
    pub kb_release_date: chrono::NaiveDate,
    pub review_state_doc: DocReviewState,
    /// ISO-8601 generation time with timezone.
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub segment_count: u32,
}

impl DocMeta {
    /// The displayed short kb hash (`kb_hash[:12]`).
    pub fn kb_hash_short(&self) -> &str {
        let end = self.kb_hash.len().min(12);
        &self.kb_hash[..end]
    }

    /// The `stuchka-v{version}` generator id used in metadata.
    pub fn generator_id(&self) -> String {
        format!("stuchka-v{}", self.version)
    }

    /// A deterministic sample meta for tests / examples.
    #[doc(hidden)]
    pub fn sample(template_id: TemplateId) -> Self {
        Self {
            doc_id: Uuid::nil(),
            // case_id_hashed is blake3-hex-32 = 32 hex chars (compliance/01 §6.1).
            case_id_hashed: "0".repeat(32),
            template_id,
            version: "0.1.0".to_string(),
            kb_hash: "a".repeat(64),
            kb_release_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
            review_state_doc: DocReviewState::Full,
            generated_at: chrono::DateTime::from_timestamp(1_748_000_000, 0).unwrap(),
            segment_count: 0,
        }
    }
}

/// XMP namespace URI for the Stučka AIGC schema (compliance/01 §4.1.2).
pub const XMP_NS: &str = "https://stuchka.example/schema/aigc/2025/";

/// Write the PDF Info Dict + XMP packet onto the `RenderDoc` (Layer 2). Pure metadata accumulation;
/// `pdf.rs` serialises it into the real PDF. The Info Dict keys / XMP fields exactly match §4.1.
pub fn write_pdf_info_and_xmp(doc: &mut RenderDoc, meta: &DocMeta) -> Result<(), Gb45438Error> {
    let kb_short = meta.kb_hash_short().to_string();
    let gen_id = meta.generator_id();
    let review = meta.review_state_doc.as_str();
    let segs = meta.segment_count.to_string();
    let gen_time = meta.generated_at.to_rfc3339();

    // ----- Info Dict (§4.1.1) -----
    let info = &mut doc.info_dict;
    info.set("/Producer", format!("Stučka {}", meta.version));
    info.set("/Creator", "Stučka Document Engine");
    info.set("/Subject", "AI-Assisted Document, GB/T 45438-2025");
    info.set(
        "/Keywords",
        format!(
            "AI-Generated, AIGC, GB45438, Stuchka, {}",
            meta.case_id_hashed
        ),
    );
    info.set("/AIGC.Standard", "GB/T 45438-2025");
    info.set("/AIGC.GeneratorId", gen_id.clone());
    info.set("/AIGC.Segments", segs.clone());
    info.set("/AIGC.KbVersion", kb_short.clone());
    info.set("/AIGC.ReviewState", review);

    // ----- XMP packet (§4.1.2) -----
    let xmp = format!(
        concat!(
            "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n",
            "  <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n",
            "    <rdf:Description rdf:about=\"\"\n",
            "        xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n",
            "        xmlns:stuchka=\"{ns}\">\n",
            "      <dc:contributor>\n",
            "        <rdf:Bag><rdf:li>Stučka AI</rdf:li></rdf:Bag>\n",
            "      </dc:contributor>\n",
            "      <stuchka:standard>GB/T 45438-2025</stuchka:standard>\n",
            "      <stuchka:generatorId>{gen}</stuchka:generatorId>\n",
            "      <stuchka:segments>{segs}</stuchka:segments>\n",
            "      <stuchka:kbVersion>{kb}</stuchka:kbVersion>\n",
            "      <stuchka:reviewState>{review}</stuchka:reviewState>\n",
            "      <stuchka:generationTime>{time}</stuchka:generationTime>\n",
            "    </rdf:Description>\n",
            "  </rdf:RDF>\n",
            "</x:xmpmeta>\n",
            "<?xpacket end=\"w\"?>"
        ),
        ns = XMP_NS,
        gen = gen_id,
        segs = segs,
        kb = kb_short,
        review = review,
        time = gen_time,
    );
    doc.xmp.xml = xmp;
    Ok(())
}

/// Required Info Dict keys (Layer 2 self-check; §4.1.1).
const REQUIRED_INFO_KEYS: &[&str] = &[
    "/Producer",
    "/Creator",
    "/Subject",
    "/Keywords",
    "/AIGC.Standard",
    "/AIGC.GeneratorId",
    "/AIGC.Segments",
    "/AIGC.KbVersion",
    "/AIGC.ReviewState",
];

/// Required XMP field substrings (Layer 2 self-check; §4.1.2).
const REQUIRED_XMP_FIELDS: &[&str] = &[
    "stuchka:standard",
    "stuchka:generatorId",
    "stuchka:segments",
    "stuchka:kbVersion",
    "stuchka:reviewState",
    "stuchka:generationTime",
    "dc:contributor",
];

/// Layer-2 self-check (compliance/01 §6.4): every required Info Dict key + XMP field must be
/// present (this is the `exiftool`-readable hit set, asserted on the lopdf output as well).
pub fn verify(doc: &RenderDoc) -> Result<bool, Gb45438Error> {
    for key in REQUIRED_INFO_KEYS {
        if doc.info_dict.get(key).is_none() {
            return Ok(false);
        }
    }
    let standard = doc.info_dict.get("/AIGC.Standard") == Some("GB/T 45438-2025");
    for field in REQUIRED_XMP_FIELDS {
        if !doc.xmp.xml.contains(field) {
            return Ok(false);
        }
    }
    Ok(standard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_info_and_xmp_hits_all_required_fields() {
        let mut doc = RenderDoc::default();
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        write_pdf_info_and_xmp(&mut doc, &meta).unwrap();
        assert!(verify(&doc).unwrap());
        assert_eq!(doc.info_dict.get("/AIGC.Standard"), Some("GB/T 45438-2025"));
        assert_eq!(
            doc.info_dict.get("/Creator"),
            Some("Stučka Document Engine")
        );
        assert!(doc.info_dict.get("/Keywords").unwrap().contains("AIGC"));
        assert!(doc.xmp.xml.contains("Stučka AI"));
        assert!(doc.xmp.xml.contains(XMP_NS));
    }

    #[test]
    fn kb_hash_short_is_twelve_chars() {
        let meta = DocMeta::sample(TemplateId::Mediation);
        assert_eq!(meta.kb_hash_short().len(), 12);
        assert_eq!(meta.generator_id(), "stuchka-v0.1.0");
    }

    #[test]
    fn missing_field_fails_verify() {
        let mut doc = RenderDoc::default();
        // no metadata written → verify must fail (Layer 2 not satisfied)
        assert!(!verify(&doc).unwrap());
        let meta = DocMeta::sample(TemplateId::Lawsuit);
        write_pdf_info_and_xmp(&mut doc, &meta).unwrap();
        // tamper: drop a required key
        doc.info_dict.entries.retain(|(k, _)| k != "/AIGC.Segments");
        assert!(!verify(&doc).unwrap());
    }
}
