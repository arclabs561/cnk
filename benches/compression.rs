//! Benchmarks for ID set compression.

use cnk::{IdSetCompressor, RocCompressor};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("compress");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000, 10000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = num_ids * 100 + 10_000;

        group.throughput(Throughput::Elements(num_ids as u64));
        group.bench_with_input(BenchmarkId::new("roc", num_ids), &num_ids, |bench, _| {
            bench.iter(|| compressor.compress_set(black_box(&ids), black_box(universe_size)))
        });

        #[cfg(feature = "sbits")]
        {
            let ef = cnk::EliasFanoCompressor::new();
            group.bench_with_input(
                BenchmarkId::new("elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| ef.compress_set(black_box(&ids), black_box(universe_size)))
                },
            );

            let pef = cnk::PartitionedEliasFanoCompressor::with_block_size(128);
            group.bench_with_input(
                BenchmarkId::new("partitioned_elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| pef.compress_set(black_box(&ids), black_box(universe_size)))
                },
            );
        }
    }

    group.finish();
}

fn bench_decompress(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000, 10000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = num_ids * 100 + 10_000;
        let compressed = compressor.compress_set(&ids, universe_size).unwrap();

        group.throughput(Throughput::Elements(num_ids as u64));
        group.bench_with_input(BenchmarkId::new("roc", num_ids), &num_ids, |bench, _| {
            bench.iter(|| {
                compressor.decompress_set(black_box(&compressed), black_box(universe_size))
            })
        });

        #[cfg(feature = "sbits")]
        {
            let ef = cnk::EliasFanoCompressor::new();
            let ef_bytes = ef.compress_set(&ids, universe_size).unwrap();
            group.bench_with_input(
                BenchmarkId::new("elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| ef.decompress_set(black_box(&ef_bytes), black_box(universe_size)))
                },
            );

            let pef = cnk::PartitionedEliasFanoCompressor::with_block_size(128);
            let pef_bytes = pef.compress_set(&ids, universe_size).unwrap();
            group.bench_with_input(
                BenchmarkId::new("partitioned_elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| {
                        pef.decompress_set(black_box(&pef_bytes), black_box(universe_size))
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = num_ids * 100 + 10_000;

        group.throughput(Throughput::Elements(num_ids as u64));
        group.bench_with_input(BenchmarkId::new("roc", num_ids), &num_ids, |bench, _| {
            bench.iter(|| {
                let compressed = compressor
                    .compress_set(black_box(&ids), black_box(universe_size))
                    .unwrap();
                compressor
                    .decompress_set(black_box(&compressed), black_box(universe_size))
                    .unwrap()
            })
        });

        #[cfg(feature = "sbits")]
        {
            let ef = cnk::EliasFanoCompressor::new();
            group.bench_with_input(
                BenchmarkId::new("elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| {
                        let bytes = ef
                            .compress_set(black_box(&ids), black_box(universe_size))
                            .unwrap();
                        ef.decompress_set(black_box(&bytes), black_box(universe_size))
                            .unwrap()
                    })
                },
            );

            let pef = cnk::PartitionedEliasFanoCompressor::with_block_size(128);
            group.bench_with_input(
                BenchmarkId::new("partitioned_elias_fano", num_ids),
                &num_ids,
                |bench, _| {
                    bench.iter(|| {
                        let bytes = pef
                            .compress_set(black_box(&ids), black_box(universe_size))
                            .unwrap();
                        pef.decompress_set(black_box(&bytes), black_box(universe_size))
                            .unwrap()
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_compress, bench_decompress, bench_round_trip);
criterion_main!(benches);
