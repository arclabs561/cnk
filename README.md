# cnk

[![crates.io](https://img.shields.io/crates/v/cnk.svg)](https://crates.io/crates/cnk)
[![Documentation](https://docs.rs/cnk/badge.svg)](https://docs.rs/cnk)

ID set compression with auto codec selection.

Compress sorted, unique `u32` IDs to bytes and restore them later. Supply a
universe size greater than every ID. The envelope API stores the codec and
parameters with the data, so decoding needs only the bytes.

```toml
[dependencies]
cnk = "0.2"
```

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

## Codecs

| Compressor | Feature required |
|------------|------------------|
| `DeltaVarintCompressor` | None |
| `EliasFanoCompressor` | `sbits` |
| `PartitionedEliasFanoCompressor` | `sbits` |
| `RocCompressor` | `ans` |

All compressors decode the complete set into a `Vec<u32>`. These APIs do not
expose random access to the compressed data.

## Auto selection

`compress_set_auto` chooses a codec from the list's gap distribution and
density. Pass the returned `CodecChoice` to `decompress_set_auto` when decoding.

For persistence, prefer the envelope API (`compress_set_enveloped` /
`decompress_set_enveloped`) which stores codec, parameters, universe
size, element count, and a CRC32 in a self-describing header.

Auto selection chooses among delta+varint, Elias-Fano, and
partitioned Elias-Fano when the relevant features are enabled. The `ans`
feature enables the experimental rANS-backed `RocCompressor`. Use it directly;
the default chooser does not select it.

## Examples

Runnable examples live in [`examples/`](examples/):

- `compressed_postings` compresses the document IDs from a posting list.
- `roc_set_bounds` compares the `ans`-feature codec against the `log2 C(N,n)` set bound.

## Limitations

The codecs require sorted, unique IDs; they do not provide set operations or
indexing. The chooser is deterministic and uses size estimates rather than
trying every codec. Compare compressed sizes on your data when that choice
matters.

Dual-licensed under MIT or Apache-2.0.
