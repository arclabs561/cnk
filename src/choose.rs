//! Deterministic heuristics for choosing an ID compression method.
//!
//! This is intentionally conservative: it is meant to be “good enough” as a default and stable
//! across versions. If you need the true optimum, benchmark on your distributions.

use crate::stats::IdListStats;
use crate::IdCompressionMethod;
#[cfg(feature = "sbits")]
use crate::{DeltaVarintCompressor, IdSetCompressor};

/// Configuration for the heuristic chooser.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChooseConfig {
    /// Minimum `n` to consider partitioned Elias–Fano.
    pub min_n_partitioned: usize,
    /// If fraction of “small gaps” is >= this, consider the list locally clustered.
    pub clustered_frac_small_gaps: f64,
    /// Default block size to recommend for partitioned Elias–Fano.
    pub partition_block_size: usize,
}

impl Default for ChooseConfig {
    fn default() -> Self {
        Self {
            min_n_partitioned: 64,
            clustered_frac_small_gaps: 0.75,
            partition_block_size: 128,
        }
    }
}

/// A codec choice, including any method parameters that should be recorded alongside the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecChoice {
    /// Chosen method.
    pub method: IdCompressionMethod,
    /// Suggested block size if `method == PartitionedEliasFano`; otherwise 0.
    pub partition_block_size: usize,
}

/// Choose a compression method from list statistics.
///
/// Notes:
/// - The choice is **deterministic**.
/// - The returned method is a *recommendation*; callers must ensure the chosen codec is
///   available in the current build (e.g. `EliasFano` requires feature `sbits`).
#[must_use]
pub fn choose_method(stats: &IdListStats, cfg: ChooseConfig) -> CodecChoice {
    if stats.n == 0 {
        return CodecChoice {
            method: IdCompressionMethod::None,
            partition_block_size: 0,
        };
    }

    {
        // If succinct structures are not available in this build, DeltaVarint is the only meaningful
        // non-empty choice (auto callers may still apply their own policy above this).
        #[cfg(not(feature = "sbits"))]
        {
            let _ = cfg;
            #[allow(clippy::needless_return)]
            return CodecChoice {
                method: IdCompressionMethod::DeltaVarint,
                partition_block_size: 0,
            };
        }

        #[cfg(feature = "sbits")]
        {
            // Always consider DeltaVarint.
            let mut best = CodecChoice {
                method: IdCompressionMethod::DeltaVarint,
                partition_block_size: 0,
            };
            let mut best_bytes = estimate_choice_bytes(&best, stats);

            // Baseline: plain Elias–Fano.
            let ef = CodecChoice {
                method: IdCompressionMethod::EliasFano,
                partition_block_size: 0,
            };
            let ef_bytes = estimate_choice_bytes(&ef, stats);
            update_best(&mut best, &mut best_bytes, ef, ef_bytes);

            // Candidate: partitioned Elias–Fano (only when list looks locally clustered).
            if stats.n >= cfg.min_n_partitioned
                && stats.frac_small_gaps >= cfg.clustered_frac_small_gaps
            {
                let bs0 = cfg.partition_block_size.max(1);
                let mut best_pef: Option<(usize, usize)> = None; // (block_size, est_bytes)
                for bs in candidate_block_sizes(bs0, stats.n) {
                    let pef_bytes = estimate_partitioned_elias_fano_bytes(stats, bs);
                    match best_pef {
                        None => best_pef = Some((bs, pef_bytes)),
                        Some((best_bs, best_b)) => {
                            if pef_bytes < best_b || (pef_bytes == best_b && bs < best_bs) {
                                best_pef = Some((bs, pef_bytes));
                            }
                        }
                    }
                }
                if let Some((bs, pef_bytes)) = best_pef {
                    let pef = CodecChoice {
                        method: IdCompressionMethod::PartitionedEliasFano,
                        partition_block_size: bs,
                    };
                    update_best(&mut best, &mut best_bytes, pef, pef_bytes);
                }
            }

            best
        }
    }
}

#[cfg(feature = "sbits")]
fn update_best(
    best: &mut CodecChoice,
    best_bytes: &mut usize,
    cand: CodecChoice,
    cand_bytes: usize,
) {
    if cand_bytes < *best_bytes
        || (cand_bytes == *best_bytes && choice_rank(&cand) < choice_rank(best))
    {
        *best = cand;
        *best_bytes = cand_bytes;
    }
}

#[cfg(feature = "sbits")]
fn choice_rank(c: &CodecChoice) -> (u8, usize) {
    // Deterministic tie-breaker: prefer “simpler” methods when estimated sizes tie.
    let method_rank = match c.method {
        IdCompressionMethod::None => 0,
        IdCompressionMethod::DeltaVarint => 1,
        IdCompressionMethod::EliasFano => 2,
        IdCompressionMethod::PartitionedEliasFano => 3,
        #[cfg(feature = "ans")]
        IdCompressionMethod::Roc => 4,
    };
    (method_rank, c.partition_block_size)
}

#[cfg(feature = "sbits")]
fn estimate_choice_bytes(choice: &CodecChoice, stats: &IdListStats) -> usize {
    match choice.method {
        IdCompressionMethod::None => 0,
        IdCompressionMethod::DeltaVarint => {
            DeltaVarintCompressor::new().estimate_size(stats.n, stats.universe_size)
        }
        #[cfg(feature = "sbits")]
        IdCompressionMethod::EliasFano => {
            crate::EliasFanoCompressor::new().estimate_size(stats.n, stats.universe_size)
        }
        #[cfg(feature = "sbits")]
        IdCompressionMethod::PartitionedEliasFano => {
            estimate_partitioned_elias_fano_bytes(stats, choice.partition_block_size.max(1))
        }
        #[cfg(feature = "ans")]
        IdCompressionMethod::Roc => {
            crate::RocCompressor::new().estimate_size(stats.n, stats.universe_size)
        }
    }
}

#[cfg(feature = "sbits")]
fn ef_total_bits(n: u64, u: u64) -> u128 {
    // Approximate Elias–Fano bits:
    // L = floor(log2(U/n)); total ≈ n*L + (n + U/2^L) bits
    if n == 0 || u == 0 || n > u {
        return 0;
    }
    let ratio = (u / n).max(1);
    let l: u64 = 63 - ratio.leading_zeros() as u64;
    let lower_bits: u128 = (n as u128).saturating_mul(l as u128);
    let upper_bits: u128 = (n as u128)
        .saturating_add((u >> l) as u128)
        .saturating_add(1);
    lower_bits.saturating_add(upper_bits)
}

#[cfg(feature = "sbits")]
fn bits_to_bytes(bits: u128) -> usize {
    let bytes = (bits.saturating_add(7)) / 8;
    bytes.min(usize::MAX as u128) as usize
}

#[cfg(feature = "sbits")]
fn candidate_block_sizes(default_bs: usize, n: usize) -> Vec<usize> {
    // Deterministic small set; sorted + deduped.
    let mut v = vec![
        32usize,
        64,
        128,
        256,
        512,
        1024,
        default_bs,
        default_bs / 2,
        default_bs.saturating_mul(2),
        n,
    ];
    v.retain(|&bs| bs >= 1 && bs <= n);
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(feature = "sbits")]
fn estimate_partitioned_elias_fano_bytes(stats: &IdListStats, block_size: usize) -> usize {
    if stats.n == 0 || stats.universe_size == 0 {
        return 0;
    }
    let bs = block_size.max(1);
    let n = stats.n;
    let blocks = n.div_ceil(bs);

    // Local-cluster model:
    // Treat “small gaps” (<=3) as intra-cluster, and the rest as inter-cluster separators.
    let f = stats.frac_small_gaps.clamp(0.0, 1.0);
    let mean_gap = stats.mean_gap.max(0.0);
    let gaps = (n.saturating_sub(1)) as f64;

    // Empirical prior: when gaps are mostly “small”, their mean is close to 1; when less so,
    // it approaches the threshold 3. Keep the range tight to avoid overconfidence.
    let mut small_gap_mean = 1.0 + (1.0 - f) * 2.0; // in [1,3]
    if mean_gap > 0.0 {
        small_gap_mean = small_gap_mean.min(mean_gap.max(0.1));
    }

    let large_gap_mean = if f >= 1.0 - 1e-12 {
        small_gap_mean
    } else {
        let est = (mean_gap - f * small_gap_mean) / (1.0 - f);
        est.max(small_gap_mean)
    };

    let est_large_gaps = ((1.0 - f) * gaps).round();
    let clusters = (est_large_gaps as usize).saturating_add(1).clamp(1, n);
    let cluster_size = n.div_ceil(clusters).max(1);

    let global_span: u64 = (stats.max_id as u64)
        .saturating_sub(stats.min_id as u64)
        .saturating_add(1)
        .max(n as u64)
        .max(1);

    let mut total_payload = 0usize;
    let mut remaining = n;
    for _ in 0..blocks {
        let block_n = remaining.min(bs);
        remaining -= block_n;
        if block_n == 0 {
            break;
        }
        if block_n == 1 {
            // A single value: store min/max + tiny overhead; model it as 1 byte payload.
            total_payload = total_payload.saturating_add(1);
            continue;
        }

        let clusters_in_block = block_n.div_ceil(cluster_size).max(1);
        let large_gaps_in_block = clusters_in_block.saturating_sub(1);
        let small_gaps_in_block = block_n.saturating_sub(1 + large_gaps_in_block);

        let span_est = (small_gaps_in_block as f64) * small_gap_mean
            + (large_gaps_in_block as f64) * large_gap_mean;

        // Local universe is approximately the block span (+1). Clamp to global span.
        let mut u_block = (span_est.ceil() as u64).saturating_add(1);
        u_block = u_block.clamp(block_n as u64, global_span);

        total_payload =
            total_payload.saturating_add(bits_to_bytes(ef_total_bits(block_n as u64, u_block)));
    }

    // Add small overhead for block descriptors / minima. Keep it conservative.
    total_payload
        .saturating_add(blocks.saturating_mul(16))
        .saturating_add(64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooser_is_deterministic() {
        let ids: Vec<u32> = (0..200).map(|i| i * 10).collect();
        let stats = IdListStats::from_sorted_unique(&ids, 10000);
        let a = choose_method(&stats, ChooseConfig::default());
        let b = choose_method(&stats, ChooseConfig::default());
        assert_eq!(a, b);
    }

    #[cfg(feature = "sbits")]
    #[test]
    fn chooser_prefers_partitioned_for_strongly_clustered_lists() {
        // 8 clusters of 256 consecutive IDs, separated by huge gaps.
        let mut ids = Vec::new();
        for k in 0..8u32 {
            let base = k * 1_000_000;
            ids.extend((0..256u32).map(|i| base + i));
        }
        let u = 8_000_000u32;
        let stats = IdListStats::from_sorted_unique(&ids, u);
        let choice = choose_method(&stats, ChooseConfig::default());
        assert_eq!(choice.method, IdCompressionMethod::PartitionedEliasFano);
        assert!(choice.partition_block_size > 0);
    }
}
