//! `hsd` — high-sensitivity detector (`ai/04` + `backend/04` §4.6, D4: pure Rust, zero Python).
//!
//! Two-layer detector whose any hit can force local routing:
//! - **Layer 1 — regex strong-signal layer (R1a 必死)**: phone / id card (ISO 7064 mod 11-2) /
//!   bank card (Luhn) / audio-file path / medical-record path, plus weak email / address signals.
//!   Offline, zero-network, zero-IO, pure CPU.
//! - **Layer 2 — candle Chinese NER (R1b 争取)**: interface/skeleton only this round (see
//!   [`ner_layer`]); real candle inference is deferred and auto-downgrades to R1a-only.
//!
//! Shared value types (`PiiHit` / `PiiKind` / `PiiLayer` / `Span` / `RouteHint`) live in
//! `crates/data-model` and are re-exported here so `crates/api` consumes them with zero conversion.

pub mod config;
pub mod decision;
pub mod error;
pub mod gazetteer;
pub mod luhn;
pub mod mask;
pub mod ner_layer;
pub mod normalize;
pub mod regex_layer;
pub mod types;

pub use config::HsdConfig;
pub use error::HsdError;
pub use luhn::{idcard_checksum, luhn_check};
pub use mask::mask;
pub use ner_layer::{NerBackend, NerLayer, LABELS};
pub use normalize::{normalize, NormalizedText};
pub use regex_layer::{PostValidator, RegexLayer, RegexRule};
pub use types::{DataGrade, PiiHit, PiiKind, PiiLayer, RouteHint, ScanReport, Span};

use config::DEFAULT_MAX_INPUT_BYTES;

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "hsd";

/// The high-sensitivity detector facade (`ai/04` §2.2). Holds the (process-shared) regex layer and
/// an optional NER layer. Constructed once and reused; `scan()` is `&self`, sync, and zero-IO.
pub struct HsdDetector {
    /// Always present (R1a 必死).
    regex: &'static RegexLayer,
    /// `None` = regex-only (R1a). `Some` once R1b real inference ships.
    ner: Option<NerLayer>,
    /// Max input bytes; longer inputs are truncated for the regex layer (never an error).
    max_input_bytes: usize,
}

impl HsdDetector {
    /// R1a-only constructor: regex strong-signal layer only. Reads no files, performs no network
    /// I/O, and never fails — the offline 必死 path (A12).
    pub fn new_regex_only() -> Self {
        Self {
            regex: RegexLayer::global(),
            ner: None,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
        }
    }

    /// Full constructor: attempts to load the NER layer per `cfg`. A missing weight file
    /// auto-downgrades to R1a-only (`ner = None`, no error — A13); a SHA-256 mismatch is a hard
    /// error (`HsdError::ModelTampered` — A14).
    pub fn try_new(cfg: &HsdConfig) -> Result<Self, HsdError> {
        let ner = NerLayer::try_load(cfg)?;
        Ok(Self {
            regex: RegexLayer::global(),
            ner,
            max_input_bytes: cfg.max_input_bytes.max(1),
        })
    }

    /// Whether the NER layer is active (R1b loaded).
    pub fn has_ner(&self) -> bool {
        self.ner.is_some()
    }

    /// Main scan entry point: synchronous, pure CPU, zero IO, zero network.
    ///
    /// Strong-signal regex hits set `is_high_sensitive` + `ForceLocal`. The NER layer (if loaded)
    /// contributes additional hits; any NER error is swallowed and logged, downgrading to
    /// R1a-only so strong-signal detection never fails because of NER (INC-4 / R1a 必死).
    pub fn scan(&self, text: &str) -> ScanReport {
        // Truncate (do not error) at a UTF-8 char boundary within the byte budget.
        let scanned = if text.len() > self.max_input_bytes {
            let mut cut = self.max_input_bytes;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            &text[..cut]
        } else {
            text
        };

        let normalized = normalize(scanned);
        let regex_hits = self.regex.scan(&normalized);

        let ner_hits = match &self.ner {
            Some(ner) => match ner.scan(scanned) {
                Ok(hits) => hits,
                Err(e) => {
                    tracing::warn!(error = %e, "NER scan failed; downgrading to R1a-only");
                    Vec::new()
                }
            },
            None => Vec::new(),
        };

        decision::decide(regex_hits, ner_hits)
    }
}

impl Default for HsdDetector {
    fn default() -> Self {
        Self::new_regex_only()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(CRATE_NAME, "hsd");
    }

    #[test]
    fn a12_new_regex_only_scans_offline() {
        let det = HsdDetector::new_regex_only();
        let rep = det.scan("我的电话13800138000");
        assert!(rep.is_high_sensitive);
        assert_eq!(rep.route_hint, RouteHint::ForceLocal);
        assert!(!det.has_ner());
    }

    #[test]
    fn a13_try_new_enabled_uses_real_gazetteer() {
        // R1b: enabling NER loads the REAL gazetteer backend even when no candle weight exists
        // (a missing candle weight downgrades candle → gazetteer, NOT → None).
        let cfg = HsdConfig {
            enable_ner_layer: true,
            ner_model_path: Some(std::path::PathBuf::from("/no/such/weights.safetensors")),
            ..HsdConfig::default()
        };
        let det = HsdDetector::try_new(&cfg).expect("missing candle weight must not error");
        assert!(det.has_ner(), "the gazetteer backend is active");
        // Strong signal still detected, and the NER layer now adds a person name.
        assert!(det.scan("13800138000").is_high_sensitive);
        let rep = det.scan("我同事张伟");
        assert!(rep.hits.iter().any(|h| h.rule_id == "NER-PER"));
        assert!(!rep.is_high_sensitive, "a lone person name is weak (INC-7)");
    }

    #[test]
    fn a14_tampered_weights_error() {
        // Write a temp "weight" file and assert a wrong expected-SHA yields ModelTampered.
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "hsd_test_weights_{}.safetensors",
            std::process::id()
        ));
        std::fs::write(&path, b"not real weights").unwrap();
        let cfg = HsdConfig {
            enable_ner_layer: true,
            ner_model_path: Some(path.clone()),
            ner_expected_sha: Some("deadbeef".repeat(8)), // 64 hex chars, definitely wrong
            ..HsdConfig::default()
        };
        let res = HsdDetector::try_new(&cfg);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(res, Err(HsdError::ModelTampered)));
    }

    #[test]
    fn input_truncation_does_not_panic() {
        let cfg = HsdConfig {
            max_input_bytes: 8,
            ..HsdConfig::default()
        };
        let det = HsdDetector::try_new(&cfg).unwrap();
        // multi-byte chars right at the truncation boundary must not panic.
        let rep = det.scan("电话电话电话13800138000");
        assert_eq!(rep.route_hint, RouteHint::Auto); // truncated before the phone number
    }

    #[test]
    fn a22_hits_serialize_as_api_pii_hits() {
        let det = HsdDetector::new_regex_only();
        let rep = det.scan("13800138000");
        let json = serde_json::to_string(&rep.hits).unwrap();
        // data-model PiiHit serializes camelCase ruleId — same type api EvidenceDto.pii_hits uses.
        assert!(json.contains("\"ruleId\":\"PII-PHONE-CN\""), "got {json}");
        assert!(json.contains("\"kind\":\"phone\""), "got {json}");
    }

    #[test]
    fn a23_is_strong_classification() {
        for k in [
            PiiKind::Phone,
            PiiKind::IdCard,
            PiiKind::BankCard,
            PiiKind::AudioPath,
            PiiKind::MedicalRecord,
        ] {
            assert!(k.is_strong());
        }
        for k in [
            PiiKind::Email,
            PiiKind::Address,
            PiiKind::PersonName,
            PiiKind::Organization,
            PiiKind::MedicalKeyword,
        ] {
            assert!(!k.is_strong());
        }
    }
}
