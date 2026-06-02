//! Layer 3 (zero-width) — paragraph-level zero-width-character watermark (compliance/01 §5.1).
//!
//! Each AI paragraph gets one 83-char frame of invisible zero-width characters encoding its
//! 64-bit `segment_id` plus a CRC-16/CCITT-FALSE checksum. The frame survives plain PDF
//! copy-paste (防剥离 grade A); the manifest (Layer 4) is the fallback when it does not.
//!
//! Frame (left→right): `[SEG_HEAD U+2060] [SENTINEL U+200D] <80-bit payload> [SENTINEL U+200D]`
//! payload = `segment_id(64, big-endian) ‖ crc16(16)`; bit0→U+200B, bit1→U+200C. Length is exactly
//! `1 + 1 + 80 + 1 = 83` characters.

use crc::{Crc, CRC_16_IBM_3740};

use crate::gb45438::manifest::AiSegment;
use crate::gb45438::Gb45438Error;
use crate::render::RenderDoc;

/// Word Joiner — frame head (alignment only, never a 0/1 bit).
const SEG_HEAD: char = '\u{2060}';
/// ZWJ — start/stop sentinel (alignment only).
const SENTINEL: char = '\u{200D}';
/// ZWSP — bit 0.
const BIT0: char = '\u{200B}';
/// ZWNJ — bit 1.
const BIT1: char = '\u{200C}';

/// Exact zero-width frame length in characters (compliance/01 §5.1.2).
pub const FRAME_LEN: usize = 83;
/// Payload bit count (`segment_id` 64 + crc16 16).
const PAYLOAD_BITS: usize = 80;

/// CRC-16/CCITT-FALSE (poly = 0x1021, init = 0xFFFF, no reflection, xorout = 0) — the `crc` crate
/// ships this exact parameterisation as `CRC_16_IBM_3740`.
const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_3740);

/// CRC-16/CCITT-FALSE over the 8 big-endian bytes of `segment_id` (compliance/01 §5.1.2).
fn crc16_ccitt_false(bytes: &[u8]) -> u16 {
    CRC16.checksum(bytes)
}

/// Encode one AI segment id into its 83-character zero-width frame (compliance/01 §5.1.2.1).
pub fn encode_frame(segment_id: u64) -> String {
    let crc = crc16_ccitt_false(&segment_id.to_be_bytes());
    let payload: u128 = ((segment_id as u128) << 16) | crc as u128; // 64 + 16 = 80 bit
    let mut frame = String::with_capacity(FRAME_LEN);
    frame.push(SEG_HEAD);
    frame.push(SENTINEL);
    for i in (0..PAYLOAD_BITS).rev() {
        let bit = (payload >> i) & 1;
        frame.push(if bit == 1 { BIT1 } else { BIT0 });
    }
    frame.push(SENTINEL);
    frame
}

/// Locate the 80-bit payload body inside a captured zero-width char slice: the run between the
/// leading `[U+2060 U+200D]` anchor and the trailing `U+200D` sentinel must be exactly 80 bit
/// chars (compliance/01 §5.1.2 decoder rule).
fn locate_body(zw: &[char]) -> Option<&[char]> {
    // find the start anchor SEG_HEAD followed by SENTINEL
    let head = zw
        .windows(2)
        .position(|w| w[0] == SEG_HEAD && w[1] == SENTINEL)?;
    let body_start = head + 2;
    // the body runs until the next SENTINEL
    let rel_end = zw[body_start..].iter().position(|&c| c == SENTINEL)?;
    let body = &zw[body_start..body_start + rel_end];
    Some(body)
}

/// Decode a captured zero-width sequence back into a `segment_id`; a length or CRC mismatch returns
/// `None` (never panics) so a corrupted frame is gracefully ignored and Layer 4 takes over.
pub fn decode_frame(zw: &[char]) -> Option<u64> {
    let body = locate_body(zw)?;
    if body.len() != PAYLOAD_BITS {
        return None;
    }
    let mut payload: u128 = 0;
    for &c in body {
        let bit = match c {
            BIT1 => 1u128,
            BIT0 => 0u128,
            _ => return None,
        };
        payload = (payload << 1) | bit;
    }
    let segment_id = (payload >> 16) as u64;
    let crc = (payload & 0xFFFF) as u16;
    (crc16_ccitt_false(&segment_id.to_be_bytes()) == crc).then_some(segment_id)
}

/// Inject the final zero-width frame before each AI paragraph (D5: only the backend injects the
/// final frame). Idempotent — re-running overwrites the same block's frame.
pub fn inject_into_segments(doc: &mut RenderDoc, segs: &[AiSegment]) {
    for seg in segs {
        let frame = encode_frame(seg.segment_id);
        doc.prepend_invisible_run(seg.paragraph_ref, &frame);
    }
}

/// Self-check (compliance/01 §6.4): every segment's frame must be present on its block and decode
/// back to the same `segment_id`. Returns `Ok(true)` only when the whole round-trip holds.
pub fn verify_roundtrip(doc: &RenderDoc, segs: &[AiSegment]) -> Result<bool, Gb45438Error> {
    for seg in segs {
        let block = doc
            .blocks
            .iter()
            .find(|b| b.paragraph_ref == Some(seg.paragraph_ref));
        let Some(block) = block else {
            return Ok(false);
        };
        if block.zerowidth_frame.is_empty() {
            return Ok(false);
        }
        let chars: Vec<char> = block.zerowidth_frame.chars().collect();
        if chars.len() != FRAME_LEN {
            return Ok(false);
        }
        match decode_frame(&chars) {
            Some(id) if id == seg.segment_id => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Decode every zero-width frame found in the document's AI blocks into a sorted set of ids
/// (mirrors the offline `extract_zerowidth.py` for the GB-02 1:1 assertion).
pub fn decode_all(doc: &RenderDoc) -> std::collections::BTreeSet<u64> {
    let mut out = std::collections::BTreeSet::new();
    for block in &doc.blocks {
        if block.zerowidth_frame.is_empty() {
            continue;
        }
        let chars: Vec<char> = block.zerowidth_frame.chars().collect();
        if let Some(id) = decode_frame(&chars) {
            out.insert(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_length_is_always_83() {
        for id in [0u64, 1, 0x1a3f, u64::MAX, 0xdead_beef_cafe_babe] {
            assert_eq!(encode_frame(id).chars().count(), FRAME_LEN, "id={id:#x}");
        }
    }

    #[test]
    fn encode_decode_round_trip() {
        for id in [0u64, 1, 42, 0x1a3f, u64::MAX, 0x0123_4567_89ab_cdef] {
            let frame = encode_frame(id);
            let chars: Vec<char> = frame.chars().collect();
            assert_eq!(decode_frame(&chars), Some(id), "id={id:#x}");
        }
    }

    #[test]
    fn frame_contains_only_zero_width_chars() {
        let frame = encode_frame(0xabcd);
        for c in frame.chars() {
            assert!(
                matches!(c, SEG_HEAD | SENTINEL | BIT0 | BIT1),
                "non-zero-width char {c:?}"
            );
        }
    }

    #[test]
    fn corrupted_crc_decodes_to_none_without_panic() {
        let mut chars: Vec<char> = encode_frame(0x1a3f).chars().collect();
        // flip one payload bit (index 2 is the first payload char) → CRC must fail
        chars[2] = if chars[2] == BIT0 { BIT1 } else { BIT0 };
        assert_eq!(decode_frame(&chars), None);
    }

    #[test]
    fn wrong_length_body_is_none() {
        // a body of 79 bits between the anchors must be rejected (not 80)
        let mut chars = vec![SEG_HEAD, SENTINEL];
        chars.extend(std::iter::repeat_n(BIT0, 79));
        chars.push(SENTINEL);
        assert_eq!(decode_frame(&chars), None);
    }

    #[test]
    fn empty_or_garbage_input_is_none() {
        assert_eq!(decode_frame(&[]), None);
        assert_eq!(decode_frame(&['a', 'b', 'c']), None);
        assert_eq!(decode_frame(&[SEG_HEAD]), None);
    }
}
