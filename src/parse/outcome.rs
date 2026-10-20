//! @agents-index The parse outcome of one exchange, as stored in exchanges.parse_outcome.

/// Why an exchange made calls or did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Call,
    NoUsage,
    Truncated,
    DecryptFailed,
    UnknownFormat,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Call => "call",
            Outcome::NoUsage => "no_usage",
            Outcome::Truncated => "truncated",
            Outcome::DecryptFailed => "decrypt_failed",
            Outcome::UnknownFormat => "unknown_format",
        }
    }
}
