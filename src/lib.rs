//! ID set compression primitives.
//!
//! `idpaq` provides compression algorithms for sorted, unique ID sets where
//! order doesn't matter. This is common in information retrieval:
//!
//! - IVF posting lists (which vectors belong to which cluster)
//! - HNSW neighbor lists (which nodes are connected)
//! - Inverted indexes (which documents contain which terms)
//!
//! # Compression Methods
//!
//! - **Delta encoding**: Simple baseline, varint-encodes gaps between IDs
//! - **ROC (Random Order Coding)**: Near-optimal for sets using bits-back with ANS
//!
//! # Historical Context
//!
//! Set compression has a rich history in information retrieval. Classic methods
//! like Elias-Fano (1971) exploit monotonicity of sorted sequences. Modern
//! methods like ROC (Severo et al., 2022) exploit the additional structure
//! that *order doesn't matter*, achieving log(C(N,n)) bits instead of
//! log(N^n) bits.
//!
//! # Example
//!
//! ```rust
//! use idpaq::{RocCompressor, IdSetCompressor};
//!
//! let compressor = RocCompressor::new();
//! let ids = vec![1u32, 5, 10, 20, 50];
//! let universe_size = 1000;
//!
//! // Compress
//! let compressed = compressor.compress_set(&ids, universe_size).unwrap();
//!
//! // Decompress
//! let decompressed = compressor.decompress_set(&compressed, universe_size).unwrap();
//! assert_eq!(ids, decompressed);
//! ```
//!
//! # References
//!
//! - Elias, P. (1974). "Efficient storage and retrieval by content and address"
//! - Fano, R. (1971). "On the number of bits required to implement an associative memory"
//! - Severo et al. (2022). "Compressing multisets with large alphabets"
//! - Severo et al. (2025). "Lossless Compression of Vector IDs for ANN Search"

#![warn(missing_docs)]
#![warn(clippy::all)]

mod error;
mod roc;
mod traits;

#[cfg(feature = "ans")]
mod ans;

pub use error::CompressionError;
pub use roc::RocCompressor;
pub use traits::IdSetCompressor;

/// Compression method selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum IdCompressionMethod {
    /// No compression (uncompressed storage).
    #[default]
    None,
    /// Elias-Fano encoding (baseline, sorted sequences).
    EliasFano,
    /// Random Order Coding (optimal for sets, uses bits-back with ANS).
    Roc,
    /// Wavelet tree (full random access, future).
    WaveletTree,
}
