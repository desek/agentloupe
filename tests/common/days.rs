//! @agents-index Test helper for day records: a migrated store, seeded sessions and calls, a build context, and loupe day commands.
#![allow(dead_code)]

use agent_loupe::day::rebuild::Ctx;
use agent_loupe::day::state::Review;
use agent_loupe::subscription::config::Subscription;
use rusqlite::{Connection, params};

/// A migrated store in a temp dir, with foreign keys off for the seed rows.
pub fn db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = agent_loupe::store::open::open(dir.path()).unwrap();
    agent_loupe::store::migrate::migrate(&mut c).unwrap();
    // The seeded calls point at no exchange row; the builder never reads exchanges.
    c.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    (dir, c)
}

/// Inserts a segment of `agent`, open or closed, and returns its id.
pub fn session(c: &Connection, agent: &str, started_at: &str, open: bool) -> i64 {
    c.query_row(
        "INSERT INTO sessions (agent_label, key_hash, key_source, segment, started_at, last_call_at, closed_at, state, call_count) \
         VALUES (?1, 'k' || abs(random()), 'header', 1, ?2, ?2, CASE WHEN ?3 THEN NULL ELSE ?2 END, CASE WHEN ?3 THEN 'open' ELSE 'closed' END, 1) RETURNING id",
        params![agent, started_at, open],
        |r| r.get(0),
    )
    .unwrap()
}

/// One call row; `tokens` is input, cache read, cache write 5m, cache write 1h, output.
pub struct Call<'a> {
    pub at: &'a str,
    pub agent: &'a str,
    pub model: &'a str,
    pub session: i64,
    pub tokens: [i64; 5],
    pub cost: Option<&'a str>,
}

pub fn call(c: &Connection, k: &Call) {
    let n: i64 = c
        .query_row("SELECT count(*) FROM calls", [], |r| r.get(0))
        .unwrap();
    let t = k.tokens;
    c.execute(
        "INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, session_id, model, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit) \
         VALUES (?1, 0, ?2, ?3, 'anthropic', ?4, ?5, 200, ?6, ?7, ?8, ?9, ?10, ?11, 'c')",
        params![format!("e{n}"), k.at, k.agent, k.session, k.model, t[0], t[1], t[2], t[3], t[4], k.cost],
    )
    .unwrap();
}

/// A simple call of `agent` at `at` in a new closed segment.
pub fn simple(c: &Connection, at: &str, agent: &str) -> i64 {
    let s = session(c, agent, at, false);
    call(
        c,
        &Call {
            at,
            agent,
            model: "claude-haiku-5-5",
            session: s,
            tokens: [10, 0, 0, 0, 5],
            cost: Some("0.001"),
        },
    );
    s
}

/// A build context in UTC with no summary client.
pub fn ctx(review: Review, subscriptions: Vec<Subscription>) -> Ctx {
    Ctx {
        tz: chrono_tz::UTC,
        subscriptions,
        review,
        summary: agent_loupe::summary::worker::Settings {
            data_key: [0; 32],
            client: None,
            public: vec![],
            max_attempts: 1,
            tz: chrono_tz::UTC,
        },
    }
}

pub fn date(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Unix milliseconds of an RFC 3339 time.
pub fn ms(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .timestamp_millis()
}

/// Runs `loupe <args>` against `data` with the test keys, LOUPE_TZ=UTC, and `env`.
pub fn loupe(data: &std::path::Path, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_DATA_KEY", key)
        .env("LOUPE_ACCOUNT_KEY", key)
        .env("LOUPE_TZ", "UTC");
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// The value of a key: value line of stdout.
pub fn field(out: &std::process::Output, key: &str) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}: ")).map(str::to_string))
        .unwrap_or_else(|| panic!("no {key} in {}", String::from_utf8_lossy(&out.stdout)))
}

/// The stored record of a day as JSON, or None.
pub fn record(c: &Connection, day: &str) -> Option<serde_json::Value> {
    c.query_row("SELECT record FROM days WHERE day = ?1", [day], |r| {
        r.get::<_, Option<String>>(0)
    })
    .ok()
    .flatten()
    .map(|t| serde_json::from_str(&t).unwrap())
}

pub fn scalar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn text(c: &Connection, sql: &str) -> Option<String> {
    c.query_row(sql, [], |r| r.get(0)).unwrap_or(None)
}
