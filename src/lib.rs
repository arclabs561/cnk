//! ID set compression primitives.
//!
//! `cnk` provides compression algorithms for sorted, unique ID sets where
//! order doesn't matter. This is common in information retrieval:
//!
//! - IVF posting lists (which vectors belong to which cluster)
//! - HNSW neighbor lists (which nodes are connected)
//! - Inverted indexes (which documents contain which terms)
//!
//! # Compression Methods
//!
//! - **Delta+varint** (`RocCompressor`): practical baseline, varint-encodes gaps between sorted IDs
//! - **Elias-Fano** (feature `sbits`): succinct monotone-sequence codec with random access
//! - **Partitioned Elias-Fano** (feature `sbits`): cluster-aware variant
//! - **ROC (Random Order Coding)**: near-optimal for sets using bits-back with ANS (planned)
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
//! use cnk::{RocCompressor, IdSetCompressor};
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

mod auto;
mod choose;
#[cfg(feature = "sbits")]
mod elias_fano;
mod envelope;
mod error;
#[cfg(feature = "sbits")]
mod partitioned_elias_fano;
mod roc;
mod stats;
mod traits;

#[cfg(feature = "ans")]
pub mod ans;

pub use auto::{compress_set_auto, decompress_set_auto, AutoConfig};
pub use choose::{choose_method, ChooseConfig, CodecChoice};
#[cfg(feature = "sbits")]
pub use elias_fano::EliasFanoCompressor;
pub use envelope::{compress_set_enveloped, decompress_set_enveloped};
pub use error::CompressionError;
#[cfg(feature = "sbits")]
pub use partitioned_elias_fano::PartitionedEliasFanoCompressor;
pub use roc::RocCompressor;
pub use stats::IdListStats;
pub use traits::{validate_ids, IdSetCompressor};

/// Compression method selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum IdCompressionMethod {
    /// No compression (uncompressed storage).
    #[default]
    None,
    /// Elias-Fano encoding (baseline, sorted sequences).
    EliasFano,
    /// Partitioned Elias–Fano (cluster-aware monotone sequences).
    PartitionedEliasFano,
    /// Random Order Coding (optimal for sets, uses bits-back with ANS).
    Roc,
    /// Wavelet tree (full random access, future).
    WaveletTree,
}
