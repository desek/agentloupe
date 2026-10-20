//! @agents-index HMAC-SHA256 under the account key of the request credential and of the account ID header.

use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// A hash in lowercase hex and the lowercase header name it came from.
pub type Hashed = Option<(String, String)>;

const ACCOUNT_ID_HEADERS: &[&str] = &["chatgpt-account-id"];

/// HMAC-SHA256 of `input` under `key` as lowercase hex.
pub fn hmac_hex(key: &[u8; 32], input: &[u8]) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("any key length is valid");
    m.update(input);
    hex::encode(m.finalize().into_bytes())
}

/// The credential: the value after `Bearer ` of `authorization`, else the value of `x-api-key`.
pub fn credential(key: &[u8; 32], h: &HeaderMap) -> Hashed {
    if let Some(v) = h.get("authorization") {
        let v = v.as_bytes();
        let token = v.strip_prefix(b"Bearer ").unwrap_or(v);
        return Some((hmac_hex(key, token), "authorization".into()));
    }
    h.get("x-api-key")
        .map(|v| (hmac_hex(key, v.as_bytes()), "x-api-key".into()))
}

/// The first account ID header that is present.
pub fn account_id(key: &[u8; 32], h: &HeaderMap) -> Hashed {
    ACCOUNT_ID_HEADERS.iter().find_map(|n| {
        h.get(*n)
            .map(|v| (hmac_hex(key, v.as_bytes()), n.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: [u8; 32] = [3u8; 32];

    fn map(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    #[test]
    fn hashes_bearer_then_api_key() {
        let a = credential(
            &K,
            &map(&[("authorization", "Bearer abc"), ("x-api-key", "xyz")]),
        );
        assert_eq!(a, Some((hmac_hex(&K, b"abc"), "authorization".into())));
        let b = credential(&K, &map(&[("x-api-key", "xyz")]));
        assert_eq!(b, Some((hmac_hex(&K, b"xyz"), "x-api-key".into())));
        assert_eq!(credential(&K, &HeaderMap::new()), None);
        assert_eq!(a.unwrap().0.len(), 64);
    }

    #[test]
    fn hashes_account_id_header() {
        let a = account_id(&K, &map(&[("chatgpt-account-id", "acct-1")]));
        assert_eq!(
            a,
            Some((hmac_hex(&K, b"acct-1"), "chatgpt-account-id".into()))
        );
        assert_eq!(account_id(&K, &HeaderMap::new()), None);
    }
}
