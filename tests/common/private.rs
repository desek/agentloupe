//! @agents-index Test helper of the private page: the admin token, a client with no redirects, and a fixture of tokens, sessions, and calls.
#![allow(dead_code)]

use super::loupe;

/// An admin token of 40 characters that a test can search for in the logs.
pub const ADMIN: &str = "adm1nTokenForTestsOnly-0123456789abcdefXY";

/// A blocking client that does not follow redirects, so a test sees each 303.
pub fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

/// Starts loupe with the admin token on a data dir that has the fixture tokens; returns the raw laptop token.
pub fn start_with_fixture() -> (loupe::Loupe, String) {
    let data = tempfile::tempdir().unwrap();
    let out = loupe::cmd(
        data.path(),
        &[
            "token",
            "create",
            "--name",
            "laptop",
            "--tag",
            "owner=alice",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let raw = v["token"].as_str().unwrap().to_string();
    loupe::create_token(data.path(), "ci");
    let l = loupe::Loupe::start_in(
        data,
        1,
        &[],
        &[("LOUPE_ADMIN_TOKEN", ADMIN), ("LOUPE_TZ", "UTC")],
    );
    seed(&l.db());
    (l, raw)
}

/// Sessions and calls of yesterday in UTC: laptop has 2 calls in session 1 (one unpriced), ci 1 call
/// in session 2, and session 3 has 1 call with no proxy token.
pub fn seed(db: &rusqlite::Connection) {
    // The fixture has calls with no exchange row, so the reference check is off for this connection.
    db.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    let day = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let id = |n: &str| -> i64 {
        db.query_row("SELECT id FROM proxy_tokens WHERE name = ?1", [n], |r| {
            r.get(0)
        })
        .unwrap()
    };
    let (laptop, ci) = (id("laptop"), id("ci"));
    let t = format!("{day}T10:00:00.000Z");
    for (sid, tok) in [(1, Some(laptop)), (2, Some(ci)), (3, None)] {
        db.execute(
            "INSERT INTO sessions (id, agent_label, proxy_token_id, key_hash, key_source, segment, started_at, last_call_at, state, call_count) \
             VALUES (?1, 'claude-code', ?2, 'k', 'header', 1, ?3, ?3, 'closed', 1)",
            rusqlite::params![sid, tok, t],
        )
        .unwrap();
    }
    // The list shows the session day summary; a kept segment line and the full summary are not shown.
    db.execute(
        "INSERT INTO session_summaries (session_id, status, line, attempts, created_at, updated_at) VALUES (1, 'done', 'Old segment line', 1, ?1, ?1)",
        [&t],
    )
    .unwrap();
    for (table, text) in [
        ("session_day_summaries", "Fixed the parser"),
        ("session_full_summaries", "The whole session"),
    ] {
        db.execute(
            &format!("INSERT INTO {table} (session_key, day, status, text, attempts, created_at, updated_at) VALUES ('k', ?1, 'done', ?2, 1, ?3, ?3)"),
            rusqlite::params![day, text, t],
        )
        .unwrap();
    }
    let calls = [
        (1, Some(laptop), 100, 10, Some("0.5")),
        (1, Some(laptop), 100, 10, None),
        (2, Some(ci), 50, 5, Some("0.25")),
        (3, None, 7, 0, Some("0.1")),
    ];
    for (i, (sid, tok, input, output, cost)) in calls.iter().enumerate() {
        db.execute(
            "INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd) \
             VALUES (?1, 0, ?2, 'claude-code', 'anthropic', ?3, ?4, 200, ?5, 0, 0, 0, ?6, ?7)",
            rusqlite::params![format!("e{i}"), t, tok, sid, input, output, cost],
        )
        .unwrap();
    }
}

/// GET `path` with the bearer admin token; returns the status and the JSON body.
pub fn get_json(l: &loupe::Loupe, path: &str) -> (u16, serde_json::Value) {
    let res = client().get(l.url(path)).bearer_auth(ADMIN).send().unwrap();
    let s = res.status().as_u16();
    (s, res.json().unwrap_or(serde_json::Value::Null))
}
