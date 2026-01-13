//! Benchmarks for ID set compression.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use idpaq::{IdSetCompressor, RocCompressor};

fn bench_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("compress");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000, 10000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = (num_ids * 100 + 10000) as u32;

        group.throughput(Throughput::Elements(num_ids as u64));
        group.bench_with_input(BenchmarkId::new("roc", num_ids), &num_ids, |bench, _| {
            bench.iter(|| compressor.compress_set(black_box(&ids), black_box(universe_size)))
        });
    }

    group.finish();
}

fn bench_decompress(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompress");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000, 10000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = (num_ids * 100 + 10000) as u32;
        let compressed = compressor.compress_set(&ids, universe_size).unwrap();

        group.throughput(Throughput::Elements(num_ids as u64));
        group.bench_with_input(BenchmarkId::new("roc", num_ids), &num_ids, |bench, _| {
            bench.iter(|| {
                compressor.decompress_set(black_box(&compressed), black_box(universe_size))
            })
        });
    }

    group.finish();
}

fn bench_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    let compressor = RocCompressor::new();

    for num_ids in [100, 1000] {
        let ids: Vec<u32> = (0..num_ids).map(|i| i * 100).collect();
        let universe_size = (num_ids * 100 + 10000) as u32;

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
    }

    group.finish();
}

criterion_group!(benches, bench_compress, bench_decompress, bench_round_trip);
criterion_main!(benches);
