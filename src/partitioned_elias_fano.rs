//! Partitioned Elias–Fano compressor (cluster-aware succinct baseline) via `sbits`.
//!
//! This module is feature-gated behind `cnk/sbits`.

use crate::error::CompressionError;
use crate::traits::IdSetCompressor;

/// Partitioned Elias–Fano compressor for sorted, unique ID sets.
#[derive(Clone, Debug)]
pub struct PartitionedEliasFanoCompressor {
    block_size: usize,
}

impl PartitionedEliasFanoCompressor {
    /// Create a new compressor with a default block size.
    ///
    /// Typical engineering values are 64–256; 128 is a reasonable default.
    #[must_use]
    pub fn new() -> Self {
        Self { block_size: 128 }
    }

    /// Create a compressor with a custom block size (must be >= 1).
    #[must_use]
    pub fn with_block_size(block_size: usize) -> Self {
        Self {
            block_size: block_size.max(1),
        }
    }
}

impl Default for PartitionedEliasFanoCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl IdSetCompressor for PartitionedEliasFanoCompressor {
    fn compress_set(&self, ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError> {
        crate::traits::validate_ids(ids)?;

        if let Some(&max_id) = ids.iter().max() {
            if max_id >= universe_size {
                return Err(CompressionError::InvalidInput(format!(
                    "ID {} exceeds universe size {}",
                    max_id, universe_size
                )));
            }
        }

        let pef = sbits::PartitionedEliasFano::new(ids, universe_size, self.block_size);
        Ok(pef.to_bytes())
    }

    fn decompress_set(
        &self,
        compressed: &[u8],
        universe_size: u32,
    ) -> Result<Vec<u32>, CompressionError> {
        if compressed.is_empty() {
            return Ok(Vec::new());
        }

        let pef = sbits::PartitionedEliasFano::from_bytes(compressed).map_err(|e| {
            CompressionError::DecompressionFailed(format!(
                "PartitionedEliasFano decode failed: {e}"
            ))
        })?;

        if pef.universe_size() != universe_size {
            return Err(CompressionError::DecompressionFailed(format!(
                "Universe mismatch: encoded {} vs requested {}",
                pef.universe_size(),
                universe_size
            )));
        }

        let mut out = Vec::with_capacity(pef.len());
        for i in 0..pef.len() {
            let v = pef
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
        // Conservative: fallback to the plain EF estimate plus small per-block overhead.
        // (Precise estimation depends on cluster structure; callers should benchmark if it matters.)
        let ef = crate::EliasFanoCompressor::new();
        let base = ef.estimate_size(num_ids, universe_size);
        let blocks = num_ids.div_ceil(self.block_size);
        base + blocks.saturating_mul(16)
    }

    fn bits_per_id(&self, num_ids: usize, universe_size: u32) -> f64 {
        if num_ids == 0 {
            return 0.0;
        }
        (self.estimate_size(num_ids, universe_size) as f64 * 8.0) / (num_ids as f64)
    }
}
