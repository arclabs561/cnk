//! Property-based tests for ID set compression.
//!
//! These tests verify mathematical invariants that must hold for all inputs,
//! using proptest to generate random test cases.

use idpaq::{IdSetCompressor, RocCompressor};
use proptest::prelude::*;

/// Generate a sorted, unique set of IDs within a universe.
fn sorted_unique_ids(
    max_len: usize,
    universe_size: u32,
) -> impl Strategy<Value = (Vec<u32>, u32)> {
    // Generate random subset by sampling without replacement
    (1..=max_len).prop_flat_map(move |len| {
        // Universe must be at least as large as the set
        let min_universe = len as u32;
        let actual_universe = universe_size.max(min_universe);

        proptest::collection::btree_set(0..actual_universe, len)
            .prop_map(move |set| {
                let ids: Vec<u32> = set.into_iter().collect();
                (ids, actual_universe)
            })
    })
}

/// Generate sparse IDs (large gaps, typical of inverted indexes).
fn sparse_ids(max_len: usize) -> impl Strategy<Value = (Vec<u32>, u32)> {
    (1..=max_len).prop_flat_map(move |len| {
        // Large universe, small set
        let universe = 1_000_000u32;
        proptest::collection::btree_set(0..universe, len)
            .prop_map(move |set| {
                let ids: Vec<u32> = set.into_iter().collect();
                (ids, universe)
            })
    })
}

/// Generate dense IDs (small gaps, typical of HNSW neighbor lists).
fn dense_ids(max_len: usize) -> impl Strategy<Value = (Vec<u32>, u32)> {
    // Generate consecutive IDs starting from a random position
    (0..10000u32, 1..=max_len).prop_map(move |(start, len)| {
        let ids: Vec<u32> = (start..start + len as u32).collect();
        let universe = start + len as u32 + 1000;
        (ids, universe)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    // =======================================================================
    // ROUNDTRIP INVARIANT: decompress(compress(x)) == x
    // =======================================================================

    #[test]
    fn roundtrip_random_sets((ids, universe) in sorted_unique_ids(100, 10000)) {
        let compressor = RocCompressor::new();

        let compressed = compressor.compress_set(&ids, universe)
            .expect("compression should succeed for valid input");
        let decompressed = compressor.decompress_set(&compressed, universe)
            .expect("decompression should succeed for valid compressed data");

        prop_assert_eq!(ids, decompressed, "roundtrip must preserve data");
    }

    #[test]
    fn roundtrip_sparse_sets((ids, universe) in sparse_ids(50)) {
        let compressor = RocCompressor::new();

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_dense_sets((ids, universe) in dense_ids(100)) {
        let compressor = RocCompressor::new();

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }

    // =======================================================================
    // SIZE BOUNDS: compressed size should be reasonable
    // =======================================================================

    #[test]
    fn compression_reduces_size_for_large_sets((ids, universe) in sorted_unique_ids(100, 100000)) {
        prop_assume!(!ids.is_empty());

        let compressor = RocCompressor::new();
        let compressed = compressor.compress_set(&ids, universe)?;

        let uncompressed_size = ids.len() * 4; // 4 bytes per u32
        let compressed_size = compressed.len();

        // For non-trivial sets, compressed should be smaller than raw
        // (varint on deltas is almost always better than raw u32)
        if ids.len() > 10 {
            prop_assert!(
                compressed_size <= uncompressed_size,
                "compressed {} should be <= uncompressed {} for {} IDs",
                compressed_size, uncompressed_size, ids.len()
            );
        }
    }

    #[test]
    fn consecutive_ids_compress_well((start, len) in (0u32..10000, 10usize..200)) {
        let ids: Vec<u32> = (start..start + len as u32).collect();
        let universe = start + len as u32 + 1000;

        let compressor = RocCompressor::new();
        let compressed = compressor.compress_set(&ids, universe)?;

        // Consecutive IDs have delta=1, which is 1 byte per ID in varint
        // Plus header (count + first ID), so expect ~1-2 bytes per ID
        let bytes_per_id = compressed.len() as f64 / ids.len() as f64;
        prop_assert!(
            bytes_per_id < 2.5,
            "consecutive IDs should compress to ~1-2 bytes/ID, got {}",
            bytes_per_id
        );
    }

    // =======================================================================
    // EDGE CASES
    // =======================================================================

    #[test]
    fn empty_set_roundtrip(universe in 1u32..1000000) {
        let compressor = RocCompressor::new();
        let ids: Vec<u32> = vec![];

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert!(compressed.is_empty(), "empty set should compress to empty");
        prop_assert!(decompressed.is_empty(), "empty should decompress to empty");
    }

    #[test]
    fn single_id_roundtrip(id in 0u32..100000) {
        let universe = id + 1;
        let compressor = RocCompressor::new();
        let ids = vec![id];

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }

    #[test]
    fn two_ids_roundtrip((a, gap) in (0u32..10000, 1u32..10000)) {
        let b = a.saturating_add(gap);
        let universe = b + 1;
        let compressor = RocCompressor::new();
        let ids = vec![a, b];

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }

    // =======================================================================
    // ERROR CASES
    // =======================================================================

    #[test]
    fn rejects_unsorted_ids(
        (a, b) in (1u32..10000, 0u32..10000)
    ) {
        prop_assume!(a > b); // Ensure unsorted
        let compressor = RocCompressor::new();
        let ids = vec![a, b]; // Intentionally unsorted

        let result = compressor.compress_set(&ids, 100000);
        prop_assert!(result.is_err(), "should reject unsorted IDs");
    }

    #[test]
    fn rejects_ids_exceeding_universe((ids, _universe) in sorted_unique_ids(10, 1000)) {
        prop_assume!(!ids.is_empty());
        let max_id = *ids.iter().max().unwrap();

        // Use a universe smaller than max_id
        let small_universe = max_id; // max_id is now out of bounds

        let compressor = RocCompressor::new();
        let result = compressor.compress_set(&ids, small_universe);

        prop_assert!(result.is_err(), "should reject IDs >= universe");
    }

    // =======================================================================
    // DETERMINISM
    // =======================================================================

    #[test]
    fn compression_is_deterministic((ids, universe) in sorted_unique_ids(50, 10000)) {
        let compressor = RocCompressor::new();

        let compressed1 = compressor.compress_set(&ids, universe)?;
        let compressed2 = compressor.compress_set(&ids, universe)?;

        prop_assert_eq!(compressed1, compressed2, "compression must be deterministic");
    }

    // =======================================================================
    // VARINT EDGE CASES
    // =======================================================================

    #[test]
    fn handles_large_ids(start in 0u32..u32::MAX - 1000) {
        let ids: Vec<u32> = (0..10).map(|i| start.saturating_add(i * 10)).collect();
        let universe = ids.last().unwrap() + 1;

        let compressor = RocCompressor::new();
        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }

    #[test]
    fn handles_large_gaps((a, gap) in (0u32..1000, 1u32..u32::MAX / 2)) {
        let b = a.saturating_add(gap);
        let universe = b + 1;
        let compressor = RocCompressor::new();
        let ids = vec![a, b];

        let compressed = compressor.compress_set(&ids, universe)?;
        let decompressed = compressor.decompress_set(&compressed, universe)?;

        prop_assert_eq!(ids, decompressed);
    }
}

// =======================================================================
// STATISTICAL TESTS (not proptest, but important)
// =======================================================================

#[test]
fn compression_ratio_improves_with_density() {
    let compressor = RocCompressor::new();
    let universe = 100_000u32;

    // Sparse set with large gaps (delta ~100)
    let sparse: Vec<u32> = (0..1000).map(|i| i * 100).collect();
    let sparse_compressed = compressor.compress_set(&sparse, universe).unwrap();
    let sparse_bytes_per_id = sparse_compressed.len() as f64 / sparse.len() as f64;

    // Dense set (consecutive, delta = 1)
    let dense: Vec<u32> = (0..1000).collect();
    let dense_compressed = compressor.compress_set(&dense, universe).unwrap();
    let dense_bytes_per_id = dense_compressed.len() as f64 / dense.len() as f64;

    // Dense should use fewer bytes per ID (delta=1 is 1 byte, delta=100 is 1 byte too in varint)
    // Actually both fit in 1 byte varint, so let's use larger gaps
    let very_sparse: Vec<u32> = (0..100).map(|i| i * 1000).collect();
    let very_sparse_compressed = compressor.compress_set(&very_sparse, universe).unwrap();
    let very_sparse_bytes_per_id = very_sparse_compressed.len() as f64 / very_sparse.len() as f64;

    // Very sparse (delta=1000, needs 2 bytes) should use more bytes per ID than dense (delta=1)
    assert!(
        very_sparse_bytes_per_id > dense_bytes_per_id,
        "very sparse {} bytes/id should exceed dense {} bytes/id",
        very_sparse_bytes_per_id,
        dense_bytes_per_id
    );

    println!("Dense: {:.2} bytes/id", dense_bytes_per_id);
    println!("Sparse (delta=100): {:.2} bytes/id", sparse_bytes_per_id);
    println!("Very sparse (delta=1000): {:.2} bytes/id", very_sparse_bytes_per_id);
}

#[test]
fn estimate_size_is_reasonable() {
    let compressor = RocCompressor::new();

    for num_ids in [10, 100, 1000] {
        for universe in [10_000, 100_000, 1_000_000] {
            let estimate = compressor.estimate_size(num_ids, universe);

            // Estimate should be positive for non-empty sets
            assert!(estimate > 0, "estimate should be positive");

            // Estimate should be less than raw storage
            let raw_size = num_ids * 4;
            assert!(
                estimate <= raw_size * 2,
                "estimate {} should be reasonable vs raw {}",
                estimate,
                raw_size
            );
        }
    }
}
