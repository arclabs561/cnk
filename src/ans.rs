//! ANS integration seam for `cnk`.
//!
//! `cnk` (ID set compression) uses ANS as the entropy-coding backbone for
//! future ROC “bits-back” implementations. The actual entropy coder lives in
//! the `ans` crate; this module is a thin adapter so higher-level code can use
//! `cnk::ans::*` without directly depending on `ans`.

#[allow(unused_imports)] // public re-export surface for callers
pub use ans::{decode, encode, AnsError, FrequencyTable};

use crate::CompressionError;

impl From<AnsError> for CompressionError {
    fn from(e: AnsError) -> Self {
        CompressionError::CompressionFailed(format!("ans: {e}"))
    }
}
