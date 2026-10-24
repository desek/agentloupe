//! @agents-index The admin session cookie: the value (expiry and HMAC-SHA-256), its check, and the Set-Cookie flags.

use axum::http::{HeaderMap, header};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const NAME: &str = "loupe_admin";
/// 12 hours.
pub const MAX_AGE: i64 = 43_200;

fn mac(key: &[u8; 32], expiry: i64) -> Hmac<Sha256> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    m.update(expiry.to_string().as_bytes());
    m
}

/// The value `<expiry unix seconds>.<hex HMAC>`; it holds no part of the admin token.
pub fn value(key: &[u8; 32], now_secs: i64) -> String {
    let expiry = now_secs + MAX_AGE;
    format!(
        "{expiry}.{}",
        hex::encode(mac(key, expiry).finalize().into_bytes())
    )
}

/// True if the HMAC matches under `key` (checked in constant time) and the expiry is in the future.
pub fn valid(key: &[u8; 32], value: &str, now_secs: i64) -> bool {
    let Some((exp, sig)) = value.split_once('.') else {
        return false;
    };
    let (Ok(expiry), Ok(sig)) = (exp.parse::<i64>(), hex::decode(sig)) else {
        return false;
    };
    expiry > now_secs && mac(key, expiry).verify_slice(&sig).is_ok()
}

/// The value of the `loupe_admin` cookie of a request, if any.
pub fn from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|p| {
            p.trim()
                .strip_prefix(NAME)
                .and_then(|r| r.strip_prefix('='))
                .map(str::to_string)
        })
}

/// True when the request came over HTTPS, directly or through a proxy that says so.
pub fn secure(headers: &HeaderMap, scheme_https: bool) -> bool {
    scheme_https
        || headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("https"))
}

/// The Set-Cookie text: `value` with `max_age`; a `max_age` of 0 clears the cookie.
pub fn set(value: &str, max_age: i64, secure: bool) -> String {
    let s = if secure { "; Secure" } else { "" };
    format!("{NAME}={value}; HttpOnly; SameSite=Strict; Path=/private; Max-Age={max_age}{s}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_checks_key_and_expiry() {
        let k = [7u8; 32];
        let v = value(&k, 1000);
        assert!(valid(&k, &v, 1000));
        assert!(!valid(&[8u8; 32], &v, 1000));
        assert!(!valid(&k, &v, 1000 + MAX_AGE));
        assert!(!valid(&k, "1.zz", 0));
    }
}
