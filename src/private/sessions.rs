//! @agents-index The session list of one proxy token in a range: at most 200 rows, newest first, with a next cursor.

use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::str::FromStr;

/// The most rows of one response.
pub const PAGE: usize = 200;

/// The sessions with a call of the token `name` in the range, by session ID from the newest; the
/// name `(no token)` selects the calls with no proxy token. `cursor` is the `next` of the page before.
/// The tokens and the cost are of the calls in the range.
pub fn list(
    conn: &Connection,
    tz: chrono_tz::Tz,
    from: NaiveDate,
    to: NaiveDate,
    name: &str,
    cursor: Option<i64>,
) -> rusqlite::Result<Value> {
    let (lo, _) = crate::day::boundary::bounds(from, tz);
    let (_, hi) = crate::day::boundary::bounds(to, tz);
    let id: Option<i64> = if name == super::report::NO_TOKEN {
        None
    } else {
        match conn.query_row("SELECT id FROM proxy_tokens WHERE name = ?1", [name], |r| {
            r.get(0)
        }) {
            Ok(i) => Some(i),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Ok(json!({"rows": [], "next": null}));
            }
            Err(e) => return Err(e),
        }
    };
    let mut st = conn.prepare(
        "SELECT s.id, s.agent_label, s.started_at, s.last_call_at, s.call_count, ss.line, \
         c.input + c.output + c.cache_read + c.cache_write_5m + c.cache_write_1h, c.cost_usd \
         FROM calls c JOIN sessions s ON s.id = c.session_id \
         LEFT JOIN session_summaries ss ON ss.session_id = s.id \
         WHERE c.proxy_token_id IS ?1 AND c.started_at >= ?2 AND c.started_at < ?3 AND c.session_id < ?4 \
         ORDER BY c.session_id DESC",
    )?;
    let mut rows = st.query(rusqlite::params![id, lo, hi, cursor.unwrap_or(i64::MAX)])?;
    let mut out: Vec<(i64, Value, i64, Decimal)> = Vec::new();
    let mut next = None;
    while let Some(r) = rows.next()? {
        let sid: i64 = r.get(0)?;
        if out.last().map(|o| o.0) != Some(sid) {
            if out.len() == PAGE {
                next = out.last().map(|o| o.0);
                break;
            }
            let base = json!({
                "agent_label": r.get::<_, String>(1)?,
                "started_at": r.get::<_, String>(2)?,
                "last_call_at": r.get::<_, String>(3)?,
                "call_count": r.get::<_, i64>(4)?,
                "summary": r.get::<_, Option<String>>(5)?,
            });
            out.push((sid, base, 0, Decimal::ZERO));
        }
        let last = out.last_mut().expect("pushed above");
        last.2 += r.get::<_, i64>(6)?;
        if let Some(d) = r
            .get::<_, Option<String>>(7)?
            .and_then(|t| Decimal::from_str(&t).ok())
        {
            last.3 += d;
        }
    }
    let rows: Vec<Value> = out
        .into_iter()
        .map(|(_, mut v, t, c)| {
            v["total_tokens"] = json!(t);
            v["api_equivalent_cost_usd"] = json!(c.normalize().to_string());
            v
        })
        .collect();
    Ok(json!({"rows": rows, "next": next.map(|n| n.to_string())}))
}
