//! @agents-index Seals the bodies of one exchange record and inserts its row; a second insert of one id is ignored.

use super::record::ExchangeRecord;
use super::seal::seal;
use rusqlite::{Connection, params};

fn utc(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Inserts one row; `INSERT OR IGNORE` on the id makes a repeated spill import a no-op.
pub fn insert(conn: &Connection, key: &[u8; 32], r: &ExchangeRecord) -> Result<(), String> {
    let req = seal(key, &r.id, "request", &r.request_body)?;
    let resp = seal(key, &r.id, "response", &r.response_body)?;
    conn.execute(
        "INSERT OR IGNORE INTO exchanges (id, started_at, ended_at, ttfb_ms, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_body, response_body, request_truncated, response_truncated, credential_hash, credential_source, account_id_hash, account_id_source, outcome, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
        params![
            r.id, utc(r.started_ms), utc(r.ended_ms), r.ttfb_ms, r.agent, r.upstream, r.method, r.path, r.query,
            r.status, r.request_size as i64, r.response_size as i64, r.request_headers.to_string(),
            r.response_headers.to_string(), req, resp, r.request_truncated, r.response_truncated,
            r.credential_hash, r.credential_source, r.account_id_hash, r.account_id_source, r.outcome, r.error
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("database insert failed: {e}"))
}
