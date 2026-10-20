//! @agents-index build_day: the pure day builder over calls, sessions, day summaries, and overrides; it reads no clock.

use super::boundary;
use super::record::{AgentTotals, Detail, ModelTotals, Record};
use super::sum::{Call, Sums, round4};
use crate::subscription::config::Subscription;
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;

/// The model key of a call with no model name.
const NO_MODEL: &str = "unknown";

fn calls(conn: &Connection, day: NaiveDate, tz: Tz) -> rusqlite::Result<Vec<Call>> {
    let (from, to) = boundary::bounds(day, tz);
    conn.prepare_cached(
        "SELECT agent_label, model, session_id, input, cache_read, cache_write_5m + cache_write_1h, output, cost_usd \
         FROM calls WHERE started_at >= ?1 AND started_at < ?2",
    )?
    .query_map(params![from, to], |r| {
        Ok(Call {
            agent_label: r.get(0)?,
            model: r.get::<_, Option<String>>(1)?.unwrap_or_else(|| NO_MODEL.into()),
            session_id: r.get(2)?,
            input: r.get(3)?,
            cache_read: r.get(4)?,
            cache_write: r.get(5)?,
            output: r.get(6)?,
            cost_usd: r.get(7)?,
        })
    })?
    .collect()
}

/// The summary of the day: the override first, then a done or fallback day line, else none.
pub fn summary(conn: &Connection, day: &str) -> rusqlite::Result<Option<String>> {
    let edited: Option<String> = conn
        .query_row(
            "SELECT summary FROM day_overrides WHERE day = ?1",
            [day],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if edited.is_some() {
        return Ok(edited);
    }
    Ok(conn
        .query_row(
            "SELECT line FROM day_summaries WHERE day = ?1 AND status IN ('done', 'fallback')",
            [day],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// The subscription cost of one day, rounded half to even to 4 fraction digits.
pub fn subscription_cost(day: NaiveDate, subscriptions: &[Subscription]) -> Decimal {
    let text = crate::subscription::prorate::subscription_cost_for_day(day, subscriptions);
    round4(Decimal::from_str(&text).unwrap_or_default())
}

/// Builds the record of `day`. The same rows give the same record, whatever their order.
pub fn build_day(
    conn: &Connection,
    day: NaiveDate,
    tz: Tz,
    subscriptions: &[Subscription],
) -> rusqlite::Result<Record> {
    let mut all = Sums::default();
    let mut agents: BTreeMap<String, Sums> = BTreeMap::new();
    let mut models: BTreeMap<String, Sums> = BTreeMap::new();
    for c in calls(conn, day, tz)? {
        all.add(&c);
        agents.entry(c.agent_label.clone()).or_default().add(&c);
        models.entry(c.model.clone()).or_default().add(&c);
    }
    let key = day.to_string();
    Ok(Record {
        summary: summary(conn, &key)?,
        day: key,
        input_total: all.input_total(),
        input: all.input,
        cache_read: all.cache_read,
        cache_write: all.cache_write,
        output: all.output,
        calls: all.calls,
        sessions: all.sessions.len() as i64,
        cost_usd: all.cost_rounded(),
        unpriced_calls: all.unpriced,
        subscription_cost_usd: subscription_cost(day, subscriptions),
        prs_opened: None,
        prs_merged: None,
        lines_added: None,
        lines_removed: None,
        detail: Detail {
            by_agent: agents
                .iter()
                .map(|(k, s)| (k.clone(), AgentTotals::from(s)))
                .collect(),
            by_model: models
                .iter()
                .map(|(k, s)| (k.clone(), ModelTotals::from(s)))
                .collect(),
        },
    })
}
