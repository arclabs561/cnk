//! Lightweight statistics for choosing ID codecs.
//!
//! This module is intentionally “cheap”: it computes a few summary numbers that are stable and
//! deterministic, and that correlate with codec performance for typical posting/neighbors lists.

/// Summary statistics for a sorted, unique ID list within `[0, universe_size)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdListStats {
    /// Number of IDs.
    pub n: usize,
    /// Universe size \(U\): all IDs must be `< U`.
    pub universe_size: u32,
    /// Minimum ID (if `n>0`).
    pub min_id: u32,
    /// Maximum ID (if `n>0`).
    pub max_id: u32,
    /// Mean gap between consecutive IDs (0 if `n<=1`).
    pub mean_gap: f64,
    /// Maximum gap between consecutive IDs (0 if `n<=1`).
    pub max_gap: u32,
    /// Fraction of gaps that are “small” (0 if `n<=1`).
    pub frac_small_gaps: f64,
}

impl IdListStats {
    /// Compute statistics from a sorted, unique ID list.
    ///
    /// This function does **not** validate sorting/uniqueness; callers that rely on it should
    /// validate upstream (all `cnk` compressors already do).
    #[must_use]
    pub fn from_sorted_unique(ids: &[u32], universe_size: u32) -> Self {
        let n = ids.len();
        if n == 0 {
            return Self {
                n: 0,
                universe_size,
                min_id: 0,
                max_id: 0,
                mean_gap: 0.0,
                max_gap: 0,
                frac_small_gaps: 0.0,
            };
        }
        let min_id = ids[0];
        let max_id = ids[n - 1];

        if n == 1 {
            return Self {
                n,
                universe_size,
                min_id,
                max_id,
                mean_gap: 0.0,
                max_gap: 0,
                frac_small_gaps: 0.0,
            };
        }

        // A “small gap” threshold that correlates with delta+varint being very effective.
        // 0..=3 fits in one varint byte for deltas (and often dominates in neighbor lists).
        const SMALL_GAP: u32 = 3;

        let mut sum_gaps: u64 = 0;
        let mut max_gap: u32 = 0;
        let mut small = 0usize;
        for w in ids.windows(2) {
            let a = w[0];
            let b = w[1];
            let gap = b.saturating_sub(a);
            sum_gaps += gap as u64;
            max_gap = max_gap.max(gap);
            if gap <= SMALL_GAP {
                small += 1;
            }
        }
        let gaps = (n - 1) as f64;
        Self {
            n,
            universe_size,
            min_id,
            max_id,
            mean_gap: (sum_gaps as f64) / gaps,
            max_gap,
            frac_small_gaps: (small as f64) / gaps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_empty() {
        let s = IdListStats::from_sorted_unique(&[], 10);
        assert_eq!(s.n, 0);
    }

    #[test]
    fn stats_consecutive() {
        let ids: Vec<u32> = (0..10).collect();
        let s = IdListStats::from_sorted_unique(&ids, 100);
        assert_eq!(s.n, 10);
        assert_eq!(s.min_id, 0);
        assert_eq!(s.max_id, 9);
        assert!((s.mean_gap - 1.0).abs() < 1e-12);
        assert_eq!(s.max_gap, 1);
        assert!(s.frac_small_gaps > 0.99);
    }
}
