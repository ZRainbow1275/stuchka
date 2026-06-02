//! JSON serialisation of the Layer-4 manifest (the三件套 JSON file, compliance/01 §6.1).

use crate::gb45438::manifest::Manifest;
use crate::gb45438::Gb45438Error;

/// Serialise the manifest to pretty JSON bytes (the `manifest.json` placed at the dossier root).
pub fn to_json_bytes(manifest: &Manifest) -> Result<Vec<u8>, Gb45438Error> {
    serde_json::to_vec_pretty(manifest)
        .map_err(|e| Gb45438Error::TemplateRender(format!("manifest serialise: {e}")))
}

/// Serialise the manifest to a pretty JSON string.
pub fn to_json_string(manifest: &Manifest) -> Result<String, Gb45438Error> {
    serde_json::to_string_pretty(manifest)
        .map_err(|e| Gb45438Error::TemplateRender(format!("manifest serialise: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb45438::manifest::{self, AiSegment};
    use crate::gb45438::metadata::DocMeta;
    use crate::render::ParagraphRef;
    use crate::template::TemplateId;

    #[test]
    fn manifest_json_round_trips_and_keeps_hex_ids() {
        let meta = DocMeta::sample(TemplateId::ArbApplication);
        let segs = vec![AiSegment::approved(
            0x1a3f,
            ParagraphRef(1),
            "$.body.section[0].paragraph[0]",
            "deepseek",
            "a".repeat(64),
            "b".repeat(64),
            0.83,
        )];
        let m = manifest::build(&segs, &meta).unwrap();
        let s = to_json_string(&m).unwrap();
        assert!(s.contains("\"0x0000000000001a3f\""));
        assert!(s.contains("\"layer4_manifest\": true"));
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.segment_ids(), m.segment_ids());
    }
}
