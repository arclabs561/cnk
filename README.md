# cnk

[![crates.io](https://img.shields.io/crates/v/cnk.svg)](https://crates.io/crates/cnk)
[![Documentation](https://docs.rs/cnk/badge.svg)](https://docs.rs/cnk)
[![CI](https://github.com/arclabs561/cnk/actions/workflows/ci.yml/badge.svg)](https://github.com/arclabs561/cnk/actions/workflows/ci.yml)

Set compression via C(n,k): ROC, ANS, delta encoding.

Dual-licensed under MIT or Apache-2.0.

```rust
use cnk::{IdSetCompressor, RocCompressor};

let compressor = RocCompressor::new();
let ids = vec![1u32, 5, 10, 20, 50];
let universe_size = 1000;

let compressed = compressor.compress_set(&ids, universe_size).unwrap();
let decompressed = compressor.decompress_set(&compressed, universe_size).unwrap();

assert_eq!(ids, decompressed);
```

A set of size \(n\) from universe \([N]\) has $\binom{N}{n}$ possibilities, so the information-theoretic minimum is $\log_2 \binom{N}{n}$ bits.
ROC approaches this bound by treating permutation as a latent variable.

## Feature matrix

| Compressor                       | Feature required | Random access |
|----------------------------------|------------------|---------------|
| `RocCompressor`                  | (always available) | no          |
| `EliasFanoCompressor`            | `sbits`          | yes           |
| `PartitionedEliasFanoCompressor` | `sbits`          | yes           |

## Succinct baselines (feature `sbits`)

If you enable feature `sbits`, `cnk` exposes succinct monotone-sequence codecs powered by `sbits/`:

- `EliasFanoCompressor`
- `PartitionedEliasFanoCompressor` (cluster-aware baseline; see Ottaviano & Venturini, SIGIR 2014: [PDF](http://groups.di.unipi.it/~ottavian/files/elias_fano_sigir14.pdf))

These are useful when you care about **random access / skipping** inside lists (typical in posting lists and some graph/ANN structures).

## Method selection (“auto”)

Downstream repos often want a stable default without hardcoding a codec per call site. `cnk` provides:

- `IdListStats`: cheap per-list summary stats (gaps, clustering proxy)
- `choose_method(...)`: deterministic heuristic method choice
- `compress_set_auto(...)` / `decompress_set_auto(...)`: choose + encode/decode, with feature-aware fallbacks

The intent is: callers record the returned `CodecChoice` next to the bytes (so decode does not depend on build-time defaults).
