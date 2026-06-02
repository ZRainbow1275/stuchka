//! Layer 1 — regex strong-signal layer (R1a 必死) (`ai/04` §4.3).
//!
//! Five strong signals (phone / id card / bank card / audio path / medical-record path) plus two
//! weak signals (email / address). Offline, zero-network, zero-IO.
//!
//! ## regex crate has no lookbehind (INC-3 / reconciliation E)
//! The spec writes `(?<![0-9])...(?![0-9])` boundaries, but the `regex` 1.x crate does NOT support
//! lookbehind and panics on that form (it is forbidden here). Instead each numeric strong-signal
//! rule matches the digit core and then a hand-written [`digit_boundary_ok`] check rejects a match
//! whose immediately adjacent character (in the normalized text) is a digit — exactly equivalent
//! to `(?<![0-9])`/`(?![0-9])` but compilable and faster (RegexSet-friendly). Only the address
//! rule uses `fancy-regex` (the spec already does).

use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

use crate::luhn::{idcard_checksum, luhn_check};
use crate::normalize::NormalizedText;
use crate::types::{PiiHit, PiiKind, PiiLayer, Span};

/// Second-stage validator applied to a matched numeric core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostValidator {
    /// Luhn mod-10 (bank card).
    Luhn,
    /// ISO 7064 mod 11-2 (id card).
    IdCardChecksum,
}

/// A compiled regex rule (`ai/04` §4.3 `RegexRule`).
pub struct RegexRule {
    pub id: &'static str,
    pub kind: PiiKind,
    pub weight: f32,
    pub pattern: Regex,
    /// Optional second-stage numeric validator.
    pub post_validator: Option<PostValidator>,
    /// Whether the match must be flanked by non-digits (hand-written lookbehind/lookahead).
    pub digit_boundary: bool,
    /// Capture-group index of the reported "core" span. `0` = the whole match. Used by the phone
    /// rule to accept an optional `+86` / `86` country-code prefix while still reporting and
    /// validating only the 11-digit core (the country-code digits would otherwise break the
    /// hand-written digit boundary).
    pub core_group: usize,
}

/// The address rule uses fancy-regex (the only rule needing the spec's Unicode pattern).
pub struct AddressRule {
    pub id: &'static str,
    pub kind: PiiKind,
    pub weight: f32,
    pub pattern: FancyRegex,
}

/// Globally cached, compiled-once regex layer (`ai/04` §4.3: `RegexSet` + `LazyLock`).
pub struct RegexLayer {
    rules: Vec<RegexRule>,
    address: AddressRule,
}

static LAYER: LazyLock<RegexLayer> = LazyLock::new(RegexLayer::build);

impl RegexLayer {
    /// Borrow the process-wide layer (compiled once).
    pub fn global() -> &'static RegexLayer {
        &LAYER
    }

    /// Build the rule table. Patterns are authored without lookbehind (see module docs).
    fn build() -> Self {
        let rules = vec![
            RegexRule {
                id: "PII-PHONE-CN",
                kind: PiiKind::Phone,
                weight: 1.0,
                // Optional `+86`/`86` country code prefix, then the 11-digit mobile core in group 1.
                // The boundary / reported span use the core group, so the country-code digits do
                // not break the hand-written digit boundary (the `+86 138...` case).
                pattern: Regex::new(r"(?:\+?86)?(1[3-9][0-9]{9})").expect("PII-PHONE-CN compiles"),
                post_validator: None,
                digit_boundary: true,
                core_group: 1,
            },
            RegexRule {
                id: "PII-IDCARD-CN",
                kind: PiiKind::IdCard,
                weight: 1.0,
                // 18-char resident id: area(6) + birth(YYYYMMDD) + seq(3) + check(0-9/X).
                pattern: Regex::new(
                    r"[1-9]\d{5}(19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[0-9Xx]",
                )
                .expect("PII-IDCARD-CN compiles"),
                post_validator: Some(PostValidator::IdCardChecksum),
                digit_boundary: true,
                core_group: 0,
            },
            RegexRule {
                id: "PII-BANKCARD-CN",
                kind: PiiKind::BankCard,
                weight: 0.95,
                // Prefix heuristic 62/4x/51-55/35/36/37 then 11..17 more digits (13..19 total).
                pattern: Regex::new(r"(62|4[0-9]|5[1-5]|35|36|37)[0-9]{11,17}")
                    .expect("PII-BANKCARD-CN compiles"),
                post_validator: Some(PostValidator::Luhn),
                digit_boundary: true,
                core_group: 0,
            },
            RegexRule {
                id: "PII-AUDIO-PATH",
                kind: PiiKind::AudioPath,
                weight: 1.0,
                // Path root + 录音/通话/录像 keyword + audio extension.
                pattern: Regex::new(
                    r"(?i)([A-Za-z]:[\\/]|\.\.?/|~/)[^\s]*?(录音|通话|录像)[^\s]*\.(mp3|m4a|wav|amr|aac|opus|3gp|wma)",
                )
                .expect("PII-AUDIO-PATH compiles"),
                post_validator: None,
                digit_boundary: false,
                core_group: 0,
            },
            RegexRule {
                id: "PII-MEDICAL-PATH",
                kind: PiiKind::MedicalRecord,
                weight: 1.0,
                pattern: Regex::new(
                    r"(?i)([A-Za-z]:[\\/]|\.\.?/|~/)[^\s]*?(病历|诊断书|住院|出院|工伤鉴定)[^\s]*\.(pdf|jpg|jpeg|png|docx?)",
                )
                .expect("PII-MEDICAL-PATH compiles"),
                post_validator: None,
                digit_boundary: false,
                core_group: 0,
            },
            RegexRule {
                id: "PII-EMAIL",
                kind: PiiKind::Email,
                weight: 0.6,
                pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                    .expect("PII-EMAIL compiles"),
                post_validator: None,
                digit_boundary: false,
                core_group: 0,
            },
        ];

        let address = AddressRule {
            id: "PII-ADDRESS-SHORT",
            kind: PiiKind::Address,
            weight: 0.7,
            pattern: FancyRegex::new(
                r"[一-龥]{2,8}(省|市|区|县|镇|乡)[一-龥]{2,30}(路|街|大道|巷|号|室)\d{0,5}",
            )
            .expect("PII-ADDRESS-SHORT compiles"),
        };

        RegexLayer { rules, address }
    }

    /// Scan normalized text. Returns hits with byte spans already mapped back to the ORIGINAL
    /// text via `normalized.offset_map` (A11). Pure CPU, zero IO.
    pub fn scan(&self, normalized: &NormalizedText) -> Vec<PiiHit> {
        let text = &normalized.text;
        let bytes = text.as_bytes();
        let mut hits = Vec::new();

        for rule in &self.rules {
            for caps in rule.pattern.captures_iter(text) {
                let whole = match caps.get(0) {
                    Some(m) => m,
                    None => continue,
                };
                // The reported span is `core_group` (0 = whole match) so an optional country-code
                // prefix is consumed but not reported.
                let core = match caps.get(rule.core_group) {
                    Some(m) => m,
                    None => continue,
                };
                let (ns, ne) = (core.start(), core.end());
                let matched = &text[ns..ne];

                // Hand-written digit boundary (equivalent to (?<![0-9]) / (?![0-9])). The "before"
                // side is checked against the WHOLE match start (so a consumed `+86` country-code
                // prefix is not mistaken for a digit run); the "after" side against the core end.
                if rule.digit_boundary && !digit_boundary_ok(bytes, whole.start(), ne) {
                    continue;
                }

                // Second-stage numeric validation (Luhn / ISO 7064).
                if let Some(pv) = rule.post_validator {
                    let ok = match pv {
                        PostValidator::Luhn => luhn_check(matched),
                        PostValidator::IdCardChecksum => idcard_checksum(matched),
                    };
                    if !ok {
                        continue;
                    }
                }

                let (os, oe) = normalized.map_span(ns, ne);
                hits.push(PiiHit {
                    kind: rule.kind,
                    span: Span::new(os, oe),
                    confidence: rule.weight,
                    layer: PiiLayer::Regex,
                    rule_id: rule.id.to_string(),
                });
            }
        }

        // Address via fancy-regex (lookbehind-capable engine; iterate captures manually).
        let mut search_from = 0usize;
        while search_from <= text.len() {
            match self.address.pattern.find_from_pos(text, search_from) {
                Ok(Some(m)) => {
                    let (ns, ne) = (m.start(), m.end());
                    let (os, oe) = normalized.map_span(ns, ne);
                    hits.push(PiiHit {
                        kind: self.address.kind,
                        span: Span::new(os, oe),
                        confidence: self.address.weight,
                        layer: PiiLayer::Regex,
                        rule_id: self.address.id.to_string(),
                    });
                    search_from = if ne > ns { ne } else { ne + 1 };
                }
                _ => break,
            }
        }

        hits
    }
}

/// True iff the byte immediately before `start` and the byte at `end` are both non-digits (or
/// out of range). Equivalent to the spec's `(?<![0-9])...(?![0-9])` without lookbehind. Because
/// the match starts/ends on a digit, checking the single adjacent ASCII byte is sufficient (a
/// preceding multi-byte UTF-8 char cannot be an ASCII digit).
fn digit_boundary_ok(bytes: &[u8], start: usize, end: usize) -> bool {
    let before_ok = start == 0 || !bytes[start - 1].is_ascii_digit();
    let after_ok = end >= bytes.len() || !bytes[end].is_ascii_digit();
    before_ok && after_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::normalize;

    fn scan_raw(raw: &str) -> Vec<PiiHit> {
        let n = normalize(raw);
        RegexLayer::global().scan(&n)
    }

    fn kinds(hits: &[PiiHit]) -> Vec<PiiKind> {
        hits.iter().map(|h| h.kind).collect()
    }

    #[test]
    fn a1_phone_plain() {
        let hits = scan_raw("13800138000");
        assert!(kinds(&hits).contains(&PiiKind::Phone));
        let h = hits.iter().find(|h| h.kind == PiiKind::Phone).unwrap();
        assert_eq!(h.span.len(), 11);
    }

    #[test]
    fn a3_idcard_digit_and_x() {
        // numeric check digit
        assert!(kinds(&scan_raw("110101199001010074")).contains(&PiiKind::IdCard));
        // X check char
        assert!(kinds(&scan_raw("11010119900101004X")).contains(&PiiKind::IdCard));
    }

    #[test]
    fn a4_idcard_bad_checksum_rejected() {
        // Valid structure, invalid ISO 7064 check digit → no IdCard hit.
        assert!(!kinds(&scan_raw("110101199001010070")).contains(&PiiKind::IdCard));
    }

    #[test]
    fn a5_bankcard_luhn_valid() {
        assert!(kinds(&scan_raw("6222021234567894")).contains(&PiiKind::BankCard));
    }

    #[test]
    fn a6_order_number_not_bankcard() {
        // 16-digit non-Luhn order number must not be a BankCard.
        assert!(!kinds(&scan_raw("1234567890123456")).contains(&PiiKind::BankCard));
    }

    #[test]
    fn a7_audio_path() {
        assert!(kinds(&scan_raw("D:/录音/2024-03-01工资沟通.m4a")).contains(&PiiKind::AudioPath));
        assert!(kinds(&scan_raw("~/录音/与张经理通话.mp3")).contains(&PiiKind::AudioPath));
    }

    #[test]
    fn a8_medical_path() {
        assert!(kinds(&scan_raw("D:/病历/2024-03-15住院.pdf")).contains(&PiiKind::MedicalRecord));
    }

    #[test]
    fn phone_inside_longer_digit_run_rejected() {
        // 13800138000 embedded in a 13-digit run → digit boundary fails, no phone.
        assert!(!kinds(&scan_raw("12138001380000")).contains(&PiiKind::Phone));
    }
}
