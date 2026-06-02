//! Text normalization with span back-mapping (`ai/04` §4.3, brief §7.1).
//!
//! Regex rules run against a *normalized* form (fullwidth→halfwidth, traditional→simplified for a
//! curated digit/keyword subset, ASCII whitespace folded). To keep `PiiHit.span` pointing at the
//! ORIGINAL text (so api/frontend highlight the right bytes, A11), [`NormalizedText`] carries two
//! byte-offset maps:
//! - `start_map[i]` = original *start* byte of the source char for normalized byte `i`,
//! - `end_map[i]`   = original *exclusive-end* byte of that same source char.
//!
//! `map_span(s, e)` then returns `(start_map[s], end_map[e-1])`, which excludes any dropped
//! trailing whitespace between the span's last char and the next char (otherwise the next char's
//! start offset would over-extend the span past dropped spaces).

/// Normalized text plus byte-offset maps back to the original (`ai/04` §4.3 / brief §3).
#[derive(Debug, Clone)]
pub struct NormalizedText {
    /// The normalized text the regex layer scans.
    pub text: String,
    /// `offset_map[i]` = original *start* byte of the source char for normalized byte `i`.
    /// Has `text.len() + 1` entries; the last is the original-text length (sentinel).
    pub offset_map: Vec<usize>,
    /// `end_map[i]` = original *exclusive-end* byte of the source char for normalized byte `i`.
    /// Same length as `text` (no sentinel needed; the end side reads `end_map[end-1]`).
    pub end_map: Vec<usize>,
}

impl NormalizedText {
    /// Map a `[start, end)` byte span in the normalized text back to the original text.
    ///
    /// `start` maps to its source char's start; the exclusive `end` maps to the exclusive end of
    /// the span's last source char (`end_map[end - 1]`), which correctly excludes dropped
    /// whitespace that may sit between the span and the following character.
    pub fn map_span(&self, start: usize, end: usize) -> (usize, usize) {
        let orig_start = self.offset_map.get(start).copied().unwrap_or(0);
        let orig_end = if end == 0 {
            orig_start
        } else {
            self.end_map
                .get(end - 1)
                .copied()
                .unwrap_or_else(|| *self.offset_map.last().unwrap_or(&0))
        };
        (orig_start, orig_end)
    }
}

/// Normalize `raw`: fullwidth→halfwidth, a curated traditional→simplified digit/keyword subset,
/// and ASCII whitespace stripped between tokens that the strong-signal regexes expect contiguous.
///
/// Whitespace handling: a run of ASCII whitespace is dropped entirely (so `"138 0013 8000"` →
/// `"13800138000"` and the phone rule matches). Non-ASCII text and punctuation are preserved so
/// address / path rules still see their context. The offset map always points back to a real
/// original byte.
pub fn normalize(raw: &str) -> NormalizedText {
    let mut text = String::with_capacity(raw.len());
    let mut offset_map: Vec<usize> = Vec::with_capacity(raw.len() + 1);
    let mut end_map: Vec<usize> = Vec::with_capacity(raw.len());

    for (byte_idx, ch) in raw.char_indices() {
        // Drop ASCII whitespace so spaced digit groups become contiguous for strong-signal rules.
        if ch.is_ascii_whitespace() {
            continue;
        }
        let char_end = byte_idx + ch.len_utf8();
        let mapped = map_char(ch);
        for out_ch in mapped {
            let mut buf = [0u8; 4];
            let s = out_ch.encode_utf8(&mut buf);
            for _ in 0..s.len() {
                // Each output byte points back to the start / exclusive-end of its source char.
                offset_map.push(byte_idx);
                end_map.push(char_end);
            }
            text.push_str(s);
        }
    }
    // Sentinel: exclusive-end byte of the final char maps to the original length.
    offset_map.push(raw.len());

    NormalizedText {
        text,
        offset_map,
        end_map,
    }
}

/// Map a single char: fullwidth ASCII → halfwidth, fullwidth space → (dropped via caller),
/// a small traditional→simplified set covering address/path keywords, else identity.
fn map_char(ch: char) -> [char; 1] {
    // Fullwidth ASCII variants U+FF01..U+FF5E map to U+0021..U+007E (offset 0xFEE0).
    let c = ch as u32;
    if (0xFF01..=0xFF5E).contains(&c) {
        return [char::from_u32(c - 0xFEE0).unwrap_or(ch)];
    }
    // Fullwidth space → halfwidth space (then folded by the whitespace check on the next pass is
    // not possible here, so emit ASCII space which downstream rules tolerate / drop is handled by
    // caller only for ASCII; emit ' ').
    if c == 0x3000 {
        return [' '];
    }
    // Curated traditional→simplified for keywords used by path / address rules.
    let simplified = match ch {
        '錄' => '录',
        '電' => '电',
        '話' => '话',
        '單' => '单',
        '據' => '据',
        '證' => '证',
        '醫' => '医',
        '療' => '疗',
        '診' => '诊',
        '斷' => '断',
        '書' => '书',
        '傷' => '伤',
        '鑑' => '鉴',
        '門' => '门',
        '區' => '区',
        '縣' => '县',
        '鄉' => '乡',
        '鎮' => '镇',
        '號' => '号',
        '樓' => '楼',
        '張' => '张',
        '經' => '经',
        '理' => '理',
        '蘇' => '苏',
        _ => ch,
    };
    [simplified]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullwidth_digits_to_halfwidth() {
        // Fullwidth "１３８" → "138".
        let n = normalize("１３８");
        assert_eq!(n.text, "138");
    }

    #[test]
    fn whitespace_is_dropped_for_digit_groups() {
        let n = normalize("138 0013 8000");
        assert_eq!(n.text, "13800138000");
    }

    #[test]
    fn span_maps_back_to_original_with_spaces() {
        // "+86 138 0013 8000": normalized "8613800138000". The 11-digit phone "13800138000"
        // starts at normalized byte 2 (after "86"). Mapping back must land on the original '1'.
        let raw = "+86 138 0013 8000";
        let n = normalize(raw);
        assert_eq!(n.text, "+8613800138000");
        let pos = n.text.find("13800138000").unwrap();
        let (os, oe) = n.map_span(pos, pos + "13800138000".len());
        assert_eq!(&raw[os..oe], "138 0013 8000");
    }

    #[test]
    fn traditional_keyword_to_simplified() {
        let n = normalize("錄音");
        assert_eq!(n.text, "录音");
    }

    #[test]
    fn offset_map_has_sentinel() {
        let n = normalize("abc");
        assert_eq!(n.offset_map.len(), n.text.len() + 1);
        assert_eq!(*n.offset_map.last().unwrap(), 3);
    }
}
