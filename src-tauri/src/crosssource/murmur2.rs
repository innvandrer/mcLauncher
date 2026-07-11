//! CurseForge file fingerprinting: 32-bit MurmurHash2 (seed 1) over the file
//! bytes with whitespace (tab, LF, CR, space) removed. This is the exact
//! algorithm the CurseForge `/v1/fingerprints` endpoint matches against, so it
//! must stay bit-for-bit compatible — don't "fix" the overflow arithmetic.

const SEED: u32 = 1;
const M: u32 = 0x5bd1_e995;
const R: u32 = 24;

/// Bytes CurseForge strips before hashing.
fn is_cf_whitespace(b: u8) -> bool {
    matches!(b, 9 | 10 | 13 | 32)
}

/// 32-bit MurmurHash2 (Austin Appleby's reference algorithm) with the
/// CurseForge seed.
pub fn murmur2(data: &[u8]) -> u32 {
    let len = data.len();
    let mut h: u32 = SEED ^ (len as u32);

    let mut chunks = data.chunks_exact(4);
    for chunk in &mut chunks {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }

    let rem = chunks.remainder();
    if rem.len() >= 3 {
        h ^= (rem[2] as u32) << 16;
    }
    if rem.len() >= 2 {
        h ^= (rem[1] as u32) << 8;
    }
    if !rem.is_empty() {
        h ^= rem[0] as u32;
        h = h.wrapping_mul(M);
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// CurseForge fingerprint of an in-memory buffer: murmur2 of the buffer with
/// whitespace bytes removed.
pub fn cf_fingerprint(data: &[u8]) -> u32 {
    // Hash needs the normalized length up front, so filter into a buffer.
    let normalized: Vec<u8> = data
        .iter()
        .copied()
        .filter(|b| !is_cf_whitespace(*b))
        .collect();
    murmur2(&normalized)
}

/// CurseForge fingerprint of a file on disk.
pub async fn cf_fingerprint_file(path: &std::path::Path) -> crate::error::Result<u32> {
    let bytes = tokio::fs::read(path).await?;
    Ok(cf_fingerprint(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values generated with an independent transcription of the
    // MurmurHash2 reference implementation (seed 1).
    #[test]
    fn murmur2_reference_vectors() {
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        let cases: &[(&[u8], u32)] = &[
            (b"", 1_540_447_798),
            (b"a", 626_045_324),
            (b"ab", 1_692_487_918),
            (b"abc", 1_621_425_345),
            (b"abcd", 3_376_380_438),
            (b"abcde", 3_469_237_630),
            (b"hello world", 2_213_174_766),
            (
                b"The quick brown fox jumps over the lazy dog",
                504_383_975,
            ),
            (&all_bytes, 253_525_554),
        ];
        for (input, expected) in cases {
            assert_eq!(murmur2(input), *expected, "input {input:?}");
        }
    }

    #[test]
    fn fingerprint_strips_curseforge_whitespace() {
        // With tab/LF/CR/space removed, "a b\tc\r\nd" hashes like "abcd".
        assert_eq!(cf_fingerprint(b"a b\tc\r\nd"), murmur2(b"abcd"));
        assert_eq!(cf_fingerprint(b"a b\tc\r\nd"), 3_376_380_438);
        assert_eq!(cf_fingerprint(b"  leading and trailing  "), 2_571_987_805);
        assert_eq!(cf_fingerprint(b"PK\x03\x04 fake jar \r\n\x00\x01"), 3_461_783_834);
    }

    #[test]
    fn fingerprint_keeps_other_control_bytes() {
        // Only 9/10/13/32 are stripped — NUL, vertical tab, form feed stay.
        assert_ne!(cf_fingerprint(b"a\x00b"), cf_fingerprint(b"ab"));
        assert_ne!(cf_fingerprint(b"a\x0bb"), cf_fingerprint(b"ab"));
        assert_ne!(cf_fingerprint(b"a\x0cb"), cf_fingerprint(b"ab"));
    }
}
