//! Stable “envelope” encoding for compressed ID sets.
//!
//! Motivation: downstream repos often want to persist compressed bytes without separately persisting
//! the method/parameters used. An envelope prepends a small header (method + params + universe) so
//! decode is self-describing and auditable.
//!
//! This is intentionally minimal and versioned; it is **not** a general container format.
//!
//! ## Wire format
//!
//! The envelope is:
//!
//! - a fixed-width header (little-endian integers)
//! - followed by the raw codec payload bytes (as returned by `compress_set_auto`)
//!
//! The format is versioned by an 8-byte ASCII magic.
//!
//! **V1** (`CNKENV01`) (legacy; still decodable):
//!
//! ```text
//! +------------+------------+--------------------+----------------+------------------+-----------+
//! | magic[8]   | tag[u8]    | pbs[u32 LE]        | u[u32 LE]      | len[u64 LE]      | payload   |
//! +------------+------------+--------------------+----------------+------------------+-----------+
//! ```
//!
//! **V2** (`CNKENV02`) (current; produced by `compress_set_enveloped`):
//!
//! ```text
//! +------------+------------+--------------------+----------------+--------------+------------------+--------------+-----------+
//! | magic[8]   | tag[u8]    | pbs[u32 LE]        | u[u32 LE]      | n[u32 LE]    | len[u64 LE]      | crc[u32 LE]  | payload   |
//! +------------+------------+--------------------+----------------+--------------+------------------+--------------+-----------+
//! ```
//!
//! Where:
//! - `tag` selects the codec (`IdCompressionMethod`) using the stable mapping below.
//! - `pbs` is the partition block size for `PartitionedEliasFano`; otherwise it must be 0.
//! - `u` is the universe size used for compression (`universe_size`).
//! - `n` is the number of IDs in the set.
//! - `len` is the number of payload bytes.
//! - `crc` is IEEE CRC32 over the payload bytes.

use crate::choose::CodecChoice;
use crate::{compress_set_auto, AutoConfig, CompressionError, IdCompressionMethod};

const MAGIC_V1: &[u8; 8] = b"CNKENV01";
const MAGIC_V2: &[u8; 8] = b"CNKENV02";

fn method_tag(m: &IdCompressionMethod) -> u8 {
    match m {
        IdCompressionMethod::None => 0,
        IdCompressionMethod::Roc => 1,
        IdCompressionMethod::EliasFano => 2,
        IdCompressionMethod::PartitionedEliasFano => 3,
        IdCompressionMethod::WaveletTree => 4,
    }
}

fn tag_method(tag: u8) -> Result<IdCompressionMethod, CompressionError> {
    match tag {
        0 => Ok(IdCompressionMethod::None),
        1 => Ok(IdCompressionMethod::Roc),
        2 => Ok(IdCompressionMethod::EliasFano),
        3 => Ok(IdCompressionMethod::PartitionedEliasFano),
        4 => Ok(IdCompressionMethod::WaveletTree),
        _ => Err(CompressionError::DecompressionFailed(format!(
            "unknown envelope method tag: {tag}"
        ))),
    }
}

fn crc32(payload: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(payload);
    h.finalize()
}

fn normalize_choice(mut choice: CodecChoice) -> CodecChoice {
    match choice.method {
        IdCompressionMethod::PartitionedEliasFano => {
            choice.partition_block_size = choice.partition_block_size.max(1);
        }
        _ => {
            choice.partition_block_size = 0;
        }
    }
    choice
}

#[cfg(test)]
fn encode_v1(choice: &CodecChoice, universe_size: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 1 + 4 + 4 + 8 + payload.len());
    out.extend_from_slice(MAGIC_V1);
    out.push(method_tag(&choice.method));
    out.extend_from_slice(&(choice.partition_block_size as u32).to_le_bytes());
    out.extend_from_slice(&universe_size.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn encode_v2(choice: &CodecChoice, universe_size: u32, n: u32, payload: &[u8]) -> Vec<u8> {
    let crc = crc32(payload);
    let mut out = Vec::with_capacity(8 + 1 + 4 + 4 + 4 + 8 + 4 + payload.len());
    out.extend_from_slice(MAGIC_V2);
    out.push(method_tag(&choice.method));
    out.extend_from_slice(&(choice.partition_block_size as u32).to_le_bytes());
    out.extend_from_slice(&universe_size.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

struct ParsedEnvelope<'a> {
    choice: CodecChoice,
    universe_size: u32,
    n: Option<u32>,
    payload: &'a [u8],
}

fn read_u32_le(bytes: &[u8], off: usize) -> Result<u32, CompressionError> {
    let slice = bytes
        .get(off..off + 4)
        .ok_or_else(|| CompressionError::DecompressionFailed("envelope header truncated".into()))?;
    Ok(u32::from_le_bytes(slice.try_into().map_err(|_| {
        CompressionError::DecompressionFailed("envelope header truncated".into())
    })?))
}

fn read_u64_le(bytes: &[u8], off: usize) -> Result<u64, CompressionError> {
    let slice = bytes
        .get(off..off + 8)
        .ok_or_else(|| CompressionError::DecompressionFailed("envelope header truncated".into()))?;
    Ok(u64::from_le_bytes(slice.try_into().map_err(|_| {
        CompressionError::DecompressionFailed("envelope header truncated".into())
    })?))
}

fn parse_envelope(bytes: &[u8]) -> Result<ParsedEnvelope<'_>, CompressionError> {
    // V2
    const V2_HDR: usize = 8 + 1 + 4 + 4 + 4 + 8 + 4;
    if bytes.len() >= V2_HDR && &bytes[..8] == MAGIC_V2 {
        let mut off = 8usize;
        let tag = bytes[off];
        off += 1;
        let pbs = read_u32_le(bytes, off)? as usize;
        off += 4;
        let universe_size = read_u32_le(bytes, off)?;
        off += 4;
        let n = read_u32_le(bytes, off)?;
        off += 4;
        let payload_len = read_u64_le(bytes, off)? as usize;
        off += 8;
        let expected_crc = read_u32_le(bytes, off)?;
        off += 4;
        if off + payload_len != bytes.len() {
            return Err(CompressionError::DecompressionFailed(
                "envelope payload length mismatch".to_string(),
            ));
        }
        let payload = &bytes[off..off + payload_len];
        let got_crc = crc32(payload);
        if got_crc != expected_crc {
            return Err(CompressionError::DecompressionFailed(
                "envelope CRC mismatch".to_string(),
            ));
        }
        let method = tag_method(tag)?;
        let choice = normalize_choice(CodecChoice {
            method,
            partition_block_size: pbs,
        });
        // Detect malformed headers early: pbs should be zero unless PartitionedEliasFano.
        if choice.method != IdCompressionMethod::PartitionedEliasFano && pbs != 0 {
            return Err(CompressionError::DecompressionFailed(
                "envelope has non-zero partition_block_size for non-partitioned method".to_string(),
            ));
        }
        return Ok(ParsedEnvelope {
            choice,
            universe_size,
            n: Some(n),
            payload,
        });
    }

    // V1
    const V1_HDR: usize = 8 + 1 + 4 + 4 + 8;
    if bytes.len() >= V1_HDR && &bytes[..8] == MAGIC_V1 {
        let mut off = 8usize;
        let tag = bytes[off];
        off += 1;
        let pbs = read_u32_le(bytes, off)? as usize;
        off += 4;
        let universe_size = read_u32_le(bytes, off)?;
        off += 4;
        let payload_len = read_u64_le(bytes, off)? as usize;
        off += 8;
        if off + payload_len != bytes.len() {
            return Err(CompressionError::DecompressionFailed(
                "envelope payload length mismatch".to_string(),
            ));
        }
        let payload = &bytes[off..off + payload_len];
        let method = tag_method(tag)?;
        let choice = normalize_choice(CodecChoice {
            method,
            partition_block_size: pbs,
        });
        if choice.method != IdCompressionMethod::PartitionedEliasFano && pbs != 0 {
            return Err(CompressionError::DecompressionFailed(
                "envelope has non-zero partition_block_size for non-partitioned method".to_string(),
            ));
        }
        return Ok(ParsedEnvelope {
            choice,
            universe_size,
            n: None,
            payload,
        });
    }

    Err(CompressionError::DecompressionFailed(
        "bad envelope magic".to_string(),
    ))
}

/// Encode as an envelope: choose method (auto) + store `(method, params, universe_size, payload)`.
///
/// # Errors
///
/// Returns `CompressionError` if input is invalid or compression fails.
pub fn compress_set_enveloped(
    ids: &[u32],
    universe_size: u32,
    cfg: AutoConfig,
) -> Result<Vec<u8>, CompressionError> {
    let (choice0, payload) = compress_set_auto(ids, universe_size, cfg)?;
    let choice = normalize_choice(choice0);
    if !ids.is_empty() && choice.method == IdCompressionMethod::None {
        return Err(CompressionError::CompressionFailed(
            "auto chooser returned None for non-empty set".to_string(),
        ));
    }
    let n: u32 = ids.len().try_into().map_err(|_| {
        CompressionError::CompressionFailed("id set too large for envelope header".to_string())
    })?;
    Ok(encode_v2(&choice, universe_size, n, &payload))
}

/// Decode an envelope.
///
/// Returns `(choice, universe_size, ids)`.
///
/// # Errors
///
/// Returns `CompressionError` if the envelope is malformed, CRC fails, or decompression fails.
pub fn decompress_set_enveloped(
    bytes: &[u8],
) -> Result<(CodecChoice, u32, Vec<u32>), CompressionError> {
    let parsed = parse_envelope(bytes)?;
    let ids =
        crate::decompress_set_auto(parsed.choice.clone(), parsed.payload, parsed.universe_size)?;
    if let Some(n) = parsed.n {
        if ids.len() != n as usize {
            return Err(CompressionError::DecompressionFailed(
                "envelope decoded length does not match declared n".to_string(),
            ));
        }
    }
    Ok((parsed.choice, parsed.universe_size, ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_roundtrip_roc() {
        let ids: Vec<u32> = (0..256).map(|i| i * 10).collect();
        let u = 100_000;
        let bytes = compress_set_enveloped(&ids, u, AutoConfig::default()).unwrap();
        let (_choice, u2, back) = decompress_set_enveloped(&bytes).unwrap();
        assert_eq!(u2, u);
        assert_eq!(back, ids);
    }

    #[test]
    fn envelope_v2_wire_golden_header() {
        let choice = CodecChoice {
            method: IdCompressionMethod::Roc,
            partition_block_size: 0,
        };
        let u = 123u32;
        let n = 7u32;
        let payload = [1u8, 2, 3];
        let got = encode_v2(&choice, u, n, &payload);

        // CRC32(payload=[1,2,3]) = 0x55bc801d, little-endian bytes: 1d 80 bc 55.
        let expected: Vec<u8> = vec![
            // MAGIC_V2
            b'C', b'N', b'K', b'E', b'N', b'V', b'0', b'2', // tag = Roc
            1,    // pbs = 0
            0, 0, 0, 0, // u = 123
            0x7b, 0, 0, 0, // n = 7
            7, 0, 0, 0, // len = 3 (u64 LE)
            3, 0, 0, 0, 0, 0, 0, 0, // crc = 0x55bc801d (u32 LE)
            0x1d, 0x80, 0xbc, 0x55, // payload
            1, 2, 3,
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn envelope_v2_crc_mismatch_is_detected() {
        let choice = CodecChoice {
            method: IdCompressionMethod::Roc,
            partition_block_size: 0,
        };
        let u = 123u32;
        let n = 7u32;
        let payload = [1u8, 2, 3];
        let mut bytes = encode_v2(&choice, u, n, &payload);
        // Corrupt a payload byte (last byte).
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        let err = decompress_set_enveloped(&bytes).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.to_ascii_lowercase().contains("crc"),
            "expected CRC error, got: {msg}"
        );
    }

    #[test]
    fn envelope_roundtrip_small_set() {
        let ids = vec![1u32, 5, 10, 20, 50, 100, 200, 500];
        let u = 1000;
        let bytes = compress_set_enveloped(&ids, u, AutoConfig::default()).unwrap();
        let (_choice, u2, back) = decompress_set_enveloped(&bytes).unwrap();
        assert_eq!(u2, u);
        assert_eq!(back, ids);
    }

    #[test]
    fn envelope_v1_is_still_decodable() {
        let ids: Vec<u32> = (0..256).map(|i| i * 10).collect();
        let u = 100_000;
        let (choice0, payload) = compress_set_auto(&ids, u, AutoConfig::default()).unwrap();
        let choice = normalize_choice(choice0);
        let bytes_v1 = encode_v1(&choice, u, &payload);
        let (_choice2, u2, back) = decompress_set_enveloped(&bytes_v1).unwrap();
        assert_eq!(u2, u);
        assert_eq!(back, ids);
    }
}
