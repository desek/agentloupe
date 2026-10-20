//! @agents-index The raw proxy token format: generate lpt_ plus base64url of 32 random bytes, parse it, and hash it with SHA-256.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

pub const PREFIX: &str = "lpt_";
/// The prefix plus 43 base64url characters for 32 bytes.
pub const LEN: usize = 47;

/// A new raw token from the OS random source.
pub fn generate() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the OS random source works");
    format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(b))
}

/// Checks that `raw` is a valid raw token; the reason never contains the value.
pub fn parse(raw: &str) -> Result<(), String> {
    let body = raw.strip_prefix(PREFIX).ok_or("does not start with lpt_")?;
    if raw.len() != LEN {
        return Err(format!("does not have {LEN} characters"));
    }
    match URL_SAFE_NO_PAD.decode(body) {
        Ok(b) if b.len() == 32 => Ok(()),
        _ => Err("is not lpt_ followed by base64url of 32 bytes with no padding".into()),
    }
}

/// The lowercase hex SHA-256 of the raw token, the only form that is stored.
pub fn hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_has_prefix_and_length() {
        let all: std::collections::HashSet<String> = (0..1000).map(|_| generate()).collect();
        assert_eq!(all.len(), 1000);
        for t in &all {
            assert!(t.starts_with(PREFIX));
            assert_eq!(t.len(), LEN);
            assert!(parse(t).is_ok());
        }
    }

    #[test]
    fn parse_rejects_bad_tokens() {
        let good = generate();
        let wrong_prefix = format!("xyz_{}", &good[4..]);
        let padded = format!("{}=", &good[..LEN - 1]);
        for bad in ["lpt_short", wrong_prefix.as_str(), padded.as_str(), ""] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
