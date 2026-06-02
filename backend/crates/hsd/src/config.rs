//! HSD configuration (`ai/04` §2.3 `HsdConfig`).
//!
//! All NER-related fields are optional: R1a (the regex strong-signal layer) is always enabled and
//! needs no configuration. When `enable_ner_layer` is `false` (the default) or the weight path is
//! absent, `HsdDetector::try_new` constructs a regex-only detector that still passes the SLA.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Default maximum input size in bytes for a single `scan()` (`ai/04` §2.3). Inputs longer than
/// this are truncated for the regex layer (never an error — R1a 必死).
pub const DEFAULT_MAX_INPUT_BYTES: usize = 4096;

/// Default minimum NER span confidence to keep a hit (`ai/04` §4.4 filters score < 0.85).
pub const DEFAULT_NER_SCORE_THRESHOLD: f32 = 0.85;

/// HSD configuration (`ai/04` §2.3). R1a needs none of the NER fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsdConfig {
    /// Enable the Layer-2 candle NER (R1b). Default `false` — R1a regex alone meets the SLA.
    pub enable_ner_layer: bool,
    /// safetensors weight path (~400MB optional download). `None` / absent → downgrade to R1a-only.
    pub ner_model_path: Option<PathBuf>,
    /// tokenizer.json path.
    pub tokenizer_path: Option<PathBuf>,
    /// bert config.json path.
    pub config_path: Option<PathBuf>,
    /// Expected SHA-256 of the weight file (tamper guard, A14). `None` skips verification.
    pub ner_expected_sha: Option<String>,
    /// Minimum NER span confidence to keep a hit.
    pub ner_score_threshold: f32,
    /// Maximum input size in bytes; longer inputs are truncated for the regex layer.
    pub max_input_bytes: usize,
}

impl Default for HsdConfig {
    fn default() -> Self {
        Self {
            enable_ner_layer: false,
            ner_model_path: None,
            tokenizer_path: None,
            config_path: None,
            ner_expected_sha: None,
            ner_score_threshold: DEFAULT_NER_SCORE_THRESHOLD,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
        }
    }
}
