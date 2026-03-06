//! Compression trait definitions.

use crate::error::CompressionError;

/// Trait for compressing sets of IDs where order doesn't matter.
///
/// This trait is designed for compressing collections of vector IDs in ANN indexes
/// where the ordering of IDs is irrelevant (e.g., IVF clusters, HNSW neighbor lists).
///
/// # Requirements
///
/// - Input IDs must be sorted and unique
/// - Compression should exploit ordering invariance
/// - Decompression should return sorted IDs
///
/// # Theoretical Background
///
/// A set of `n` elements from universe `[N]` has `C(N, n)` possible sets.
/// The information-theoretic lower bound is `log2(C(N, n))` bits.
/// This is significantly less than encoding a sequence (`N^n` possibilities).
///
/// Implementations should aim to approach this bound.
pub trait IdSetCompressor {
    /// Compress a set of IDs (order-invariant).
    ///
    /// # Arguments
    ///
    /// * `ids` - Sorted, unique IDs (must be sorted for correctness)
    /// * `universe_size` - Maximum possible ID value (for entropy calculation)
    ///
    /// # Returns
    ///
    /// Compressed representation as byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError::InvalidInput`] if `ids` is not sorted in
    /// strictly ascending order (i.e. not unique) or contains values
    /// `>= universe_size`.
    ///
    /// Returns [`CompressionError::CompressionFailed`] if the codec encounters
    /// an internal encoding failure.
    fn compress_set(&self, ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError>;

    /// Decompress a set of IDs.
    ///
    /// # Arguments
    ///
    /// * `compressed` - Compressed byte vector
    /// * `universe_size` - Maximum possible ID value (must match compression)
    ///
    /// # Returns
    ///
    /// Sorted vector of IDs.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError::DecompressionFailed`] if the compressed
    /// data is malformed, truncated, contains trailing bytes, or produces
    /// IDs `>= universe_size`.
    fn decompress_set(
        &self,
        compressed: &[u8],
        universe_size: u32,
    ) -> Result<Vec<u32>, CompressionError>;

    /// Estimate compressed size without full compression.
    ///
    /// Useful for deciding whether to compress.
    ///
    /// # Arguments
    ///
    /// * `num_ids` - Number of IDs in the set
    /// * `universe_size` - Maximum possible ID value
    ///
    /// # Returns
    ///
    /// Estimated compressed size in bytes.
    fn estimate_size(&self, num_ids: usize, universe_size: u32) -> usize;

    /// Get compression ratio (bits per ID).
    ///
    /// # Arguments
    ///
    /// * `num_ids` - Number of IDs in the set
    /// * `universe_size` - Maximum possible ID value
    ///
    /// # Returns
    ///
    /// Average bits per ID (theoretical lower bound).
    fn bits_per_id(&self, num_ids: usize, universe_size: u32) -> f64;
}

/// Validate that `ids` is sorted in strictly ascending order (sorted + unique).
///
/// Uniqueness is required because set compressors encode *sets*, not multisets:
/// the compressed representation assumes each ID appears exactly once, and
/// duplicate or out-of-order elements would silently corrupt the delta encoding
/// (producing zero or negative gaps that cannot round-trip).
///
/// Returns `Ok(())` for empty slices.
pub fn validate_ids(ids: &[u32]) -> Result<(), CompressionError> {
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
