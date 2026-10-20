//! @agents-index A 32-byte key decoded from standard padded base64; anything else is rejected.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

pub const KEY_LEN: usize = 32;

/// Decodes a key; the error text says why the value is not a key and never repeats the value.
pub fn parse_key(text: &str) -> Result<[u8; KEY_LEN], String> {
    let bytes = STANDARD
        .decode(text.trim())
        .map_err(|_| "the value is not standard base64 with padding".to_string())?;
    bytes
        .try_into()
        .map_err(|b: Vec<u8>| format!("the value decodes to {} bytes, not {KEY_LEN}", b.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_key_not_32_bytes() {
        assert!(parse_key(&STANDARD.encode([1u8; 31])).is_err());
        assert!(parse_key(&STANDARD.encode([1u8; 33])).is_err());
        assert!(parse_key("not base64 at all!").is_err());
        assert!(parse_key(&STANDARD.encode([1u8; 32])).is_ok());
    }
}
