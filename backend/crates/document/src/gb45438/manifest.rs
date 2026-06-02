//! Layer 4 — dossier JSON manifest (`ai_generated_segments[]`) + builder (compliance/01 §6).
//!
//! The manifest is the "防修改" tamper-evidence layer: a verifiable list of every AI-drafted
//! segment with its provenance (model / kb_hash / prompt_hash / review state). The `segment_id`
//! set here is asserted 1:1 against the decoded zero-width frames (GB-02).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::gb45438::metadata::DocMeta;
use crate::gb45438::Gb45438Error;
use crate::render::ParagraphRef;
use crate::template::TemplateId;

/// Review outcome of an AI-drafted segment (compliance/01 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Approved,
    Edited,
    Rejected,
}

/// One AI-drafted segment: the Layer-4 record + the Layer-3 zero-width input contract. The
/// `segment_id` is the 64-bit id encoded into the zero-width frame and listed in the manifest;
/// they must form an exact 1:1 set (GB-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiSegment {
    /// 64-bit segment id (rendered as `0x…` hex in the manifest, mirrored in the zero-width frame).
    #[serde(with = "hex_u64")]
    pub segment_id: u64,
    /// Reference to the rendered paragraph this segment marks (not serialised into the manifest).
    #[serde(skip)]
    pub paragraph_ref: ParagraphRef,
    /// ProseMirror path, e.g. `$.body.section[2].paragraph[5]`.
    pub doc_path: String,
    /// Document-level character offsets `[start, end]`.
    pub char_range: (u32, u32),
    /// Model identifier, e.g. `deepseek-v3.1-2026Q1` (ai/01 provider).
    pub model_id: String,
    /// Frozen knowledge-base hash (sha256 hex 64, INV-04).
    pub kb_hash: String,
    /// Prompt hash (sha256 hex 64).
    pub prompt_hash: String,
    /// Model confidence in `[0, 1]`.
    pub confidence: f32,
    pub review_state: ReviewState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_time: Option<chrono::DateTime<chrono::Utc>>,
    /// blake3 hex 32 of the final-vs-draft diff when the segment was edited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_diff_hash: Option<String>,
}

impl AiSegment {
    /// Build a minimal approved segment (ergonomic constructor for callers / tests).
    pub fn approved(
        segment_id: u64,
        paragraph_ref: ParagraphRef,
        doc_path: impl Into<String>,
        model_id: impl Into<String>,
        kb_hash: impl Into<String>,
        prompt_hash: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            segment_id,
            paragraph_ref,
            doc_path: doc_path.into(),
            char_range: (0, 0),
            model_id: model_id.into(),
            kb_hash: kb_hash.into(),
            prompt_hash: prompt_hash.into(),
            confidence,
            review_state: ReviewState::Approved,
            reviewer_id: None,
            review_time: None,
            edit_diff_hash: None,
        }
    }
}

/// The five-boolean layer completeness, serialised into the manifest (compliance/01 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerCompletenessJson {
    pub layer1_explicit: bool,
    pub layer2_metadata: bool,
    pub layer3_zerowidth: bool,
    pub layer3_lsb: bool,
    pub layer4_manifest: bool,
}

/// Generator stamp (compliance/01 §6.1: name / version / license = GPL-3.0-only, W3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    pub version: String,
    pub license: String,
}

/// One document entry inside the manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoc {
    pub doc_id: Uuid,
    pub filename: String,
    pub ai_generated_segments: Vec<AiSegment>,
}

/// The dossier `manifest.json` root (compliance/01 §6.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub case_id_hashed: String,
    pub documents: Vec<ManifestDoc>,
    pub kb_frozen_hash: String,
    pub generator: Generator,
    pub gb45438_layer_completeness: LayerCompletenessJson,
}

/// Schema version emitted by this build (compliance/01 §6.1).
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Build the manifest for a single document from its AI segments + metadata. The
/// `gb45438_layer_completeness` is filled by `run_all_layers`; `build` sets it to all-true and the
/// caller overwrites it with the真实 self-check booleans (the validator re-checks it).
pub fn build(segs: &[AiSegment], meta: &DocMeta) -> Result<Manifest, Gb45438Error> {
    let filename = meta.template_id.default_filename();
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION.to_string(),
        case_id_hashed: meta.case_id_hashed.clone(),
        documents: vec![ManifestDoc {
            doc_id: meta.doc_id,
            filename,
            ai_generated_segments: segs.to_vec(),
        }],
        kb_frozen_hash: meta.kb_hash.clone(),
        generator: Generator {
            name: "Stučka".to_string(),
            version: meta.version.clone(),
            license: "GPL-3.0-only".to_string(),
        },
        gb45438_layer_completeness: LayerCompletenessJson {
            layer1_explicit: true,
            layer2_metadata: true,
            layer3_zerowidth: true,
            layer3_lsb: true,
            layer4_manifest: true,
        },
    };
    Ok(manifest)
}

impl Manifest {
    /// The set of `segment_id`s across all documents (used for the GB-02 1:1 assertion).
    pub fn segment_ids(&self) -> std::collections::BTreeSet<u64> {
        self.documents
            .iter()
            .flat_map(|d| d.ai_generated_segments.iter().map(|s| s.segment_id))
            .collect()
    }
}

/// Helper extension so [`TemplateId`] can name its default output file.
impl TemplateId {
    /// The default dossier filename for this template (compliance/01 §6.1 example).
    pub fn default_filename(self) -> String {
        let stem = match self {
            TemplateId::ArbApplication => "labor_arbitration_application",
            TemplateId::Mediation => "mediation_application",
            TemplateId::Inspection => "labor_inspection_complaint",
            TemplateId::Settlement => "settlement_agreement",
            TemplateId::Lawsuit => "lawsuit_complaint",
            TemplateId::Enforcement => "enforcement_application",
        };
        format!("{stem}.pdf")
    }
}

/// serde adapter: serialise a `u64` segment id as the `0x…` hex string the spec shows, accept both
/// the hex string and a plain integer on the way back in.
mod hex_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("0x{v:016x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Hex(String),
            Int(u64),
        }
        match Repr::deserialize(d)? {
            Repr::Int(n) => Ok(n),
            Repr::Hex(s) => {
                let s = s.trim_start_matches("0x").trim_start_matches("0X");
                u64::from_str_radix(s, 16).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_id_serialises_as_hex_and_round_trips() {
        let seg = AiSegment::approved(
            0x1a3f,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek-v3.1-2026Q1",
            "a".repeat(64),
            "b".repeat(64),
            0.83,
        );
        let j = serde_json::to_string(&seg).unwrap();
        assert!(j.contains("\"0x0000000000001a3f\""), "got {j}");
        let back: AiSegment = serde_json::from_str(&j).unwrap();
        assert_eq!(back.segment_id, 0x1a3f);
        assert_eq!(back.review_state, ReviewState::Approved);
    }

    #[test]
    fn manifest_segment_ids_are_collected() {
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        let segs = vec![
            AiSegment::approved(1, ParagraphRef(1), "p", "m", "k", "p", 0.9),
            AiSegment::approved(2, ParagraphRef(2), "p", "m", "k", "p", 0.9),
        ];
        let m = build(&segs, &meta).unwrap();
        let ids = m.segment_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1) && ids.contains(&2));
        assert_eq!(m.generator.license, "GPL-3.0-only");
        assert_eq!(m.schema_version, "1.0.0");
    }
}
