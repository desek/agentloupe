//! @agents-index Integration test: a full record and the envelope hold only allowlisted fields, and the routes work with only the days table.

#[path = "common/days.rs"]
mod days;

use agent_loupe::api::{ApiState, router};
use agent_loupe::day::allowlist;
use agent_loupe::day::rebuild::rebuild;
use agent_loupe::day::state::Review;
use std::sync::{Arc, Mutex};

/// Serves only the API router over `conn` on a free port; returns the base URL.
async fn serve(conn: rusqlite::Connection) -> String {
    let state = Arc::new(ApiState {
        conn: Mutex::new(conn),
        tz: chrono_tz::UTC,
        subscriptions: vec![agent_loupe::subscription::config::Subscription {
            name: "claude_max".into(),
            usd_per_month: 200.into(),
        }],
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    format!("http://{addr}")
}

#[tokio::test(flavor = "multi_thread")]
async fn record_fields_within_allowlist() {
    let (dir, c) = days::db();
    for (agent, model) in [("claude-code", "a"), ("pi", "b"), ("codex", "c")] {
        days::simple(&c, "2026-10-10T10:00:00.000Z", agent);
        let s = days::session(&c, agent, "2026-10-10T11:00:00.000Z", false);
        days::call(
            &c,
            &days::Call {
                at: "2026-10-10T11:00:00.000Z",
                agent,
                model,
                session: s,
                tokens: [1, 2, 3, 4, 5],
                cost: None,
            },
        );
    }
    agent_loupe::day::overrides::set_summary(&c, "2026-10-10", "A day.", "t").unwrap();
    let ctx = days::ctx(Review::Auto, ctx_subs());
    let b = rebuild(&c, &ctx, days::date("2026-10-10"), "t", false, false).unwrap();
    let v: serde_json::Value = serde_json::from_str(&b.record.json()).unwrap();
    assert!(allowlist::violations(&v).is_empty(), "{v}");
    let mut bad = v.clone();
    bad["account_count"] = 1.into();
    assert_eq!(allowlist::violations(&bad), ["account_count"]);
    drop(c);
    let c = agent_loupe::store::open::open(dir.path()).unwrap();
    let base = serve(c).await;
    let body: serde_json::Value =
        reqwest::get(format!("{base}/v1/days?from=2026-10-01&to=2026-10-31"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    assert_eq!(body["days"].as_array().unwrap().len(), 1);
    assert!(allowlist::envelope_violations(&body).is_empty(), "{body}");
    // 31 days of October at 200 per month.
    assert_eq!(body["subscription_cost_usd"], 200);
}

fn ctx_subs() -> Vec<agent_loupe::subscription::config::Subscription> {
    vec![agent_loupe::subscription::config::Subscription {
        name: "claude_max".into(),
        usd_per_month: 200.into(),
    }]
}

#[tokio::test(flavor = "multi_thread")]
async fn api_routes_read_only_days_table() {
    // A store that holds the days table and no other table: a read of another table fails.
    let dir = tempfile::tempdir().unwrap();
    let c = rusqlite::Connection::open(dir.path().join("only-days.db")).unwrap();
    c.execute_batch(include_str!("../migrations/0014_days.sql"))
        .unwrap();
    c.execute(
        "INSERT INTO days (day, record, content_hash, revision, built_at, state, published_at) VALUES ('2026-10-10', '{\"day\":\"2026-10-10\",\"detail\":{\"by_agent\":{},\"by_model\":{},\"github\":{\"failed_tokens\":1}}}', 'h', 1, 't', 'published', 't')",
        [],
    )
    .unwrap();
    let base = serve(c).await;
    let client = reqwest::Client::new();
    for (method, path, want) in [
        ("GET", "/v1/days", 200),
        ("GET", "/v1/days?from=2026-10-01&to=2026-10-31", 200),
        ("GET", "/v1/days/2026-10-10", 200),
        ("GET", "/v1/days/2026-10-11", 404),
        ("GET", "/v1/days?from=bad", 400),
        ("OPTIONS", "/v1/days", 204),
        ("POST", "/v1/days", 405),
    ] {
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        let r = client
            .request(m, format!("{base}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), want, "{method} {path}");
    }
    // The allowlist holds github and failed_tokens, and nothing else under detail.github.
    let body: serde_json::Value = client
        .get(format!("{base}/v1/days"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["days"][0]["detail"]["github"]["failed_tokens"], 1);
    assert!(allowlist::envelope_violations(&body).is_empty(), "{body}");
    let bad = serde_json::json!({"detail": {"github": {"failed_tokens": 1, "labels": ["A"]}}});
    assert_eq!(allowlist::violations(&bad), ["detail.github.labels"]);
}
