//! Error types for compression operations.

use std::fmt;

/// Errors that can occur during compression operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CompressionError {
    /// Invalid input (e.g., unsorted IDs, empty universe).
    InvalidInput(String),

    /// Compression operation failed.
    CompressionFailed(String),

    /// Decompression operation failed.
    DecompressionFailed(String),

    /// ANS encoding/decoding error.
    #[cfg(feature = "ans")]
    AnsError(String),
}

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressionError::InvalidInput(msg) => {
                write!(f, "Invalid input: {}", msg)
            }
            CompressionError::CompressionFailed(msg) => {
                write!(f, "Compression failed: {}", msg)
            }
            CompressionError::DecompressionFailed(msg) => {
                write!(f, "Decompression failed: {}", msg)
            }
            #[cfg(feature = "ans")]
            CompressionError::AnsError(msg) => {
                write!(f, "ANS encoding error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CompressionError {}
