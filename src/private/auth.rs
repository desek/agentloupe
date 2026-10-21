//! @agents-index Authorization of a private request: the bearer admin token or a valid admin session cookie, compared in constant time.

use super::PrivateState;
use axum::http::{HeaderMap, header};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// True if `candidate` is the admin token. The compare is on the SHA-256 digests, so its time
/// does not depend on the length or the content of the admin token.
pub fn token_matches(state: &PrivateState, candidate: &str) -> bool {
    let d: [u8; 32] = Sha256::digest(candidate.as_bytes()).into();
    d.ct_eq(&state.digest).into()
}

/// True if the request carries `Authorization: Bearer <admin token>` or a valid admin session cookie.
/// A proxy token never matches, because only the admin token has this digest.
pub fn authorized(state: &PrivateState, headers: &HeaderMap, now_secs: i64) -> bool {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| token_matches(state, t.trim()));
    bearer
        || super::cookie::from_headers(headers)
            .is_some_and(|v| super::cookie::valid(&state.cookie_key, &v, now_secs))
}
