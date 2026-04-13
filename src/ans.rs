//! ANS integration seam for `cnk`.

use crate::CompressionError;
use ans::AnsError;

impl From<AnsError> for CompressionError {
    fn from(e: AnsError) -> Self {
        CompressionError::AnsError(format!("{e}"))
    }
}
