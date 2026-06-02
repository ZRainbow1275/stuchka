//! Layer 4 — manifest schema validation (compliance/01 §6.2). Validation failure refuses dossier
//! generation. R1a enforces the load-bearing structural invariants directly (no external
//! JSON-Schema engine pulled in), matching the `schemas/manifest.v1.json` contract.

use crate::gb45438::manifest::{Manifest, ReviewState};
use crate::gb45438::Gb45438Error;

/// Expected schema version (compliance/01 §6.1).
const EXPECTED_SCHEMA: &str = "1.0.0";
/// Required SPDX license in the generator stamp (W3).
const EXPECTED_LICENSE: &str = "GPL-3.0-only";

/// Validate the manifest against `schemas/manifest.v1.json` (R1a structural enforcement). On any
/// violation returns [`Gb45438Error::ManifestSchemaInvalid`] so generation is refused.
pub fn validate(m: &Manifest) -> Result<(), Gb45438Error> {
    if m.schema_version != EXPECTED_SCHEMA {
        return Err(Gb45438Error::ManifestSchemaInvalid);
    }
    // case_id_hashed is blake3-hex-32 (32 hex chars); kb_frozen_hash is sha256-hex-64 (64 chars).
    if !is_hex_chars(&m.case_id_hashed, 32) {
        return Err(Gb45438Error::ManifestSchemaInvalid);
    }
    if !is_hex_chars(&m.kb_frozen_hash, 64) {
        return Err(Gb45438Error::ManifestSchemaInvalid);
    }
    if m.generator.name != "Stučka" || m.generator.license != EXPECTED_LICENSE {
        return Err(Gb45438Error::ManifestSchemaInvalid);
    }
    if m.documents.is_empty() {
        return Err(Gb45438Error::ManifestSchemaInvalid);
    }
    for d in &m.documents {
        if d.filename.is_empty() {
            return Err(Gb45438Error::ManifestSchemaInvalid);
        }
        for s in &d.ai_generated_segments {
            // kb_hash / prompt_hash must be sha256 hex 64 (INV-04).
            if !is_hex_chars(&s.kb_hash, 64) || !is_hex_chars(&s.prompt_hash, 64) {
                return Err(Gb45438Error::ManifestSchemaInvalid);
            }
            if !(0.0..=1.0).contains(&s.confidence) {
                return Err(Gb45438Error::ManifestSchemaInvalid);
            }
            if s.doc_path.is_empty() {
                return Err(Gb45438Error::ManifestSchemaInvalid);
            }
            // an edited segment must record the diff hash (blake3 hex 32 = 32 hex chars).
            if s.review_state == ReviewState::Edited {
                match &s.edit_diff_hash {
                    Some(h) if is_hex_chars(h, 32) => {}
                    _ => return Err(Gb45438Error::ManifestSchemaInvalid),
                }
            }
        }
    }
    Ok(())
}

/// A hex string of exactly `n` hex characters (case-insensitive).
fn is_hex_chars(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb45438::manifest::{self, AiSegment};
    use crate::gb45438::metadata::DocMeta;
    use crate::render::ParagraphRef;
    use crate::template::TemplateId;

    fn good_manifest() -> Manifest {
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        let segs = vec![AiSegment::approved(
            1,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek",
            "a".repeat(64),
            "b".repeat(64),
            0.83,
        )];
        manifest::build(&segs, &meta).unwrap()
    }

    #[test]
    fn valid_manifest_passes() {
        validate(&good_manifest()).unwrap();
    }

    #[test]
    fn bad_schema_version_rejected() {
        let mut m = good_manifest();
        m.schema_version = "9.9.9".into();
        assert!(matches!(
            validate(&m),
            Err(Gb45438Error::ManifestSchemaInvalid)
        ));
    }

    #[test]
    fn wrong_license_rejected() {
        let mut m = good_manifest();
        m.generator.license = "GPL-3.0-or-later".into();
        assert!(matches!(
            validate(&m),
            Err(Gb45438Error::ManifestSchemaInvalid)
        ));
    }

    #[test]
    fn bad_kb_hash_length_rejected() {
        let mut m = good_manifest();
        m.documents[0].ai_generated_segments[0].kb_hash = "tooshort".into();
        assert!(matches!(
            validate(&m),
            Err(Gb45438Error::ManifestSchemaInvalid)
        ));
    }

    #[test]
    fn edited_without_diff_hash_rejected() {
        let mut m = good_manifest();
        m.documents[0].ai_generated_segments[0].review_state = ReviewState::Edited;
        m.documents[0].ai_generated_segments[0].edit_diff_hash = None;
        assert!(matches!(
            validate(&m),
            Err(Gb45438Error::ManifestSchemaInvalid)
        ));
        // with a valid blake3 hex 32 it passes
        m.documents[0].ai_generated_segments[0].edit_diff_hash = Some("c".repeat(32));
        validate(&m).unwrap();
    }
}
