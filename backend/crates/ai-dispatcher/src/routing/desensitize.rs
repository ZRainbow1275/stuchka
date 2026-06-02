//! 境内云脱敏管线 (`compliance/02` §3.4 + INV-05 + 法律 P9).
//!
//! Before a domestic-cloud request carrying L3 fields leaves the device it must pass through this
//! pipeline: every PII span the high-sensitivity detector (`crates/hsd`) finds is masked (reusing
//! the SAME `hsd::mask` rendering the UI uses — no second masking implementation, D4), then the
//! masked payload is re-scanned. If a strong PII signal still survives the mask, the pipeline
//! reports `residual_pii = true` and the guard MUST downgrade to the local model (§3.4: 否则降级到
//! 本地小钢炮). The masking is byte-exact and zero-network (the detector is offline, pure CPU).

use hsd::{mask, HsdDetector};

/// The result of running the desensitisation pipeline over a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desensitized {
    /// The masked payload (PII spans replaced; safe to inspect / preview, §4.4).
    pub masked_text: String,
    /// Whether anything was masked at all.
    pub applied: bool,
    /// `true` when a strong PII signal survived masking → the guard must force local (§3.4).
    pub residual_pii: bool,
}

/// Desensitise `text` using `detector` (`compliance/02` §3.4). Masks every detected PII span via
/// `hsd::mask`, then re-scans the masked output; a surviving strong signal sets `residual_pii`.
pub fn desensitize(detector: &HsdDetector, text: &str) -> Desensitized {
    let report = detector.scan(text);
    if report.hits.is_empty() {
        return Desensitized {
            masked_text: text.to_string(),
            applied: false,
            residual_pii: false,
        };
    }

    // Mask every hit span (phone 138****8000 / idcard 110101********1234 / bankcard ...7894 /
    // address + name + path whole-span, per hsd::mask which implements §3.4 exactly).
    let masked_text = mask(text, &report.hits);

    // §3.4: 脱敏后的载荷必须再经 NER 二次扫描，确保零漏 PII；否则降级到本地小钢炮。
    let rescan = detector.scan(&masked_text);
    let residual_pii = rescan.is_high_sensitive;

    Desensitized {
        masked_text,
        applied: true,
        residual_pii,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> HsdDetector {
        HsdDetector::new_regex_only()
    }

    #[test]
    fn masks_phone_for_domestic_cloud() {
        let d = desensitize(&detector(), "我的电话13800138000，请联系。");
        assert!(d.applied);
        assert!(d.masked_text.contains("138****8000"));
        assert!(!d.masked_text.contains("13800138000"));
        // The masked rendering no longer trips the strong-signal detector.
        assert!(!d.residual_pii, "masked phone must not be a residual strong signal");
    }

    #[test]
    fn no_pii_is_a_noop() {
        let d = desensitize(&detector(), "公司拖欠工资三个月，要求支付。");
        assert!(!d.applied);
        assert!(!d.residual_pii);
        assert_eq!(d.masked_text, "公司拖欠工资三个月，要求支付。");
    }

    #[test]
    fn masks_idcard() {
        // A structurally valid resident id with a correct ISO 7064 mod-11-2 check digit (`7`); the
        // HSD layer rejects bad-checksum candidates, so the fixture must carry a real one.
        let d = desensitize(&detector(), "身份证110101199001011237");
        assert!(d.applied);
        assert!(d.masked_text.contains("110101********1237"));
        assert!(!d.residual_pii);
    }
}
