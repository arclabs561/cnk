//! Convenience “auto” API: choose + compress/decompress with feature-aware fallbacks.

use crate::choose::{choose_method, ChooseConfig, CodecChoice};
use crate::stats::IdListStats;
use crate::{CompressionError, IdCompressionMethod, IdSetCompressor, RocCompressor};

/// Configuration for `compress_set_auto`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoConfig {
    /// Heuristic chooser configuration.
    pub choose: ChooseConfig,
}

impl Default for AutoConfig {
    fn default() -> Self {
        Self {
            choose: ChooseConfig::default(),
        }
    }
}

/// Choose a method (feature-aware) and compress.
///
/// Returns the chosen method so callers can record it alongside the bytes.
pub fn compress_set_auto(
    ids: &[u32],
    universe_size: u32,
    cfg: AutoConfig,
) -> Result<(CodecChoice, Vec<u8>), CompressionError> {
    // Stats are cheap; compressors validate sorted+unique bounds.
    let stats = IdListStats::from_sorted_unique(ids, universe_size);
    let choice0 = choose_method(&stats, cfg.choose);

    // Feature-aware fallback: if `sbits` is not enabled, we can only do Roc/None.
    #[cfg(not(feature = "sbits"))]
    let choice = {
        if matches!(
            choice0.method,
            IdCompressionMethod::EliasFano | IdCompressionMethod::PartitionedEliasFano
        ) {
            CodecChoice {
                method: IdCompressionMethod::Roc,
                partition_block_size: 0,
            }
        } else {
            choice0
        }
    };
    #[cfg(feature = "sbits")]
    let choice = choice0;

    let bytes = match choice.method {
        IdCompressionMethod::None => Vec::new(),
        IdCompressionMethod::Roc => RocCompressor::new().compress_set(ids, universe_size)?,
        #[cfg(feature = "sbits")]
        IdCompressionMethod::EliasFano => {
            crate::EliasFanoCompressor::new().compress_set(ids, universe_size)?
        }
        #[cfg(feature = "sbits")]
        IdCompressionMethod::PartitionedEliasFano => {
            crate::PartitionedEliasFanoCompressor::with_block_size(
                choice.partition_block_size.max(1),
            )
            .compress_set(ids, universe_size)?
        }
        _ => {
            return Err(CompressionError::CompressionFailed(
                "unsupported compression method in this build".to_string(),
            ))
        }
    };

    Ok((choice, bytes))
}

/// Decompress bytes previously produced by `compress_set_auto` (or any compatible encoder),
/// using the recorded `choice.method`.
pub fn decompress_set_auto(
    choice: CodecChoice,
    compressed: &[u8],
    universe_size: u32,
) -> Result<Vec<u32>, CompressionError> {
    match choice.method {
        IdCompressionMethod::None => Ok(Vec::new()),
        IdCompressionMethod::Roc => RocCompressor::new().decompress_set(compressed, universe_size),
        #[cfg(feature = "sbits")]
        IdCompressionMethod::EliasFano => {
            crate::EliasFanoCompressor::new().decompress_set(compressed, universe_size)
        }
        #[cfg(feature = "sbits")]
        IdCompressionMethod::PartitionedEliasFano => {
            crate::PartitionedEliasFanoCompressor::new().decompress_set(compressed, universe_size)
        }
        _ => Err(CompressionError::DecompressionFailed(
            "unsupported compression method in this build".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_roundtrip_smoke() {
        let ids: Vec<u32> = (0..256).map(|i| i * 10).collect();
        let u = 10_000;
        let (choice, bytes) = compress_set_auto(&ids, u, AutoConfig::default()).unwrap();
        let back = decompress_set_auto(choice, &bytes, u).unwrap();
        assert_eq!(ids, back);
    }
}
