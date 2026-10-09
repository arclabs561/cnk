//! Single-bit flips in an enveloped set must not decode to a different set.
//!
//! The V2 envelope's CRC32 covers the payload, so every payload flip must be
//! rejected. Header fields are checked against the payload instead: a flipped
//! tag, partition size, count or length must fail to decode. The universe
//! field is the exception: a flip that keeps every id below it decodes to the
//! same ids with a different reported universe, because no checksum covers it.

use cnk::{compress_set_enveloped, decompress_set_enveloped, ChooseConfig};

/// Byte range of the V2 header's `u` (universe) field: after the 8-byte magic,
/// the 1-byte tag and the 4-byte partition size.
const UNIVERSE_FIELD: std::ops::Range<usize> = 13..17;

#[test]
fn single_bit_flips_never_decode_a_different_set() {
    let ids: Vec<u32> = (0..200u32).map(|i| i * 7 + (i % 3)).collect();
    let bytes = compress_set_enveloped(&ids, 10_000, ChooseConfig::default()).unwrap();
    let (_, _, restored) = decompress_set_enveloped(&bytes).unwrap();
    assert_eq!(restored, ids);

    for bit in 0..bytes.len() * 8 {
        let mut flipped = bytes.clone();
        flipped[bit / 8] ^= 1 << (bit % 8);
        if let Ok((_, _, decoded)) = decompress_set_enveloped(&flipped) {
            assert_eq!(decoded, ids, "flip of bit {bit} decoded a different set");
            assert!(
                UNIVERSE_FIELD.contains(&(bit / 8)),
                "flip of bit {bit} (byte {}) outside the universe field decoded",
                bit / 8
            );
        }
    }
}
