//! @agents-index The private report: the calls of a range grouped by proxy token or by one tag key, with per-day entries.

use super::query::Group;
use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;

/// The group of the calls that have no proxy token: calls from before the proxy tokens existed.
pub const NO_TOKEN: &str = "(no token)";
/// The group of the tokens that do not have the tag key.
pub const NO_TAG: &str = "(none)";

/// The sums of one group, its sums per day, and the token ID of its first call.
type GroupSums = (Agg, BTreeMap<NaiveDate, Agg>, Option<i64>);

/// One row of proxy_tokens, as the report shows it.
struct Token {
    name: String,
    tags: Value,
    created_at: String,
    last_used_at: Option<String>,
    revoked: bool,
}

/// The running sums of a group or of one day of a group; the cost stays exact.
#[derive(Default)]
struct Agg {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    calls: i64,
    sessions: HashSet<i64>,
    cost: Decimal,
    unpriced: i64,
}

impl Agg {
    fn add(&mut self, c: &[i64; 5], cost: Option<Decimal>) {
        self.input += c[1];
        self.output += c[2];
        self.cache_read += c[3];
        self.cache_write += c[4];
        self.calls += 1;
        self.sessions.insert(c[0]);
        match cost {
            Some(d) => self.cost += d,
            None => self.unpriced += 1,
        }
    }

    fn json(&self) -> serde_json::Map<String, Value> {
        let total = self.input + self.output + self.cache_read + self.cache_write;
        let Value::Object(m) = json!({
            "input_tokens": self.input,
            "output_tokens": self.output,
            "cache_read_tokens": self.cache_read,
            "cache_write_tokens": self.cache_write,
            "total_tokens": total,
            "calls": self.calls,
            "sessions": self.sessions.len(),
            "api_equivalent_cost_usd": self.cost.normalize().to_string(),
            "unpriced_calls": self.unpriced,
        }) else {
            unreachable!("a json object")
        };
        m
    }
}

fn tokens(conn: &Connection) -> rusqlite::Result<HashMap<i64, Token>> {
    let mut st = conn
        .prepare("SELECT id, name, tags, created_at, last_used_at, revoked_at FROM proxy_tokens")?;
    let rows = st.query_map([], |r| {
        let tags: String = r.get(2)?;
        Ok((
            r.get::<_, i64>(0)?,
            Token {
                name: r.get(1)?,
                tags: serde_json::from_str(&tags).unwrap_or_else(|_| json!({})),
                created_at: r.get(3)?,
                last_used_at: r.get(4)?,
                revoked: r.get::<_, Option<String>>(5)?.is_some(),
            },
        ))
    })?;
    rows.collect()
}

/// The group name of a call of the token `id`.
fn group_of(id: Option<i64>, toks: &HashMap<i64, Token>, group: &Group) -> String {
    let Some(t) = id.and_then(|i| toks.get(&i)) else {
        return NO_TOKEN.into();
    };
    match group {
        Group::Token => t.name.clone(),
        Group::Tag(k) => t
            .tags
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or(NO_TAG)
            .into(),
    }
}

/// The report of the days `from` to `to` in `tz`. A session counts in a group and on a day when
/// one of its calls is in that group and on that day.
pub fn build(
    conn: &Connection,
    tz: chrono_tz::Tz,
    from: NaiveDate,
    to: NaiveDate,
    group: &Group,
) -> rusqlite::Result<Value> {
    let toks = tokens(conn)?;
    let (lo, _) = crate::day::boundary::bounds(from, tz);
    let (_, hi) = crate::day::boundary::bounds(to, tz);
    let mut st = conn.prepare(
        "SELECT proxy_token_id, session_id, started_at, input, output, cache_read, \
         cache_write_5m + cache_write_1h, cost_usd FROM calls \
         WHERE started_at >= ?1 AND started_at < ?2",
    )?;
    let mut rows = st.query(rusqlite::params![lo, hi])?;
    let mut groups: BTreeMap<String, GroupSums> = BTreeMap::new();
    while let Some(r) = rows.next()? {
        let id: Option<i64> = r.get(0)?;
        let c = [r.get(1)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?];
        let started: String = r.get(2)?;
        let cost = r
            .get::<_, Option<String>>(7)?
            .and_then(|t| Decimal::from_str(&t).ok());
        let day = crate::day::boundary::day_of_text(&started, tz);
        let g = groups
            .entry(group_of(id, &toks, group))
            .or_insert_with(|| (Agg::default(), BTreeMap::new(), id));
        g.0.add(&c, cost);
        g.1.entry(day).or_default().add(&c, cost);
    }
    let mut out: Vec<Value> = groups
        .into_iter()
        .map(|(name, (all, days, id))| {
            let mut m = all.json();
            m.insert("group".into(), json!(name));
            let days: Vec<Value> = days
                .iter()
                .map(|(d, a)| {
                    let mut e = a.json();
                    e.insert("date".into(), json!(d.to_string()));
                    Value::Object(e)
                })
                .collect();
            m.insert("days".into(), json!(days));
            if let (Group::Token, Some(t)) = (group, id.and_then(|i| toks.get(&i))) {
                m.insert("tags".into(), t.tags.clone());
                m.insert("created_at".into(), json!(t.created_at));
                m.insert("last_used_at".into(), json!(t.last_used_at));
                m.insert(
                    "status".into(),
                    json!(if t.revoked { "revoked" } else { "active" }),
                );
            }
            Value::Object(m)
        })
        .collect();
    out.sort_by_key(|r| std::cmp::Reverse(r["total_tokens"].as_i64().unwrap_or(0)));
    let (g, k) = match group {
        Group::Token => ("token", None),
        Group::Tag(k) => ("tag", Some(k.as_str())),
    };
    Ok(
        json!({"from": from.to_string(), "to": to.to_string(), "group": g, "tag_key": k, "rows": out}),
    )
}
