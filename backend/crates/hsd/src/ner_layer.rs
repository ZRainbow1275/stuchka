//! Layer 2 — candle Chinese NER (R1b 争取) — INTERFACE / SKELETON ONLY this round.
//!
//! Per the implementation brief §7.2 and the cross-crate reconciliation (E), real candle inference
//! is DEFERRED: candle 0.10 is a heavy build, and the ~400MB safetensors weights are an optional
//! `crates/core` first-run download (hsd only reads local paths). To keep the R1a dependency tree
//! clean (zero candle / pyo3 / onnx / network — D4 + `tests/isolation.rs`), this module ships only:
//!
//! - the BIO label set + [`NerBackend`] enum,
//! - [`NerLayer::try_load`], which performs the offline guard rails (disabled / missing-weight →
//!   `Ok(None)` downgrade, SHA-256 tamper check → `Err(ModelTampered)`), WITHOUT linking candle,
//! - [`NerLayer::scan`], reserved for the real inference path.
//!
//! When R1b lands, the candle model/tokenizer fields and inference go inside this same interface
//! (`HsdDetector::scan` already swallows NER errors and downgrades to R1a — INC-4).

use std::fs;
use std::path::Path;

use crate::config::HsdConfig;
use crate::error::HsdError;
use crate::types::PiiHit;

/// BIO label set for the token-classification head (`ai/04` §4.4): person / location / org /
/// medical keyword.
pub const LABELS: [&str; 9] = [
    "O", "B-PER", "I-PER", "B-LOC", "I-LOC", "B-ORG", "I-ORG", "B-MED", "I-MED",
];

/// NER inference backend. candle is the R1b default; ONNX (`ort`, pure Rust, still zero-Python) is
/// only a recorded R1.5+ fallback (`ai/04` §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NerBackend {
    /// candle pure-Rust inference (R1b default).
    Candle,
    /// `ort` pure-Rust ONNX fallback (R1.5+ only).
    Onnx,
}

/// Layer-2 NER detector. The candle model / tokenizer fields land when R1b real inference ships;
/// for now the struct only records the chosen backend so `HsdDetector` can report capability.
pub struct NerLayer {
    backend: NerBackend,
    score_threshold: f32,
}

impl NerLayer {
    /// Attempt to load the NER layer per config. Offline guard rails (no candle linked yet):
    ///
    /// - `enable_ner_layer == false` → `Ok(None)` (R1a default).
    /// - weight path absent / missing on disk → `Ok(None)` + `tracing::warn` (graceful downgrade).
    /// - weight present but SHA-256 mismatch → `Err(HsdError::ModelTampered)` (hard error, A14).
    /// - weight present + (no expected sha OR sha matches) → real inference is NOT yet implemented,
    ///   so this returns `Ok(None)` and logs that R1b is deferred (R1a still 必死).
    pub fn try_load(cfg: &HsdConfig) -> Result<Option<Self>, HsdError> {
        if !cfg.enable_ner_layer {
            return Ok(None);
        }
        let Some(weight_path) = cfg.ner_model_path.as_deref() else {
            tracing::warn!("NER enabled but no weight path; staying R1a-only");
            return Ok(None);
        };
        if !weight_path.exists() {
            tracing::warn!(
                path = %weight_path.display(),
                "NER weights not downloaded; staying R1a-only (still meets SLA)"
            );
            return Ok(None);
        }
        // Tamper guard (A14): when an expected SHA is configured it must match the file.
        if let Some(expected) = cfg.ner_expected_sha.as_deref() {
            let actual = sha256_file(weight_path)?;
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(HsdError::ModelTampered);
            }
        }
        // R1b real candle inference is deferred (brief §7.2). The weights are present and (if a
        // hash was given) verified, but no inference engine is linked yet, so downgrade to R1a.
        tracing::warn!(
            "NER weights present but R1b candle inference is deferred this round; R1a-only"
        );
        let _ = NerBackend::Candle; // backend selection point for R1b.
        Ok(None)
    }

    /// Run NER inference (R1b). Not yet implemented — reserved interface. `HsdDetector::scan`
    /// swallows the error and stays R1a-only (INC-4), so this never breaks strong-signal detection.
    pub fn scan(&self, _text: &str) -> Result<Vec<PiiHit>, HsdError> {
        let _ = (self.backend, self.score_threshold);
        Err(HsdError::Inference(
            "R1b candle NER inference deferred (brief §7.2)".to_string(),
        ))
    }
}

/// Hex SHA-256 of a file, computed without any crypto dependency (pure-Rust FIPS 180-4
/// implementation) so the isolation tree stays minimal. Used only for the NER tamper guard.
fn sha256_file(path: &Path) -> Result<String, HsdError> {
    let data = fs::read(path).map_err(|e| HsdError::Tokenizer(e.to_string()))?;
    Ok(sha256_hex(&data))
}

/// Minimal SHA-256 (FIPS 180-4) over a byte slice → lowercase hex. Self-contained to avoid pulling
/// a crypto crate into the R1a isolation tree.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let j = i * 4;
            *word = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ner_disabled_returns_none() {
        let cfg = HsdConfig::default();
        assert!(NerLayer::try_load(&cfg).unwrap().is_none());
    }

    #[test]
    fn ner_missing_weight_path_downgrades() {
        let cfg = HsdConfig {
            enable_ner_layer: true,
            ner_model_path: None,
            ..HsdConfig::default()
        };
        assert!(NerLayer::try_load(&cfg).unwrap().is_none());
    }

    #[test]
    fn ner_nonexistent_weight_file_downgrades() {
        let cfg = HsdConfig {
            enable_ner_layer: true,
            ner_model_path: Some(std::path::PathBuf::from(
                "/definitely/not/a/real/weights.safetensors",
            )),
            ..HsdConfig::default()
        };
        assert!(NerLayer::try_load(&cfg).unwrap().is_none());
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
