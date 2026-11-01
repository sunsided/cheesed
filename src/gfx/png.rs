//! A minimal PNG encoder (uncompressed deflate blocks), so headless snapshots of the
//! framebuffer need no image crate.
//!
//! A PNG file is an 8-byte signature followed by chunks. Each chunk is a big-endian `u32`
//! data length, a 4-byte type (`IHDR`, `IDAT`, `IEND`, ...), the data, and a CRC-32 over
//! the type and data. This encoder writes the minimum three chunks:
//!
//! - `IHDR`: width, height, bit depth 8, color type 2 (RGB), compression 0, filter method
//!   0, no interlacing.
//! - `IDAT`: the image as a zlib stream. The raw data is one filter-type byte per row
//!   (always 0, "none") followed by the row's RGB bytes.
//! - `IEND`: empty, marks the end.
//!
//! The zlib stream must be valid deflate, but deflate allows "stored" blocks that hold
//! bytes verbatim, so no compression code is needed. A 320x200 snapshot at scale 3 is
//! about 1.7 MB, which is fine for the screenshots the snapshot example writes.

use super::{H, Screen, W};

/// Encode `screen` upscaled by `scale` as an RGB PNG, returning the complete file bytes.
///
/// The upscaling is nearest-neighbour: output pixel `(x, y)` takes framebuffer pixel
/// `(x / scale, y / scale)`, so each framebuffer pixel becomes a `scale` x `scale` block,
/// matching how the game window shows it. `scale` must be at least 1.
pub fn encode(screen: &Screen, scale: u32) -> Vec<u8> {
    let (w, h) = (W as u32 * scale, H as u32 * scale);
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0); // filter: none
        for x in 0..w {
            let p = screen.get((x / scale) as i32, (y / scale) as i32);
            raw.extend_from_slice(&[p.0, p.1, p.2]);
        }
    }

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Append one PNG chunk to `out`: the big-endian data length, the 4-byte chunk type, the
/// data, and the big-endian CRC-32 of type plus data (the length is not covered by the CRC).
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Wrap `data` in a zlib stream made only of stored (uncompressed) deflate blocks.
///
/// Layout:
///
/// - The 2-byte zlib header `78 01`: deflate with a 32 KiB window, no preset dictionary,
///   "fastest" compression level; the pair is chosen so `0x7801` is a multiple of 31, as zlib
///   requires.
/// - One stored block per 65535 bytes of input (the most a stored block can hold). Each block
///   starts byte-aligned with a 3-bit header in the low bits of one byte: BFINAL (1 only
///   on the last block) and BTYPE = 00 (stored). A stored block then skips to the next
///   byte boundary, so the rest of that byte is padding and the whole header is the
///   single byte 0 or 1. LEN (a little-endian `u16`) and NLEN (its one's complement)
///   follow, then the raw bytes.
/// - The Adler-32 checksum of the uncompressed data, big-endian. Adler-32 keeps two sums
///   modulo 65521 (the largest prime below 2^16): `a` is 1 plus the sum of all bytes and
///   `b` is the sum of every intermediate `a`; the checksum is `b << 16 | a`.
///
/// Empty input would produce no blocks at all, which is not a valid deflate stream; the
/// encoder never passes empty data because every row has at least its filter byte.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// CRC-32 as used by PNG (and zlib, gzip, Ethernet): the reflected polynomial
/// `0xEDB88320`, initial value `0xFFFFFFFF`, final XOR with `0xFFFFFFFF`.
///
/// This is the bit-at-a-time form without a lookup table: each byte is XORed into the low end
/// of the register, then eight times the register shifts right by one and, if the bit shifted
/// out was 1, is XORed with the polynomial. Slow compared to a table, but it only runs over
/// one snapshot at a time.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    #[test]
    /// Every PNG ends with the same `IEND` chunk, so its CRC (`AE 42 60 82`) is a well-known
    /// constant to check the implementation against.
    fn crc_of_iend_is_known() {
        assert_eq!(super::crc32(b"IEND"), 0xae42_6082);
    }
}
