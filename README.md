# idpaq

ID set compression primitives.

Dual-licensed under MIT or Apache-2.0.

## What

Compression for sorted, unique ID sets where order doesn't matter:
- IVF posting lists (which vectors belong to which cluster)
- HNSW neighbor lists (which nodes are connected)
- Inverted indexes (which documents contain which terms)

## Methods

- **Delta encoding** — varint-encodes gaps between sorted IDs
- **ROC** — Random Order Coding (bits-back with ANS, optimal for sets)

## Usage

```rust
use idpaq::{RocCompressor, IdSetCompressor};

let compressor = RocCompressor::new();
let ids = vec![1u32, 5, 10, 20, 50];
let universe_size = 1000;

// Compress
let compressed = compressor.compress_set(&ids, universe_size).unwrap();

// Decompress
let decompressed = compressor.decompress_set(&compressed, universe_size).unwrap();
assert_eq!(ids, decompressed);
```

## Theory

A set of $n$ elements from universe $[N]$ has $\binom{N}{n}$ possibilities.

Information-theoretic minimum: $\log_2 \binom{N}{n}$ bits.

This is less than encoding a sequence: $n \log_2 N$ bits (which ignores uniqueness).

ROC approaches this bound by treating permutation as a latent variable.

## Features

- `ans` — full ANS entropy coding via `constriction`
- `full` — all features

## Why "idpaq"

ID packing. Compress those posting lists.
