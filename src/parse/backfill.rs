//! @agents-index Prices the calls with a null price_commit; calls that have a price commit keep their cost, so history stays stable.

use super::cost::{price, warn};
use super::usage::Usage;
use rusqlite::{Connection, params};
use std::collections::BTreeMap;

/// The counts that `loupe price backfill` prints.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub calls_priced: u64,
    pub calls_unpriced: u64,
}

struct Row {
    id: i64,
    upstream: String,
    usage: Usage,
}

fn rows(conn: &Connection) -> rusqlite::Result<Vec<Row>> {
    conn.prepare(
        "SELECT id, upstream, model, service_tier, input, cache_read, cache_write_5m, cache_write_1h, output \
         FROM calls WHERE price_commit IS NULL ORDER BY id",
    )?
    .query_map([], |r| {
        Ok(Row {
            id: r.get(0)?,
            upstream: r.get(1)?,
            usage: Usage {
                model: r.get(2)?,
                service_tier: r.get(3)?,
                input: r.get(4)?,
                cache_read: r.get(5)?,
                cache_write_5m: r.get(6)?,
                cache_write_1h: r.get(7)?,
                output: r.get(8)?,
            },
        })
    })?
    .collect()
}

/// Prices every call with a null price_commit in one transaction; a dry run writes nothing.
pub fn run(
    conn: &Connection,
    providers: &BTreeMap<String, String>,
    dry_run: bool,
) -> rusqlite::Result<Counts> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let mut c = Counts::default();
        for row in rows(conn)? {
            let p = price(providers, &row.upstream, &row.usage);
            match &p.unpriced_reason {
                None => c.calls_priced += 1,
                Some(reason) => {
                    c.calls_unpriced += 1;
                    if !dry_run {
                        warn(row.id, row.usage.model.as_deref(), &row.upstream, reason);
                    }
                }
            }
            // The WHERE clause keeps a call that another pass priced in the meantime unchanged.
            conn.execute(
                "UPDATE calls SET cost_usd = ?2, price_commit = ?3, unpriced_reason = ?4 WHERE id = ?1 AND price_commit IS NULL",
                params![row.id, p.cost_usd, p.price_commit, p.unpriced_reason],
            )?;
        }
        Ok(c)
    })();
    let end = if dry_run || result.is_err() {
        "ROLLBACK"
    } else {
        "COMMIT"
    };
    conn.execute_batch(end)?;
    result
}
