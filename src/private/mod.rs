//! @agents-index The private page: the state built from the admin token, and the handler of every path under /private/.

pub mod auth;
pub mod cookie;
pub mod headers;
pub mod limit;
pub mod login;
pub mod query;
pub mod report;
pub mod routes;
pub mod sessions;

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

/// What the private routes read. It exists only when the admin token is set, so without the
/// token no private route exists. The token itself is not kept: only its digest and the cookie key.
pub struct PrivateState {
    pub conn: Mutex<Connection>,
    pub tz: chrono_tz::Tz,
    /// SHA-256 of the admin token, for the constant-time compare.
    pub digest: [u8; 32],
    /// The HMAC key of the admin session cookie; a new admin token gives a new key.
    pub cookie_key: [u8; 32],
    pub limit: limit::Limiter,
}

impl PrivateState {
    /// Derives the digest and the cookie key from `admin_token`; the value is not stored.
    pub fn new(conn: Connection, tz: chrono_tz::Tz, admin_token: &str) -> PrivateState {
        PrivateState {
            conn: Mutex::new(conn),
            tz,
            digest: Sha256::digest(admin_token.as_bytes()).into(),
            cookie_key: cookie_key(admin_token),
            limit: limit::Limiter::default(),
        }
    }
}

/// The HMAC key of the admin session cookie for `admin_token`: a SHA-256 under a fixed label, so
/// the key differs from the digest that the bearer compare uses.
pub fn cookie_key(admin_token: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"loupe admin session cookie key\0");
    h.update(admin_token.as_bytes());
    h.finalize().into()
}
