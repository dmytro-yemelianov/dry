/// A decode error — the bytes are not a valid Dry IR v0 binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Ran out of bytes mid-field.
    Truncated,
    /// The magic prefix did not match.
    BadMagic,
    /// The encoding version is not supported by this build.
    UnsupportedVersion(u8),
    /// A dictionary string was not valid UTF-8.
    BadUtf8,
    /// The compressed body could not be inflated.
    BadCompression,
    /// The meta trailer was not valid `Meta` JSON.
    BadMeta,
    /// The segment kind dictionary contained a value this engine does not support.
    BadKind(String),
    /// The value cannot be represented in the fixed-width binary format.
    TooLarge { field: &'static str, len: usize },
    /// A declared input size exceeds the configured decoder resource budget.
    LimitExceeded {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    /// Generic or underlying I/O / JSON deserialization error.
    Other(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Truncated => write!(f, "truncated Dry IR binary"),
            CodecError::BadMagic => write!(f, "not a Dry IR binary (bad magic)"),
            CodecError::UnsupportedVersion(v) => write!(f, "unsupported encoding version {v}"),
            CodecError::BadUtf8 => write!(f, "invalid UTF-8 in kind dictionary"),
            CodecError::BadCompression => write!(f, "corrupt compressed body"),
            CodecError::BadMeta => write!(f, "invalid IR meta header"),
            CodecError::BadKind(s) => write!(f, "unsupported segment kind {s:?}"),
            CodecError::TooLarge { field, len } => {
                write!(f, "{field} length/count {len} exceeds u32::MAX")
            }
            CodecError::LimitExceeded {
                field,
                limit,
                actual,
            } => write!(
                f,
                "{field} length/count {actual} exceeds decoder limit {limit}"
            ),
            CodecError::Other(s) => write!(f, "error: {s}"),
        }
    }
}

impl std::error::Error for CodecError {}
