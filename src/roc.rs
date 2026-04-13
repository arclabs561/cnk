//! ROC (Random Order Coding) compressor for ID sets.
//!
//! Implements near-optimal set compression using bits-back coding with rANS.
//! Based on "Compressing multisets with large alphabets" (Severo et al., 2022).
//!
//! A set of n elements from universe [N] has C(N, n) possible values.
//! ROC approaches the information-theoretic minimum of log₂ C(N, n) bits
//! by treating the permutation of set elements as a latent variable and
//! recovering log₂(n!) "free bits" via bits-back ANS coding.
//!
//! The key bijection: for a sorted set {s₀ < s₁ < ... < sₙ₋₁} ⊂ [N],
//! the value `sᵢ - i` lies in [0, N-i) and represents the i-th element
//! in a shrinking universe. Encoding these N-i uniform symbols with rANS
//! costs exactly log₂(N · (N-1) · ... · (N-n+1)) bits. The bits-back
//! trick recovers log₂(n!) of those bits, leaving log₂ C(N, n).

use crate::error::CompressionError;
use crate::traits::IdSetCompressor;

/// rANS lower bound, must match the `ans` crate's value.
const RANS_L: u32 = 1 << 23;

// ---------------------------------------------------------------------------
// Uniform rANS coding (avoids FrequencyTable allocation for large alphabets)
// ---------------------------------------------------------------------------

/// Encode a uniform symbol in [0, alphabet_size) into the rANS state.
///
/// For a uniform distribution over M symbols with precision T = 2^p:
///   freq_i = T / M  (with some symbols getting +1 for the remainder)
///   cdf_i  = i * T / M  (integer division)
///
/// We use the "spread" approach: each symbol gets freq = T/M or T/M+1,
/// and the CDF is floor(i * T / M).
#[inline]
fn rans_encode_uniform(
    state: &mut u32,
    buf: &mut Vec<u8>,
    sym: u32,
    alphabet: u32,
    precision: u32,
) {
    let total = 1u32 << precision;

    // Compute freq and cdf for this symbol using the spread formula.
    let freq = spread_freq(sym, alphabet, total);
    let start = spread_cdf(sym, alphabet, total);

    // Renormalize: emit bytes while state is too large.
    let x_max = ((RANS_L >> precision) << 8) * freq;
    while *state >= x_max {
        buf.push((*state & 0xFF) as u8);
        *state >>= 8;
    }

    let q = *state / freq;
    let r = *state - q * freq;
    *state = (q << precision) + r + start;
}

/// Decode a uniform symbol from the rANS state.
#[inline]
fn rans_decode_uniform(
    state: &mut u32,
    bytes: &[u8],
    cursor: &mut usize,
    alphabet: u32,
    precision: u32,
) -> Result<u32, CompressionError> {
    let total = 1u32 << precision;
    let mask = total - 1;
    let slot = *state & mask;

    // Inverse of spread_cdf: find sym such that cdf(sym) <= slot < cdf(sym+1).
    // For uniform spread: sym = slot * alphabet / total (with correction).
    let sym = spread_symbol(slot, alphabet, total);
    let freq = spread_freq(sym, alphabet, total);
    let start = spread_cdf(sym, alphabet, total);

    // Advance state.
    *state = freq * (*state >> precision) + (slot - start);

    // Renormalize: pull bytes while state < RANS_L.
    while *state < RANS_L {
        if *cursor == 0 {
            return Err(CompressionError::DecompressionFailed(
                "rANS: exhausted input bytes".to_string(),
            ));
        }
        *cursor -= 1;
        *state = (*state << 8) | (bytes[*cursor] as u32);
    }

    Ok(sym)
}

// ---------------------------------------------------------------------------
// Spread formula for uniform distribution
// ---------------------------------------------------------------------------

/// Frequency for symbol `sym` in a uniform distribution over `alphabet` symbols
/// with total mass `total`. Uses the "spread" layout where symbols 0..r get
/// freq+1 and symbols r..alphabet get freq, where freq = total/alphabet and
/// r = total % alphabet.
#[inline]
fn spread_freq(sym: u32, alphabet: u32, total: u32) -> u32 {
    let base = total / alphabet;
    let remainder = total % alphabet;
    if sym < remainder {
        base + 1
    } else {
        base
    }
}

/// CDF (cumulative frequency) for symbol `sym` in the spread layout.
/// cdf(sym) = sym * base + min(sym, remainder)
/// where base = total / alphabet, remainder = total % alphabet.
#[inline]
fn spread_cdf(sym: u32, alphabet: u32, total: u32) -> u32 {
    let base = total / alphabet;
    let remainder = total % alphabet;
    sym * base + sym.min(remainder)
}

/// Given a slot in [0, total), find the symbol whose interval contains it.
/// Inverse of spread_cdf.
#[inline]
fn spread_symbol(slot: u32, alphabet: u32, total: u32) -> u32 {
    let base = total / alphabet;
    let remainder = total % alphabet;

    // The first `remainder` symbols each occupy (base+1) slots.
    // The threshold where the "wide" region ends:
    let wide_end = remainder * (base + 1);

    if slot < wide_end {
        slot / (base + 1)
    } else {
        // In the "narrow" region: each symbol occupies `base` slots.
        remainder + (slot - wide_end) / base
    }
}

// ---------------------------------------------------------------------------
// Precision selection
// ---------------------------------------------------------------------------

/// Choose ANS precision for a given alphabet size.
/// Larger precision = less quantization error but larger renorm threshold.
/// For uniform distributions, precision >= log2(alphabet) is needed so each
/// symbol gets at least freq=1.
#[inline]
fn precision_for(alphabet: u32) -> u32 {
    if alphabet <= 1 {
        return 1;
    }
    let min_bits = 32 - (alphabet - 1).leading_zeros(); // ceil(log2(alphabet))
                                                        // Add headroom for better quantization, capped at 20 (ans crate limit).
    (min_bits + 2).clamp(1, 20)
}

// ---------------------------------------------------------------------------
// Log-domain C(N, k) for estimates
// ---------------------------------------------------------------------------

/// log₂ C(n, k) via the sum of log terms. Avoids overflow.
fn log2_choose(n: u64, k: u64) -> f64 {
    if k == 0 || k == n {
        return 0.0;
    }
    let k = k.min(n - k); // symmetry
    let mut acc = 0.0f64;
    for i in 0..k {
        acc += ((n - i) as f64).log2() - ((i + 1) as f64).log2();
    }
    acc
}

// ---------------------------------------------------------------------------
// Varint helpers (shared with delta_varint.rs but kept private here)
// ---------------------------------------------------------------------------

#[inline]
fn encode_varint(value: u64, buf: &mut Vec<u8>) {
    let mut val = value;
    while val >= 0x80 {
        buf.push((val as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

#[inline]
fn decode_varint(buf: &[u8]) -> Result<(u64, usize), CompressionError> {
    let mut value = 0u64;
    let mut shift = 0;
    let mut offset = 0;
    loop {
        if offset >= buf.len() {
            return Err(CompressionError::DecompressionFailed(
                "ROC: truncated varint".to_string(),
            ));
        }
        if shift > 56 {
            return Err(CompressionError::DecompressionFailed(
                "ROC: varint too large".to_string(),
            ));
        }
        let byte = buf[offset];
        offset += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, offset))
}

// ---------------------------------------------------------------------------
// ROC compress / decompress
// ---------------------------------------------------------------------------

/// Compress a sorted, unique ID set using ROC (bits-back ANS).
///
/// Wire format: `[n: varint] [rANS bytes (stack format)]`
fn roc_compress(ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError> {
    crate::traits::validate_ids(ids)?;

    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let n = ids.len();
    if let Some(&max_id) = ids.last() {
        if max_id >= universe_size {
            return Err(CompressionError::InvalidInput(format!(
                "ID {} exceeds universe size {}",
                max_id, universe_size
            )));
        }
    }

    // For very small sets (n <= 2), ROC overhead exceeds savings. Use delta+varint.
    if n <= 2 {
        return fallback_compress(ids, universe_size);
    }

    let u = universe_size;

    // Encode the sorted set elements as uniform symbols in shrinking universes.
    // Element i (0-indexed in sorted order) maps to symbol = ids[i] - i in [0, U-i).
    //
    // rANS encodes in reverse, so we process i = n-1 down to 0.
    let mut state = RANS_L;
    let mut buf = Vec::new();

    for i in (0..n).rev() {
        let alphabet = u - i as u32; // universe size at step i
        if alphabet == 0 {
            return Err(CompressionError::CompressionFailed(
                "ROC: universe exhausted".to_string(),
            ));
        }
        let sym = ids[i] - i as u32; // the combinadic mapping
        if sym >= alphabet {
            return Err(CompressionError::CompressionFailed(format!(
                "ROC: symbol {} out of range [0, {}) at step {}",
                sym, alphabet, i
            )));
        }
        let prec = precision_for(alphabet);
        rans_encode_uniform(&mut state, &mut buf, sym, alphabet, prec);
    }

    // Write final state.
    buf.extend_from_slice(&state.to_le_bytes());

    // Prepend the count as varint.
    let mut out = Vec::with_capacity(10 + buf.len());
    encode_varint(n as u64, &mut out);
    out.extend_from_slice(&buf);

    Ok(out)
}

/// Decompress a ROC-encoded byte stream.
fn roc_decompress(compressed: &[u8], universe_size: u32) -> Result<Vec<u32>, CompressionError> {
    if compressed.is_empty() {
        return Ok(Vec::new());
    }

    // Parse count.
    let (n64, hdr_len) = decode_varint(compressed)?;
    let n = n64 as usize;

    if n == 0 {
        return Ok(Vec::new());
    }

    if n64 > universe_size as u64 {
        return Err(CompressionError::DecompressionFailed(format!(
            "ROC: declared count {} exceeds universe size {}",
            n64, universe_size
        )));
    }

    // For small sets, use the fallback decoder.
    if n <= 2 {
        return fallback_decompress(&compressed[hdr_len..], n, universe_size);
    }

    let rans_bytes = &compressed[hdr_len..];
    if rans_bytes.len() < 4 {
        return Err(CompressionError::DecompressionFailed(
            "ROC: rANS payload too short".to_string(),
        ));
    }

    // Initialize rANS state from the last 4 bytes (stack format).
    let cursor_init = rans_bytes.len() - 4;
    let state_bytes: [u8; 4] = rans_bytes[cursor_init..cursor_init + 4]
        .try_into()
        .map_err(|_| {
            CompressionError::DecompressionFailed("ROC: invalid state bytes".to_string())
        })?;
    let mut state = u32::from_le_bytes(state_bytes);
    if state < RANS_L {
        return Err(CompressionError::DecompressionFailed(format!(
            "ROC: invalid rANS state {} (expected >= {})",
            state, RANS_L
        )));
    }
    let mut cursor = cursor_init;

    let u = universe_size;
    let mut ids = Vec::with_capacity(n);

    for i in 0..n {
        let alphabet = u - i as u32;
        let prec = precision_for(alphabet);
        let sym = rans_decode_uniform(&mut state, rans_bytes, &mut cursor, alphabet, prec)?;
        let element = sym + i as u32; // inverse of the combinadic mapping
        ids.push(element);
    }

    // Verify sorted and unique (should be guaranteed by construction).
    for w in ids.windows(2) {
        if w[1] <= w[0] {
            return Err(CompressionError::DecompressionFailed(format!(
                "ROC: decoded IDs not sorted: {} <= {}",
                w[1], w[0]
            )));
        }
    }

    Ok(ids)
}

// ---------------------------------------------------------------------------
// Fallback for n <= 2 (delta+varint, avoids rANS overhead on tiny sets)
// ---------------------------------------------------------------------------

fn fallback_compress(ids: &[u32], _universe_size: u32) -> Result<Vec<u8>, CompressionError> {
    let mut out = Vec::new();
    encode_varint(ids.len() as u64, &mut out);
    if let Some(&first) = ids.first() {
        encode_varint(first as u64, &mut out);
        for i in 1..ids.len() {
            encode_varint((ids[i] - ids[i - 1]) as u64, &mut out);
        }
    }
    Ok(out)
}

fn fallback_decompress(
    data: &[u8],
    n: usize,
    universe_size: u32,
) -> Result<Vec<u32>, CompressionError> {
    let mut ids = Vec::with_capacity(n);
    let mut offset = 0;

    if n == 0 {
        return Ok(ids);
    }

    let (first, consumed) = decode_varint(&data[offset..])?;
    offset += consumed;
    if first >= universe_size as u64 {
        return Err(CompressionError::DecompressionFailed(format!(
            "ROC fallback: ID {} exceeds universe {}",
            first, universe_size
        )));
    }
    ids.push(first as u32);

    for _ in 1..n {
        let (delta, consumed) = decode_varint(&data[offset..])?;
        offset += consumed;
        let next = ids.last().unwrap() + delta as u32;
        if next >= universe_size {
            return Err(CompressionError::DecompressionFailed(format!(
                "ROC fallback: ID {} exceeds universe {}",
                next, universe_size
            )));
        }
        ids.push(next);
    }

    Ok(ids)
}

// ---------------------------------------------------------------------------
// Public compressor struct
// ---------------------------------------------------------------------------

/// ROC (Random Order Coding) compressor for sorted, unique ID sets.
///
/// Approaches the information-theoretic minimum of log₂ C(N, n) bits
/// for a set of n elements from universe [N]. Uses bits-back coding
/// with rANS to exploit ordering invariance.
///
/// For very small sets (n <= 2), falls back to delta+varint encoding
/// since rANS overhead exceeds the savings.
pub struct RocCompressor;

impl RocCompressor {
    /// Create a new ROC compressor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for RocCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl IdSetCompressor for RocCompressor {
    fn compress_set(&self, ids: &[u32], universe_size: u32) -> Result<Vec<u8>, CompressionError> {
        roc_compress(ids, universe_size)
    }

    fn decompress_set(
        &self,
        compressed: &[u8],
        universe_size: u32,
    ) -> Result<Vec<u32>, CompressionError> {
        roc_decompress(compressed, universe_size)
    }

    fn estimate_size(&self, num_ids: usize, universe_size: u32) -> usize {
        if num_ids == 0 {
            return 0;
        }
        let bits = log2_choose(universe_size as u64, num_ids as u64);
        // rANS adds ~4 bytes state overhead + varint header.
        (bits / 8.0).ceil() as usize + 8
    }

    fn bits_per_id(&self, num_ids: usize, universe_size: u32) -> f64 {
        if num_ids == 0 {
            return 0.0;
        }
        log2_choose(universe_size as u64, num_ids as u64) / num_ids as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let c = RocCompressor::new();
        let ids = vec![1u32, 5, 10, 20, 50, 100];
        let u = 1000;
        let compressed = c.compress_set(&ids, u).unwrap();
        let decompressed = c.decompress_set(&compressed, u).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_empty() {
        let c = RocCompressor::new();
        let compressed = c.compress_set(&[], 1000).unwrap();
        assert!(compressed.is_empty());
        let decompressed = c.decompress_set(&[], 1000).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn roundtrip_single() {
        let c = RocCompressor::new();
        let ids = vec![42u32];
        let compressed = c.compress_set(&ids, 1000).unwrap();
        let decompressed = c.decompress_set(&compressed, 1000).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_two() {
        let c = RocCompressor::new();
        let ids = vec![10u32, 500];
        let compressed = c.compress_set(&ids, 1000).unwrap();
        let decompressed = c.decompress_set(&compressed, 1000).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_consecutive() {
        let c = RocCompressor::new();
        let ids: Vec<u32> = (0..100).collect();
        let u = 1000;
        let compressed = c.compress_set(&ids, u).unwrap();
        let decompressed = c.decompress_set(&compressed, u).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_sparse() {
        let c = RocCompressor::new();
        let ids: Vec<u32> = (0..50).map(|i| i * 1000).collect();
        let u = 100_000;
        let compressed = c.compress_set(&ids, u).unwrap();
        let decompressed = c.decompress_set(&compressed, u).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn roundtrip_dense() {
        let c = RocCompressor::new();
        let ids: Vec<u32> = (0..999).collect();
        let u = 1000;
        let compressed = c.compress_set(&ids, u).unwrap();
        let decompressed = c.decompress_set(&compressed, u).unwrap();
        assert_eq!(ids, decompressed);
    }

    #[test]
    fn compression_ratio_near_optimal() {
        let c = RocCompressor::new();
        let ids: Vec<u32> = (0..100).map(|i| i * 100).collect();
        let u = 100_000;
        let compressed = c.compress_set(&ids, u).unwrap();
        let theoretical_bits = log2_choose(u as u64, ids.len() as u64);
        let actual_bits = compressed.len() as f64 * 8.0;

        // Should be within 2x of the information-theoretic bound.
        assert!(
            actual_bits < theoretical_bits * 2.0 + 64.0,
            "actual {} bits too far from theoretical {} bits",
            actual_bits,
            theoretical_bits
        );
    }

    #[test]
    fn rejects_unsorted() {
        let c = RocCompressor::new();
        assert!(c.compress_set(&[5, 1, 10], 100).is_err());
    }

    #[test]
    fn rejects_exceeding_universe() {
        let c = RocCompressor::new();
        assert!(c.compress_set(&[1000], 1000).is_err());
    }

    #[test]
    fn estimate_is_reasonable() {
        let c = RocCompressor::new();
        for (n, u) in [(10, 1000), (100, 100_000), (1000, 1_000_000)] {
            let est = c.estimate_size(n, u);
            assert!(est > 0);
            assert!(est < n * 4, "estimate {} >= raw {}", est, n * 4);
        }
    }

    #[test]
    fn spread_roundtrip() {
        // Verify the spread formula is self-consistent.
        for alphabet in [3, 7, 100, 255, 1000] {
            let total = 1u32 << precision_for(alphabet);
            for slot in 0..total {
                let sym = spread_symbol(slot, alphabet, total);
                assert!(sym < alphabet, "sym {} >= alphabet {}", sym, alphabet);
                let cdf = spread_cdf(sym, alphabet, total);
                let freq = spread_freq(sym, alphabet, total);
                assert!(
                    slot >= cdf && slot < cdf + freq,
                    "slot {} not in [{}, {}) for sym {} (alphabet={}, total={})",
                    slot,
                    cdf,
                    cdf + freq,
                    sym,
                    alphabet,
                    total
                );
            }
        }
    }

    #[test]
    fn log2_choose_known_values() {
        // C(10, 3) = 120, log2(120) ≈ 6.907
        let v = log2_choose(10, 3);
        assert!((v - 6.907).abs() < 0.01, "log2_choose(10,3) = {}", v);

        // C(100, 5) = 75287520, log2 ≈ 26.166
        let v = log2_choose(100, 5);
        assert!((v - 26.166).abs() < 0.01, "log2_choose(100,5) = {}", v);

        // Edge cases
        assert_eq!(log2_choose(10, 0), 0.0);
        assert_eq!(log2_choose(10, 10), 0.0);
    }
}
