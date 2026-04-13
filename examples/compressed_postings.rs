//! Compress postings lists with cnk.
//!
//! Builds a `postings::PostingsIndex` with ~100 documents, extracts doc-ID lists
//! for several terms, then compresses each list with `cnk::DeltaVarintCompressor`.
//! Prints a comparison table: raw size, delta-encoded size, cnk-compressed size,
//! and bits-per-id for each term's posting list.

use cnk::{DeltaVarintCompressor, IdSetCompressor};
use postings::codec::gaps_from_sorted_ids;
use postings::PostingsIndex;

/// Vocabulary used to build synthetic documents. Terms at lower indices
/// appear more frequently (Zipf-like).
const VOCAB: &[&str] = &[
    "the", "of", "and", "to", "in", "a", "is", "that", "for", "it", "was", "on", "are", "be",
    "with", "as", "at", "by", "this", "from", "or", "an", "but", "not", "all", "have", "had",
    "one", "our", "out", "up", "been", "she", "he", "they", "which", "their", "if", "will", "each",
    "about", "how", "many", "then", "them", "would", "like", "so", "these", "her",
];

/// Seed a deterministic PRNG (xorshift32).
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// Uniform in [0, n).
    fn uniform(&mut self, n: u32) -> u32 {
        self.next_u32() % n
    }
}

/// Build a postings index with `num_docs` documents. Returns the index and the
/// universe size (== num_docs, since doc IDs are 0..num_docs).
fn build_index(num_docs: u32) -> PostingsIndex<String> {
    let mut rng = Rng::new(0xDEAD_BEEF);
    let mut idx = PostingsIndex::new();

    for doc_id in 0..num_docs {
        // Document length: 5..35 terms, biased by doc_id for variety.
        let len = 5 + rng.uniform(30) as usize;
        let terms: Vec<String> = (0..len)
            .map(|_| {
                // Zipf-like: square the uniform draw to skew toward low indices.
                let r = rng.uniform(VOCAB.len() as u32) as usize;
                let r2 = (r * r) / VOCAB.len();
                VOCAB[r2.min(VOCAB.len() - 1)].to_string()
            })
            .collect();
        idx.add_document(doc_id, &terms).unwrap();
    }

    idx
}

/// Size of a gap sequence when each gap is varint-encoded.
fn varint_encoded_size(gaps: &[u32]) -> usize {
    let mut buf = Vec::new();
    for &g in gaps {
        postings::codec::varint::encode_u32(g, &mut buf);
    }
    buf.len()
}

/// Row in the summary table.
struct Row {
    term: String,
    df: u32,
    raw_bytes: usize,
    delta_bytes: usize,
    cnk_bytes: usize,
    bits_per_id: f64,
}

fn main() {
    let num_docs: u32 = 100;
    let universe_size = num_docs;

    // -- 1. Build index --
    let idx = build_index(num_docs);
    println!(
        "Index: {} docs, {} distinct terms, avg doc len {:.1}",
        idx.num_docs(),
        idx.terms().count(),
        idx.avg_doc_len(),
    );

    // -- 2. Pick terms spanning different frequencies --
    let mut term_dfs: Vec<(String, u32)> = idx.terms().map(|t| (t.clone(), idx.df(t))).collect();
    term_dfs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Select up to 8 terms: top-2 by df, bottom-2 by df, and 4 from the middle.
    let mut selected: Vec<String> = Vec::new();
    if term_dfs.len() <= 8 {
        selected.extend(term_dfs.iter().map(|(t, _)| t.clone()));
    } else {
        selected.push(term_dfs[0].0.clone());
        selected.push(term_dfs[1].0.clone());
        let mid = term_dfs.len() / 2;
        for i in mid.saturating_sub(2)..mid.saturating_sub(2) + 4 {
            if i < term_dfs.len() {
                selected.push(term_dfs[i].0.clone());
            }
        }
        selected.push(term_dfs[term_dfs.len() - 2].0.clone());
        selected.push(term_dfs[term_dfs.len() - 1].0.clone());
    }
    // Deduplicate (edge case with tiny vocab).
    selected.sort();
    selected.dedup();
    selected.sort_by_key(|t| std::cmp::Reverse(idx.df(t)));

    // -- 3. Compress each posting list and collect stats --
    let compressor = DeltaVarintCompressor::new();
    let mut rows: Vec<Row> = Vec::new();
    let mut all_ok = true;

    for term in &selected {
        // Extract sorted doc-ID list.
        let mut doc_ids: Vec<u32> = idx.postings_iter(term.as_str()).map(|(id, _)| id).collect();
        doc_ids.sort_unstable();
        doc_ids.dedup();

        let df = doc_ids.len() as u32;
        if df == 0 {
            continue;
        }

        // Raw size: 4 bytes per u32 doc ID.
        let raw_bytes = doc_ids.len() * 4;

        // Delta-encoded size (varint-packed gaps).
        let gaps = gaps_from_sorted_ids(&doc_ids).expect("ids are sorted");
        let delta_bytes = varint_encoded_size(&gaps);

        // cnk compressed size.
        let compressed = compressor
            .compress_set(&doc_ids, universe_size)
            .expect("compression should succeed");
        let cnk_bytes = compressed.len();

        // Theoretical bits-per-id from the compressor.
        let bpi = compressor.bits_per_id(doc_ids.len(), universe_size);

        // Round-trip verification.
        let decompressed = compressor
            .decompress_set(&compressed, universe_size)
            .expect("decompression should succeed");
        if decompressed != doc_ids {
            eprintln!(
                "ROUND-TRIP FAILURE for term '{}': expected {} ids, got {}",
                term,
                doc_ids.len(),
                decompressed.len()
            );
            all_ok = false;
        }

        rows.push(Row {
            term: term.clone(),
            df,
            raw_bytes,
            delta_bytes,
            cnk_bytes,
            bits_per_id: bpi,
        });
    }

    // -- 4. Print summary table --
    println!();
    println!(
        "{:<12} {:>4} {:>10} {:>12} {:>10} {:>8}",
        "term", "df", "raw bytes", "delta bytes", "cnk bytes", "bits/id"
    );
    println!("{}", "-".repeat(62));
    for r in &rows {
        println!(
            "{:<12} {:>4} {:>10} {:>12} {:>10} {:>8.2}",
            r.term, r.df, r.raw_bytes, r.delta_bytes, r.cnk_bytes, r.bits_per_id,
        );
    }

    // -- 5. Aggregates --
    let total_raw: usize = rows.iter().map(|r| r.raw_bytes).sum();
    let total_delta: usize = rows.iter().map(|r| r.delta_bytes).sum();
    let total_cnk: usize = rows.iter().map(|r| r.cnk_bytes).sum();
    println!("{}", "-".repeat(62));
    println!(
        "{:<12} {:>4} {:>10} {:>12} {:>10}",
        "TOTAL", "", total_raw, total_delta, total_cnk,
    );
    if total_raw > 0 {
        println!(
            "\ndelta compression ratio: {:.2}x",
            total_raw as f64 / total_delta as f64
        );
        println!(
            "cnk   compression ratio: {:.2}x",
            total_raw as f64 / total_cnk as f64
        );
    }

    // -- 6. Final verdict --
    println!();
    if all_ok {
        println!("All round-trip checks passed.");
    } else {
        eprintln!("Some round-trip checks FAILED.");
        std::process::exit(1);
    }
}
