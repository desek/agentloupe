//! @agents-index The strong ETag of a static file: the quoted lowercase hex SHA-256 of its bytes.

use sha2::{Digest, Sha256};

pub fn of(bytes: &[u8]) -> String {
    format!("\"{}\"", hex::encode(Sha256::digest(bytes)))
}
