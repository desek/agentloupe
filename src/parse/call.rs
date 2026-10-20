//! @agents-index Stores one call of an exchange: its provider account, its session segment, and its calls row.

use super::exchange::Row;
use super::usage::Usage;
use crate::account::{source, store as accounts};
use crate::session::store::Scope;
use crate::session::{key, segment, time};
use rusqlite::{Connection, OptionalExtension, params};

/// The changes that one call made.
#[derive(Default)]
pub struct Added {
    pub calls: u64,
    pub opened: u64,
    pub closed: u64,
}

/// A call that exists for (exchange, index) adds nothing, so a repeated parse is a no-op.
pub fn store(
    conn: &Connection,
    account_key: &[u8; 32],
    idle_secs: u64,
    row: &Row,
    request: &[u8],
    u: &Usage,
) -> rusqlite::Result<Added> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM calls WHERE exchange_id = ?1 AND call_index = 0",
            [&row.id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if exists {
        return Ok(Added::default());
    }
    let req: serde_json::Value = serde_json::from_slice(request).unwrap_or_default();
    let stored = source::Stored {
        account_id: row
            .account_id_hash
            .as_deref()
            .zip(row.account_id_source.as_deref()),
        credential: row
            .credential_hash
            .as_deref()
            .zip(row.credential_source.as_deref()),
    };
    let account_id = source::select(account_key, &stored, &req)
        .map(|a| accounts::upsert(conn, &a, &row.started_at))
        .transpose()?;
    let k = key::select(account_key, row.session_hint_hash.as_deref(), &req);
    let scope = Scope {
        agent_label: row.agent.clone(),
        proxy_token_id: row.proxy_token_id,
        key_hash: k.hash,
    };
    let at_ms = time::ms(&row.started_at);
    let seg = segment::assign(conn, &scope, k.source, at_ms, idle_secs, account_id)?;
    conn.execute(
        "INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, account_id, model, service_tier, status, input, cache_read, cache_write_5m, cache_write_1h, output) \
         VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            row.id, row.started_at, row.agent, row.upstream, row.proxy_token_id, seg.session_id, account_id,
            u.model, u.service_tier, row.status, u.input, u.cache_read, u.cache_write_5m, u.cache_write_1h, u.output
        ],
    )?;
    Ok(Added {
        calls: 1,
        opened: seg.opened,
        closed: seg.closed,
    })
}
