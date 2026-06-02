//! Layer 3 (LSB) — image least-significant-bit watermark (compliance/01 §5.2).
//!
//! R1a embeds a 256-bit payload (`segment_id 64 ‖ doc_id-low 64 ‖ checksum 64 ‖ reserved 64`) into
//! the cover image of the dossier (one image only, §5.3). The embed is a real spatial-domain LSB
//! over the RGB channels of a PNG raster, which round-trips losslessly through PNG re-encoding (the
//! DCT-domain strong-adversary variant — rated F — is explicitly out of R1a scope, §5.2.3).

use image::{ImageBuffer, Rgba};

/// Payload size in bits (compliance/01 §5.2.1: ≥256 bit).
pub const PAYLOAD_BITS: usize = 256;
/// Payload size in bytes.
pub const PAYLOAD_BYTES: usize = PAYLOAD_BITS / 8;

/// A 32-byte LSB payload: `segment_id(8) ‖ doc_id_low(8) ‖ checksum(8) ‖ reserved(8)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LsbPayload(pub [u8; PAYLOAD_BYTES]);

impl LsbPayload {
    /// Build the payload from the segment id + doc-id low 64 bits; the checksum is the FNV-1a of
    /// the first 16 bytes and the reserved field is zero (R1a).
    pub fn new(segment_id: u64, doc_id_low: u64) -> Self {
        let mut buf = [0u8; PAYLOAD_BYTES];
        buf[0..8].copy_from_slice(&segment_id.to_be_bytes());
        buf[8..16].copy_from_slice(&doc_id_low.to_be_bytes());
        let checksum = fnv1a64(&buf[0..16]);
        buf[16..24].copy_from_slice(&checksum.to_be_bytes());
        // reserved [24..32] stays zero
        Self(buf)
    }

    /// The segment id field.
    pub fn segment_id(&self) -> u64 {
        u64::from_be_bytes(self.0[0..8].try_into().unwrap())
    }

    /// Verify the embedded checksum (returns `true` when intact).
    pub fn checksum_ok(&self) -> bool {
        let expect = fnv1a64(&self.0[0..16]);
        let got = u64::from_be_bytes(self.0[16..24].try_into().unwrap());
        expect == got
    }
}

/// FNV-1a 64 over a byte slice (compact, dependency-free checksum for the reserved field).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Embed `payload` into the RGBA raster via spatial LSB over the R/G/B channels (alpha untouched so
/// transparency is preserved). Errors when the image is too small for 256 bits.
pub fn embed(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, payload: &LsbPayload) -> Result<(), String> {
    let capacity = (img.width() as usize) * (img.height() as usize) * 3; // 3 LSBs per pixel
    if capacity < PAYLOAD_BITS {
        return Err(format!(
            "cover image too small: capacity {capacity} bits < {PAYLOAD_BITS}"
        ));
    }
    let bits = bytes_to_bits(&payload.0);
    let mut bit_idx = 0usize;
    'outer: for pixel in img.pixels_mut() {
        for ch in 0..3 {
            if bit_idx >= bits.len() {
                break 'outer;
            }
            pixel.0[ch] = (pixel.0[ch] & 0xFE) | bits[bit_idx];
            bit_idx += 1;
        }
    }
    Ok(())
}

/// Decode the 256-bit payload from the first `PAYLOAD_BITS` channel LSBs. Returns `None` if the
/// image is too small.
pub fn decode(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Option<LsbPayload> {
    let capacity = (img.width() as usize) * (img.height() as usize) * 3;
    if capacity < PAYLOAD_BITS {
        return None;
    }
    let mut bits = Vec::with_capacity(PAYLOAD_BITS);
    'outer: for pixel in img.pixels() {
        for ch in 0..3 {
            if bits.len() >= PAYLOAD_BITS {
                break 'outer;
            }
            bits.push(pixel.0[ch] & 1);
        }
    }
    let bytes = bits_to_bytes(&bits);
    let mut buf = [0u8; PAYLOAD_BYTES];
    buf.copy_from_slice(&bytes);
    Some(LsbPayload(buf))
}

fn bytes_to_bits(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 8);
    for &byte in bytes {
        for i in (0..8).rev() {
            out.push((byte >> i) & 1);
        }
    }
    out
}

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|chunk| chunk.iter().fold(0u8, |acc, &b| (acc << 1) | (b & 1)))
        .collect()
}

/// Generate a deterministic solid PNG cover canvas of `w`×`h` (the dossier cover background before
/// LSB embedding; the visible cover art is a frontend concern, the backend只需 a real raster to
/// carry the watermark, §5.3 "1 only").
pub fn blank_cover(w: u32, h: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    // 印章红 #9d2f24 tinted background (Prototype tokens.css --seal-500) at low opacity over white.
    ImageBuffer::from_fn(w, h, |x, y| {
        let shade = ((x.wrapping_add(y)) % 16) as u8;
        Rgba([0xf5 - shade / 4, 0xf0, 0xee, 0xff])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn payload_round_trips_through_raster() {
        let mut img = blank_cover(64, 64);
        let payload = LsbPayload::new(0xdead_beef_cafe_babe, 0x0123_4567_89ab_cdef);
        embed(&mut img, &payload).unwrap();
        let got = decode(&img).unwrap();
        assert_eq!(got, payload);
        assert_eq!(got.segment_id(), 0xdead_beef_cafe_babe);
        assert!(got.checksum_ok());
    }

    #[test]
    fn payload_survives_png_reencode() {
        let mut img = blank_cover(64, 64);
        let payload = LsbPayload::new(7, 9);
        embed(&mut img, &payload).unwrap();

        // encode to PNG bytes and decode back (lossless)
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let reloaded = image::load_from_memory(buf.get_ref()).unwrap().to_rgba8();
        let got = decode(&reloaded).unwrap();
        assert_eq!(got, payload);
        assert!(got.checksum_ok());
    }

    #[test]
    fn too_small_image_errors() {
        let mut img = blank_cover(4, 4); // 48 bits capacity < 256
        let err = embed(&mut img, &LsbPayload::new(1, 1)).unwrap_err();
        assert!(err.contains("too small"));
        assert!(decode(&img).is_none());
    }

    #[test]
    fn checksum_detects_tamper() {
        let mut img = blank_cover(32, 32);
        let payload = LsbPayload::new(5, 6);
        embed(&mut img, &payload).unwrap();
        let mut got = decode(&img).unwrap();
        got.0[2] ^= 0xFF; // flip a segment-id byte
        assert!(!got.checksum_ok());
    }
}
