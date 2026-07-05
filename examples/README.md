# cnk examples

Each example is runnable from the repo root.

| I want to... | Example |
|---|---|
| Compress posting-list document IDs | `compressed_postings` |
| Compare the `ans` codec with the set entropy bound | `roc_set_bounds` |

## Example dependencies

`compressed_postings` builds a small `postings::PostingsIndex`, extracts sorted
document ID lists, and compresses them with `cnk`.

`roc_set_bounds` uses `RocCompressor`, which requires the `ans` feature, and
prints byte counts next to the `log2 C(N,n)` lower bound.

```sh
cargo run --example compressed_postings
cargo run --features ans --example roc_set_bounds
```
