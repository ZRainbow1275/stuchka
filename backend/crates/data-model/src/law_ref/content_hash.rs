//! D8 content-addressing hash (`data/02` §2.3). Normalised-text SHA-256 of a law clause body.
//!
//! The hash drops all Unicode whitespace and maps Chinese / full-width punctuation to its
//! half-width equivalent before hashing, so KB maintainers reformatting whitespace or swapping
//! punctuation width does NOT drift the hash (INV-04 freeze boundary). Substantive character
//! changes (汉字 / 数字) DO change the hash. The normalisation table is a deterministic constant
//! mapping — never locale-dependent — so the hex output is reproducible across platforms (LR-03).

use sha2::{Digest, Sha256};

/// Compute the SHA-256 (lower-case hex, 64 chars) of `body` after deterministic normalisation
/// (`data/02` §2.3). Whitespace is stripped and punctuation is folded to half-width; substantive
/// characters are preserved.
pub fn content_hash(body: &str) -> String {
    let normalized: String = body
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(normalize_punct)
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Deterministic punctuation normalisation: full-width / curly punctuation → half-width
/// (`data/02` §2.3). Opening and closing brackets map to distinct half-width forms (NOT collapsed
/// to one character, per the §2.3 implementation note). Any character not in the table is returned
/// unchanged.
pub fn normalize_punct(c: char) -> char {
    match c {
        '\u{201C}' | '\u{201D}' | '\u{FF02}' => '"', // " " ＂ → "
        '\u{2018}' | '\u{2019}' | '\u{FF07}' => '\'', // ' ' ＇ → '
        '\u{FF0C}' | '\u{3001}' => ',',              // ， 、 → ,
        '\u{3002}' => '.',                           // 。 → .
        '\u{FF1B}' => ';',                           // ； → ;
        '\u{FF1A}' => ':',                           // ： → :
        '\u{FF01}' => '!',                           // ！ → !
        '\u{FF1F}' => '?',                           // ？ → ?
        '\u{FF08}' => '(',                           // （ → (  (opening)
        '\u{FF09}' => ')',                           // ） → )  (closing, distinct from opening)
        '\u{3010}' => '[',                           // 【 → [
        '\u{3011}' => ']',                           // 】 → ]
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LR-01: whitespace changes and full-width↔half-width punctuation swaps do NOT change the hash.
    #[test]
    fn lr01_whitespace_and_punct_invariant() {
        let canonical = "劳动者有下列情形之一的,用人单位可以解除劳动合同:";
        let with_whitespace = "劳动者有下列情形之一的，用人单位可以解除劳动合同： \n\t";
        let with_fullwidth_punct = "劳动者有下列情形之一的，用人单位可以解除劳动合同：";
        assert_eq!(content_hash(canonical), content_hash(with_whitespace));
        assert_eq!(content_hash(canonical), content_hash(with_fullwidth_punct));

        // curly quotes / full-width quotes fold to the same half-width form
        let a = "他说“这是工资”。";
        let b = "他说\"这是工资\".";
        assert_eq!(content_hash(a), content_hash(b));
    }

    /// LR-02: a substantive character change DOES change the hash.
    #[test]
    fn lr02_substantive_change_differs() {
        let original = "经济补偿按劳动者在本单位工作的年限";
        let changed = "经济赔偿按劳动者在本单位工作的年限"; // 补→赔
        assert_ne!(content_hash(original), content_hash(changed));

        let num_a = "支付三个月工资";
        let num_b = "支付二个月工资";
        assert_ne!(content_hash(num_a), content_hash(num_b));
    }

    /// LR-03: normalisation is deterministic — a fixed input yields a fixed hex constant
    /// (locale-independent, cross-platform stable).
    #[test]
    fn lr03_fixed_input_fixed_hex() {
        // SHA-256 of the empty (fully-stripped) string is the well-known constant.
        assert_eq!(
            content_hash("   \n\t  "),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // SHA-256("a") — a single ASCII char survives normalisation unchanged.
        assert_eq!(
            content_hash("a"),
            "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"
        );
    }

    /// Opening and closing brackets must NOT collapse to a single character (§2.3 note).
    #[test]
    fn brackets_are_distinct() {
        assert_eq!(normalize_punct('\u{FF08}'), '(');
        assert_eq!(normalize_punct('\u{FF09}'), ')');
        assert_ne!(normalize_punct('\u{FF08}'), normalize_punct('\u{FF09}'));
    }

    #[test]
    fn hash_is_64_hex_chars() {
        let h = content_hash("任意正文");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }
}
