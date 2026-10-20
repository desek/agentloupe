//! @agents-index Reads one stored exchange row for the parser, with its sealed bodies and the hashes that the capture wrote.

use rusqlite::{Connection, params};

/// The columns of one exchange that the parser reads; the bodies are still sealed.
pub struct Row {
    pub id: String,
    pub started_at: String,
    pub agent: String,
    pub upstream: String,
    pub path: String,
    pub status: i64,
    pub response_headers: String,
    pub request_body: Vec<u8>,
    pub response_body: Vec<u8>,
    pub response_truncated: bool,
    pub credential_hash: Option<String>,
    pub credential_source: Option<String>,
    pub account_id_hash: Option<String>,
    pub account_id_source: Option<String>,
    pub session_hint_hash: Option<String>,
    pub proxy_token_id: Option<i64>,
}

pub fn read(conn: &Connection, rowid: i64) -> rusqlite::Result<Row> {
    conn.query_row(
        "SELECT id, started_at, agent, upstream, path, status, response_headers, request_body, response_body, response_truncated, credential_hash, credential_source, account_id_hash, account_id_source, session_hint_hash, proxy_token_id FROM exchanges WHERE rowid = ?1",
        params![rowid],
        |r| {
            Ok(Row {
                id: r.get(0)?,
                started_at: r.get(1)?,
                agent: r.get(2)?,
                upstream: r.get(3)?,
                path: r.get(4)?,
                status: r.get(5)?,
                response_headers: r.get(6)?,
                request_body: r.get(7)?,
                response_body: r.get(8)?,
                response_truncated: r.get(9)?,
                credential_hash: r.get(10)?,
                credential_source: r.get(11)?,
                account_id_hash: r.get(12)?,
                account_id_source: r.get(13)?,
                session_hint_hash: r.get(14)?,
                proxy_token_id: r.get(15)?,
            })
        },
    )
}

/// The value of `content-type` in the stored response headers, or empty.
pub fn content_type(headers_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(headers_json)
        .ok()
        .and_then(|v| {
            v.as_array()?
                .iter()
                .find(|p| {
                    p[0].as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
                })
                .and_then(|p| p[1].as_str().map(str::to_string))
        })
        .unwrap_or_default()
}
