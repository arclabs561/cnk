//! Elias–Fano compressor (succinct baseline) via `sbits`.
//!
//! This module is feature-gated behind `cnk/sbits`.

use crate::error::CompressionError;
use crate::traits::IdSetCompressor;

/// Elias–Fano compressor for sorted, unique ID sets.
#[derive(Clone, Debug, Default)]
pub struct EliasFanoCompressor;

impl EliasFanoCompressor {
    /// Create a new Elias–Fano compressor.
    pub fn new() -> Self {
        Self
    }

    fn validate_ids(ids: &[u32]) -> Result<(), CompressionError> {
        if ids.is_empty() {
            return Ok(());
        }
        for i in 1..ids.len() {
            if ids[i] <= ids[i - 1] {
                return Err(CompressionError::InvalidInput(format!(
                    "IDs must be sorted and unique, found {} <= {}",
                    ids[i],
                    ids[i - 1]
                )));
            }
        }
        Ok(())
    }
}

impl IdSetCompressor for EliasFanoCompressor {
    fn compress_set(&self, ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError> {
        Self::validate_ids(ids)?;

        if let Some(&max_id) = ids.iter().max() {
            if max_id >= universe_size {
                return Err(CompressionError::InvalidInput(format!(
                    "ID {} exceeds universe size {}",
                    max_id, universe_size
                )));
            }
        }

        let ef = sbits::EliasFano::new(ids, universe_size);
        Ok(ef.to_bytes())
    }

    fn decompress_set(
        &self,
        compressed: &[u8],
        universe_size: u32,
    ) -> Result<Vec<u32>, CompressionError> {
        if compressed.is_empty() {
            return Ok(Vec::new());
        }

        let ef = sbits::EliasFano::from_bytes(compressed).map_err(|e| {
            CompressionError::DecompressionFailed(format!("EliasFano decode failed: {e}"))
        })?;

        if ef.universe_size() != universe_size {
            return Err(CompressionError::DecompressionFailed(format!(
                "Universe mismatch: encoded {} vs requested {}",
                ef.universe_size(),
                universe_size
            )));
        }

        let mut out = Vec::with_capacity(ef.len());
        for i in 0..ef.len() {
            let v = ef
                .get(i)
                .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;
            out.push(v);
        }
        Ok(out)
    }

    fn estimate_size(&self, num_ids: usize, universe_size: u32) -> usize {
        if num_ids == 0 || universe_size == 0 {
            return 0;
        }

        // Approximate Elias–Fano bits:
        // L = floor(log2(U/n)); total ≈ n*L + (n + U/2^L) bits (+ small overhead)
        let n = num_ids as u64;
        let u = universe_size as u64;
        if n == 0 || u == 0 || n > u {
            return 0;
        }

        let ratio = (u / n).max(1);
        let l = 63 - ratio.leading_zeros() as u64;
        let lower_bits = n * l;
        let upper_bits = n + (u >> l) + 1;
        let total_bits = lower_bits + upper_bits;

        // Add a small constant overhead for headers/indices.
        (total_bits as usize).div_ceil(8) + 32
    }

    fn bits_per_id(&self, num_ids: usize, universe_size: u32) -> f64 {
        if num_ids == 0 {
            return 0.0;
        }
        (self.estimate_size(num_ids, universe_size) as f64 * 8.0) / (num_ids as f64)
    }
}
