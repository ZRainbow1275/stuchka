//! Second-stage numeric validators (`ai/04` §4.3).
//!
//! - [`luhn_check`] — Luhn mod-10 for bank cards (rejects 16-digit order numbers, A6).
//! - [`idcard_checksum`] — ISO 7064 mod 11-2 check digit for the 18-digit mainland resident id
//!   card (rejects malformed 18-digit strings, A4).

/// Luhn (mod 10) checksum. `digits` must be ASCII digits only; returns `false` for any non-digit
/// or empty input.
pub fn luhn_check(digits: &str) -> bool {
    let bytes = digits.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    // Process right-to-left; double every second digit from the rightmost.
    for (i, &b) in bytes.iter().rev().enumerate() {
        if !b.is_ascii_digit() {
            return false;
        }
        let mut d = (b - b'0') as u32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

/// ISO 7064 mod 11-2 check digit for an 18-character mainland resident id card.
///
/// The first 17 chars must be digits; the 18th is the check character (`0-9` or `X`/`x`).
/// Returns `false` for any length other than 18, non-digit body, or check-digit mismatch.
pub fn idcard_checksum(id18: &str) -> bool {
    let chars: Vec<char> = id18.chars().collect();
    if chars.len() != 18 {
        return false;
    }
    // Weights for positions 1..=17 (GB 11643 / ISO 7064 mod 11-2).
    const WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    let mut sum = 0u32;
    for i in 0..17 {
        let c = chars[i];
        if !c.is_ascii_digit() {
            return false;
        }
        let d = (c as u8 - b'0') as u32;
        sum += d * WEIGHTS[i];
    }
    // Check character lookup table for (12 - sum % 11) % 11.
    const CHECK: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
    let expected = CHECK[(sum % 11) as usize];
    let actual = chars[17].to_ascii_uppercase();
    actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luhn_valid_bank_card() {
        // 6222021234567894 is Luhn-valid (16-digit UnionPay-style, verified).
        assert!(luhn_check("6222021234567894"));
        // 19-digit UnionPay-style, verified Luhn-valid.
        assert!(luhn_check("6225881234567890120"));
        // Visa / Mastercard prefixes, verified.
        assert!(luhn_check("4000001234567899"));
        assert!(luhn_check("5100001234567895"));
    }

    #[test]
    fn luhn_rejects_order_number() {
        // A typical 16-digit order number that is not Luhn-valid (A6).
        assert!(!luhn_check("1234567890123456"));
    }

    #[test]
    fn luhn_rejects_non_digit() {
        assert!(!luhn_check("62220212a4567890"));
        assert!(!luhn_check(""));
    }

    #[test]
    fn idcard_valid_checksum() {
        // 11010119900101004X is ISO 7064 mod 11-2 valid (check char X), verified.
        assert!(idcard_checksum("11010119900101004X"));
        // 110101199001010074 is valid with a numeric check digit, verified.
        assert!(idcard_checksum("110101199001010074"));
    }

    #[test]
    fn idcard_rejects_bad_checksum() {
        // Correct body, deliberately wrong final char.
        assert!(!idcard_checksum("110101199001010070"));
        assert!(!idcard_checksum("11010119900101004Y"));
    }

    #[test]
    fn idcard_rejects_wrong_length() {
        assert!(!idcard_checksum("1101011990030761"));
        assert!(!idcard_checksum("11010119900307617XY"));
    }
}
