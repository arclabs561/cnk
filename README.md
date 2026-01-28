# cnk

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