//! @agents-index AES-256-GCM sealing of a body into a versioned blob bound to the exchange id and part, and its open.

// Self-contained on purpose: the integration tests include this file by path to decrypt stored bodies.
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

fn aad(id: &str, part: &str) -> Vec<u8> {
    format!("{id}:{part}").into_bytes()
}

/// Seals `body` with a fresh random nonce; the blob is version, nonce, ciphertext with tag.
pub fn seal(key: &[u8; 32], id: &str, part: &str, body: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| format!("no random nonce: {e}"))?;
    let aad = aad(id, part);
    let ct = Aes256Gcm::new(key.into())
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: body,
                aad: &aad,
            },
        )
        .map_err(|_| "encryption failed".to_string())?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// The one decrypt function: fails if the blob moved to another id or part, or the key is wrong.
pub fn open(key: &[u8; 32], id: &str, part: &str, blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 1 + NONCE_LEN || blob[0] != VERSION {
        return Err("the blob is not a version 1 sealed body".into());
    }
    let aad = aad(id, part);
    let nonce = Nonce::try_from(&blob[1..1 + NONCE_LEN])
        .map_err(|_| "the nonce has the wrong length".to_string())?;
    Aes256Gcm::new(key.into())
        .decrypt(
            &nonce,
            Payload {
                msg: &blob[1 + NONCE_LEN..],
                aad: &aad,
            },
        )
        .map_err(|_| "decryption failed: wrong key, id, or part".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: [u8; 32] = [9u8; 32];

    #[test]
    fn seal_then_open_round_trips() {
        let mut body = vec![0u8; 4096];
        getrandom::fill(&mut body).unwrap();
        let blob = seal(&K, "id1", "request", &body).unwrap();
        assert_eq!(open(&K, "id1", "request", &blob).unwrap(), body);
    }

    #[test]
    fn open_fails_on_wrong_id_or_part() {
        let blob = seal(&K, "id1", "request", b"x").unwrap();
        assert!(open(&K, "id2", "request", &blob).is_err());
        assert!(open(&K, "id1", "response", &blob).is_err());
    }

    #[test]
    fn nonce_is_unique() {
        let nonces: std::collections::HashSet<Vec<u8>> = (0..1000)
            .map(|_| seal(&K, "id", "request", b"same").unwrap()[1..13].to_vec())
            .collect();
        assert_eq!(nonces.len(), 1000);
    }
}
