//! Delta+varint compressor for sorted ID sets.
//!
//! Encodes gaps between sorted IDs as varints.

use crate::error::CompressionError;
use crate::traits::IdSetCompressor;

/// Estimate varint bytes needed to encode a value.
fn varint_bytes(value: u64) -> usize {
    if value == 0 {
        return 1;
    }
    let bits = 64 - value.leading_zeros() as usize;
    bits.div_ceil(7)
}

/// Estimate compressed size for delta+varint encoding.
fn estimated_varint_size(num_ids: usize, universe_size: u32) -> usize {
    if num_ids == 0 {
        return 0;
    }
    let n = num_ids as u64;
    let u = universe_size as u64;
    if n > u {
        return 0;
    }

    // Mean gap for uniformly distributed IDs
    let mean_gap = if n <= 1 { u } else { u / n };

    // count varint + first_id varint + (n-1) gap varints
    let count_bytes = varint_bytes(n);
    let first_id_bytes = varint_bytes(mean_gap); // first ID ~ mean_gap for uniform
    let gap_bytes = if n > 1 {
        (n as usize - 1) * varint_bytes(mean_gap)
    } else {
        0
    };

    count_bytes + first_id_bytes + gap_bytes
}

/// Delta+varint compressor for sorted, unique ID sets.
///
/// Encodes gaps between consecutive IDs using variable-length integers.
pub struct DeltaVarintCompressor {}

impl DeltaVarintCompressor {
    /// Create a new delta+varint compressor.
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// Encode a u64 as varint into the buffer.
    #[inline]
    fn encode_varint(value: u64, buf: &mut Vec<u8>) {
        let mut val = value;
        while val >= 0x80 {
            buf.push((val as u8) | 0x80);
            val >>= 7;
        }
        buf.push(val as u8);
    }

    /// Decode a varint from the buffer, returning (value, bytes_consumed).
    #[inline]
    fn decode_varint(buf: &[u8]) -> Result<(u64, usize), CompressionError> {
        let mut value = 0u64;
        let mut shift = 0;
        let mut offset = 0;

        loop {
            if offset >= buf.len() {
                return Err(CompressionError::DecompressionFailed(
                    "Unexpected end of compressed data".to_string(),
                ));
            }

            if shift > 56 {
                return Err(CompressionError::DecompressionFailed(
                    "Varint encoding too large".to_string(),
                ));
            }

            let byte = buf[offset];
            offset += 1;
            value |= ((byte & 0x7F) as u64) << shift;

            if (byte & 0x80) == 0 {
                break;
            }
            shift += 7;
        }

        Ok((value, offset))
    }
}

impl IdSetCompressor for DeltaVarintCompressor {
    fn compress_set(&self, ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError> {
        crate::traits::validate_ids(ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Check bounds
        if let Some(&max_id) = ids.iter().max() {
            if max_id >= universe_size {
                return Err(CompressionError::InvalidInput(format!(
                    "ID {} exceeds universe size {}",
                    max_id, universe_size
                )));
            }
        }

        let mut encoded = Vec::new();

        // Store number of IDs
        Self::encode_varint(ids.len() as u64, &mut encoded);

        // Delta encode IDs
        if let Some(&first) = ids.first() {
            Self::encode_varint(first as u64, &mut encoded);

            for i in 1..ids.len() {
                let delta = ids[i] - ids[i - 1];
                Self::encode_varint(delta as u64, &mut encoded);
            }
        }

        Ok(encoded)
    }

    fn decompress_set(
        &self,
        compressed: &[u8],
        universe_size: u32,
    ) -> Result<Vec<u32>, CompressionError> {
        if compressed.is_empty() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let mut offset = 0;

        // Decode number of IDs
        let (num_ids, consumed) = Self::decode_varint(&compressed[offset..])?;
        offset += consumed;

        if num_ids > universe_size as u64 {
            return Err(CompressionError::DecompressionFailed(format!(
                "declared count {} exceeds universe size {}",
                num_ids, universe_size
            )));
        }

        if num_ids == 0 {
            return Ok(ids);
        }

        // Decode first ID
        let (first_id, consumed) = Self::decode_varint(&compressed[offset..])?;
        offset += consumed;

        if first_id >= universe_size as u64 {
            return Err(CompressionError::DecompressionFailed(format!(
                "ID {} exceeds universe size {}",
                first_id, universe_size
            )));
        }
        ids.push(first_id as u32);

        // Decode deltas
        for _ in 1..num_ids {
            let (delta, consumed) = Self::decode_varint(&compressed[offset..])?;
            offset += consumed;

            let next_id = ids.last().unwrap() + delta as u32;
            if next_id >= universe_size {
                return Err(CompressionError::DecompressionFailed(format!(
                    "ID {} exceeds universe size {}",
                    next_id, universe_size
                )));
            }
            ids.push(next_id);
        }

        // Verify we consumed all data
        if offset < compressed.len() {
            return Err(CompressionError::DecompressionFailed(format!(
                "Extra data after decompression: {} bytes",
                compressed.len() - offset
            )));
        }

        Ok(ids)
    }

    fn estimate_size(&self, num_ids: usize, universe_size: u32) -> usize {
        estimated_varint_size(num_ids, universe_size)
    }

    fn bits_per_id(&self, num_ids: usize, universe_size: u32) -> f64 {
        if num_ids == 0 {
            return 0.0;
        }
        (estimated_varint_size(num_ids, universe_size) as f64 * 8.0) / (num_ids as f64)
    }
}

impl Default for DeltaVarintCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![1u32, 5, 10, 20, 50, 100];
        let universe_size = 1000;

        let compressed = compressor.compress_set(&ids, universe_size).unwrap();
        let decompressed = compressor
            .decompress_set(&compressed, universe_size)
            .unwrap();

        assert_eq!(ids, decompressed);
    }

    #[test]
    fn test_empty_set() {
        let compressor = DeltaVarintCompressor::new();
        let compressed = compressor.compress_set(&[], 1000).unwrap();
        assert!(compressed.is_empty());

        let decompressed = compressor.decompress_set(&[], 1000).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_unsorted_ids() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![5u32, 1, 10]; // Not sorted

        let result = compressor.compress_set(&ids, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_ids() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![1u32, 5, 5, 10]; // Duplicate

        let result = compressor.compress_set(&ids, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_consecutive_ids() {
        let compressor = DeltaVarintCompressor::new();
        let ids: Vec<u32> = (0..100).collect();
        let universe_size = 1000;

        let compressed = compressor.compress_set(&ids, universe_size).unwrap();
        let decompressed = compressor
            .decompress_set(&compressed, universe_size)
            .unwrap();

        assert_eq!(ids, decompressed);

        // Consecutive IDs should compress well (deltas are all 1)
        let uncompressed_size = ids.len() * 4;
        let ratio = uncompressed_size as f64 / compressed.len() as f64;
        assert!(
            ratio > 2.0,
            "Consecutive IDs should compress well: {}",
            ratio
        );
    }

    #[test]
    fn test_single_id() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![42u32];

        let compressed = compressor.compress_set(&ids, 1000).unwrap();
        let decompressed = compressor.decompress_set(&compressed, 1000).unwrap();

        assert_eq!(ids, decompressed);
    }

    #[test]
    fn test_id_exceeds_universe() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![1000u32]; // Exceeds universe_size = 1000

        let result = compressor.compress_set(&ids, 1000);
        assert!(result.is_err());
    }

    #[test]
    fn sparse_single_id() {
        let compressor = DeltaVarintCompressor::new();
        let ids = vec![999_999u32];
        let universe_size = 1_000_000;
        let compressed = compressor.compress_set(&ids, universe_size).unwrap();
        let decompressed = compressor
            .decompress_set(&compressed, universe_size)
            .unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn dense_set() {
        let compressor = DeltaVarintCompressor::new();
        let ids: Vec<u32> = (0..999).collect();
        let universe_size = 1000;
        let compressed = compressor.compress_set(&ids, universe_size).unwrap();
        let decompressed = compressor
            .decompress_set(&compressed, universe_size)
            .unwrap();
        assert_eq!(ids, decompressed);
    }
}
