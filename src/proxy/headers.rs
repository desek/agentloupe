//! @agents-index Header rules of the forwarder: drop accept-encoding and the hop-by-hop headers, keep the rest.

use axum::http::HeaderMap;
use axum::http::header::{ACCEPT_ENCODING, CONNECTION};

const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Removes the hop-by-hop headers and each header that `connection` names.
pub fn strip_hop_by_hop(h: &mut HeaderMap) {
    let named: Vec<String> = h
        .get_all(CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|n| n.trim().to_ascii_lowercase())
        .filter(|n| !n.is_empty())
        .collect();
    for n in HOP_BY_HOP
        .iter()
        .copied()
        .chain(named.iter().map(String::as_str))
    {
        h.remove(n);
    }
}

/// The request rule: hop-by-hop removed and `accept-encoding` removed, so the capture sees plain bytes.
pub fn for_request(h: &mut HeaderMap) {
    strip_hop_by_hop(h);
    h.remove(ACCEPT_ENCODING);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_hop_by_hop_and_accept_encoding() {
        let mut h = HeaderMap::new();
        for (k, v) in [
            ("connection", "x-foo"),
            ("x-foo", "1"),
            ("accept-encoding", "gzip"),
            ("te", "trailers"),
            ("x-keep", "2"),
            ("content-type", "application/json"),
        ] {
            h.insert(k, v.parse().unwrap());
        }
        for_request(&mut h);
        for gone in ["connection", "x-foo", "accept-encoding", "te"] {
            assert!(!h.contains_key(gone), "{gone}");
        }
        assert_eq!(h["x-keep"], "2");
        assert_eq!(h["content-type"], "application/json");
    }
}
