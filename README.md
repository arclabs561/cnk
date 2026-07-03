# cnk

[![crates.io](https://img.shields.io/crates/v/cnk.svg)](https://crates.io/crates/cnk)
[![Documentation](https://docs.rs/cnk/badge.svg)](https://docs.rs/cnk)
[![CI](https://github.com/arclabs561/cnk/actions/workflows/ci.yml/badge.svg)](https://github.com/arclabs561/cnk/actions/workflows/ci.yml)

ID set compression with auto codec selection.

```rust
use cnk::{compress_set_enveloped, decompress_set_enveloped, ChooseConfig};

let ids = vec![1u32, 5, 10, 20, 50];
let universe_size = 1000;

// Compress with an auto-selected codec and a self-describing envelope.
let bytes = compress_set_enveloped(&ids, universe_size, ChooseConfig::default()).unwrap();

// Decompress without separately tracking the selected codec.
let (_, _, restored) = decompress_set_enveloped(&bytes).unwrap();
assert_eq!(ids, restored);
```

A set of size n from universe [N] has C(N, n) possibilities, so the
information-theoretic minimum is log₂ C(N, n) bits. `cnk` provides byte-stream
codecs for sorted, unique `u32` ID sets.

## Codecs

| Compressor                       | Feature required | Random access |
|----------------------------------|------------------|---------------|
| `DeltaVarintCompressor`          | (always available) | no          |
| `EliasFanoCompressor`            | `sbits`          | yes           |
| `PartitionedEliasFanoCompressor` | `sbits`          | yes           |
| `RocCompressor`                  | `ans`            | no            |

## Auto selection

`compress_set_auto` / `decompress_set_auto` choose a codec based on list
statistics (gap distribution, density). The returned `CodecChoice` should
be recorded alongside the bytes so decode does not depend on build-time
defaults.

For persistence, prefer the envelope API (`compress_set_enveloped` /
`decompress_set_enveloped`) which stores codec, parameters, universe
size, element count, and a CRC32 in a self-describing header.

Auto selection currently chooses among delta+varint, Elias-Fano, and
partitioned Elias-Fano when the relevant features are enabled. `RocCompressor`
is available directly behind the `ans` feature, but is not selected by the
default chooser.

## Succinct baselines (feature `sbits`)

Enables Elias-Fano and Partitioned Elias-Fano codecs powered by `sbits`.
Useful when random access or skipping inside lists matters (posting lists,
graph structures).

## Examples

Runnable examples live in [`examples/`](examples/):

- `compressed_postings` compresses an inverted-index posting list with cnk, the storage-shrink use case these ID-set codecs exist for.

## Non-goals

- **Not a set object**: cnk compresses to byte streams. For intersection,
  union, or in-memory set operations, use `roaring`.
- **Not a framework**: query planning, scoring, and segment management
  belong in the engine layer (e.g. tantivy).
- **Not general-purpose compression**: requires sorted, unique u32 IDs and
  a universe size.
- **Not an optimal-codec searcher**: the chooser is deterministic and
  conservative. Benchmark on the target distribution when codec choice is
  load-bearing.

Dual-licensed under MIT or Apache-2.0.
