//! Compare CNK's `ans`-feature set codec with the set entropy bound.
//!
//! For a sorted unique set of size n drawn from universe [N], the lower bound is
//! log2 C(N,n) bits. `RocCompressor` is an rANS-backed sorted-set codec, not a
//! full BB-ANS Random Order Coding implementation, so this example prints the
//! measured gap instead of treating the bound as achieved.

use cnk::{DeltaVarintCompressor, IdSetCompressor, RocCompressor};

struct Case {
    name: &'static str,
    universe: u32,
    ids: Vec<u32>,
}

fn main() {
    let cases = vec![
        Case {
            name: "sparse-even",
            universe: 100_000,
            ids: (0..128).map(|i| 17 + i * 733).collect(),
        },
        Case {
            name: "dense-prefix",
            universe: 10_000,
            ids: (0..2_000).collect(),
        },
        Case {
            name: "blocky",
            universe: 50_000,
            ids: blocky_ids(),
        },
    ];

    let delta = DeltaVarintCompressor::new();
    let roc = RocCompressor::new();

    println!(
        "{:<13} {:>6} {:>9} {:>9} {:>8} {:>8} {:>8} {:>9}",
        "case", "n", "universe", "bound B", "raw B", "delta B", "rans B", "rans/bound"
    );
    println!("{}", "-".repeat(84));

    for case in cases {
        let bound_bits = log2_choose(case.universe as u64, case.ids.len() as u64);
        let bound_bytes = (bound_bits / 8.0).ceil() as usize;

        let delta_bytes = delta
            .compress_set(&case.ids, case.universe)
            .expect("delta compression")
            .len();
        let roc_bytes = roc
            .compress_set(&case.ids, case.universe)
            .expect("rans compression")
            .len();
        let roundtrip = roc
            .decompress_set(
                &roc.compress_set(&case.ids, case.universe)
                    .expect("rans compression"),
                case.universe,
            )
            .expect("rans decompression");
        assert_eq!(roundtrip, case.ids);

        println!(
            "{:<13} {:>6} {:>9} {:>9} {:>8} {:>8} {:>8} {:>9.2}",
            case.name,
            case.ids.len(),
            case.universe,
            bound_bytes,
            case.ids.len() * 4,
            delta_bytes,
            roc_bytes,
            roc_bytes as f64 / bound_bytes.max(1) as f64
        );
    }
}

fn blocky_ids() -> Vec<u32> {
    let mut ids = Vec::new();
    for base in [100, 2_000, 20_000, 35_000] {
        for offset in 0..64 {
            ids.push(base + offset);
        }
    }
    ids
}

fn log2_choose(n: u64, k: u64) -> f64 {
    if k > n {
        return f64::INFINITY;
    }
    let k = k.min(n - k);
    (1..=k)
        .map(|i| ((n - k + i) as f64).log2() - (i as f64).log2())
        .sum()
}
