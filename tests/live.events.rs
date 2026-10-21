//! @agents-index Integration test of the live events: capture and parser sends, the allowlist of each kind, and the day rule of the snapshot.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/live.rs"]
mod live;
#[path = "common/loupe.rs"]
mod loupe;

use agent_loupe::live::event::{
    ALLOWLIST, CallsBatch, Event, ExchangesBatch, Snapshot, Tokens, heartbeat,
};
use live::{frame, keys, open, until};
use loupe::Loupe;
use std::time::Duration;

const FIVE: [&str; 5] = ["input", "output", "cache_read", "cache_write", "reasoning"];

fn allow(kind: &str) -> Vec<String> {
    let mut k: Vec<String> = ALLOWLIST
        .iter()
        .find(|(n, _)| *n == kind)
        .unwrap()
        .1
        .iter()
        .map(|s| s.to_string())
        .collect();
    k.sort();
    k
}

#[tokio::test(flavor = "multi_thread")]
async fn exchange_events_in_order() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    // The stream takes 400 ms, so the end is past the next free slot of the rate limit.
    let r = reqwest::get(l.url("/pi/fake/sse?chunks=2&delay_ms=200"))
        .await
        .unwrap();
    r.bytes().await.unwrap();
    let a = frame(&mut ws, 5).await;
    let b = frame(&mut ws, 5).await;
    assert_eq!(a["kind"], "exchange.start");
    assert_eq!(keys(&a), allow("exchange.start"));
    assert_eq!(b["kind"], "exchange.end");
    assert_eq!(keys(&b), allow("exchange.end"));
    assert_eq!(b["data"]["status"], 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn call_stored_after_parse() {
    let l = Loupe::start(
        fake::spawn().await,
        &[],
        &[("LOUPE_PARSE_INTERVAL_SECS", "1")],
    );
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    let body =
        serde_json::json!({"model":"claude-haiku-5-5","messages":[{"role":"user","content":"hi"}]});
    let r = reqwest::Client::new()
        .post(l.url("/pi/fake/v1/messages"))
        .json(&body)
        .send()
        .await
        .unwrap();
    r.bytes().await.unwrap();
    let (f, _) = until(&mut ws, "call.stored", 10).await;
    assert_eq!(keys(&f), allow("call.stored"));
    let row: (i64, i64, i64, i64) = l
        .db()
        .query_row(
            "SELECT input, output, cache_read, cache_write_5m + cache_write_1h FROM calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let d = &f["data"];
    assert_eq!(
        (
            d["input"].as_i64().unwrap(),
            d["output"].as_i64().unwrap(),
            d["cache_read"].as_i64().unwrap(),
            d["cache_write"].as_i64().unwrap()
        ),
        row
    );
    assert_eq!(d["reasoning"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn segments_open_after_parse() {
    let l = Loupe::start(
        fake::spawn().await,
        &[],
        &[("LOUPE_PARSE_INTERVAL_SECS", "1"), ("LOUPE_IDLE_SECS", "1")],
    );
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    let body =
        serde_json::json!({"model":"claude-haiku-5-5","messages":[{"role":"user","content":"hi"}]});
    let r = reqwest::Client::new()
        .post(l.url("/pi/fake/v1/messages"))
        .json(&body)
        .send()
        .await
        .unwrap();
    r.bytes().await.unwrap();
    let mut counts = Vec::new();
    while counts.last() != Some(&0) || counts.len() < 2 {
        let (f, _) = until(&mut ws, "segments.open", 15).await;
        assert_eq!(keys(&f), allow("segments.open"));
        let c = f["data"]["count"].as_i64().unwrap();
        if counts.last() != Some(&c) {
            counts.push(c);
        }
    }
    assert_eq!(counts[counts.len() - 2..], [1, 0]);
}

#[tokio::test(flavor = "multi_thread")]
async fn allowlist_per_kind() {
    // Each serializer writes exactly the public allowlist of its kind.
    let t = Tokens {
        input: 1,
        output: 2,
        cache_read: 3,
        cache_write: 4,
        reasoning: 0,
    };
    let frames = [
        Event::ExchangeStart { ts: 1 }.frame(),
        Event::ExchangeEnd { ts: 1, status: 200 }.frame(),
        Event::CallStored {
            ts: 1,
            tokens: t,
            cost_usd: None,
        }
        .frame(),
        Event::SegmentsOpen { ts: 1, count: 2 }.frame(),
        heartbeat(1),
        Snapshot::default().frame(),
        CallsBatch {
            calls: 2,
            tokens: t,
            cost_usd: None,
        }
        .frame(1),
        ExchangesBatch::default().frame(1),
    ];
    for f in &frames {
        let v: serde_json::Value = serde_json::from_str(f).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 3);
        assert_eq!(v["v"], 1);
        assert_eq!(keys(&v), allow(v["kind"].as_str().unwrap()), "{f}");
    }
    let snap: serde_json::Value = serde_json::from_str(&frames[5]).unwrap();
    let mut tk: Vec<&str> = snap["data"]["tokens"]
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    tk.sort();
    let mut five = FIVE.to_vec();
    five.sort();
    assert_eq!(tk, five);
    // End to end: seeded secrets in the token, the path, the query, the headers, and the body reach no frame.
    let data = tempfile::tempdir().unwrap();
    let token = loupe::create_token(data.path(), "secret-name-zq");
    let l = Loupe::start_in(
        data,
        fake::spawn().await,
        &[],
        &[
            ("LOUPE_PARSE_INTERVAL_SECS", "1"),
            ("LOUPE_LIVE_MAX_RATE", "1"),
        ],
    );
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    let mut seen = vec![frame(&mut ws, 5).await];
    let body = serde_json::json!({"model":"claude-haiku-5-5","metadata":{"user_id":"user_ab_account_acc-zq_session_ses-zq"},"messages":[{"role":"user","content":"prompt-secret-zq"}]});
    for _ in 0..3 {
        let r = reqwest::Client::new()
            .post(l.url(&format!(
                "/{token}/agent-zq/fake/v1/messages?q=query-secret-zq"
            )))
            .header("x-api-key", "sk-secret-zq")
            .json(&body)
            .send()
            .await
            .unwrap();
        r.bytes().await.unwrap();
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
    while tokio::time::Instant::now() < deadline {
        if let Some(tokio_tungstenite::tungstenite::Message::Text(t)) = live::next(&mut ws, 1).await
        {
            seen.push(serde_json::from_str(&t).unwrap());
        }
    }
    let all = serde_json::to_string(&seen).unwrap();
    for secret in [
        token.as_str(),
        "secret-name-zq",
        "agent-zq",
        "prompt-secret-zq",
        "query-secret-zq",
        "sk-secret-zq",
        "acc-zq",
        "ses-zq",
        "v1/messages",
        "claude-haiku",
        "127.0.0.1",
    ] {
        assert!(!all.contains(secret), "{secret} in {all}");
    }
    for f in &seen {
        assert_eq!(keys(f), allow(f["kind"].as_str().unwrap()));
    }
    assert!(
        seen.iter()
            .any(|f| f["kind"] == "calls.batch" || f["kind"] == "call.stored")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshot_matches_day_rules() {
    let (_d, c) = days::db();
    let tz: chrono_tz::Tz = "Europe/Stockholm".parse().unwrap();
    // 2026-10-20 in Stockholm (UTC+3) is 2026-10-19T21:00Z to 2026-10-20T21:00Z.
    for (at, cost) in [
        ("2026-10-19T20:59:59.000Z", "1"),
        ("2026-10-19T21:00:00.000Z", "0.25"),
        ("2026-10-20T20:59:59.999Z", "0.5"),
        ("2026-10-20T21:00:00.000Z", "2"),
    ] {
        let s = days::session(&c, "pi", at, false);
        days::call(
            &c,
            &days::Call {
                at,
                agent: "pi",
                model: "m",
                session: s,
                tokens: [1, 2, 3, 4, 5],
                cost: Some(cost),
            },
        );
    }
    days::session(&c, "pi", "2026-10-20T10:00:00.000Z", true);
    let s =
        agent_loupe::live::snapshot::read(&c, tz, days::ms("2026-10-20T12:00:00.000Z"), 0).unwrap();
    assert_eq!(s.date, "2026-10-20");
    assert_eq!(s.calls, 2);
    assert_eq!(
        s.tokens,
        Tokens {
            input: 2,
            cache_read: 4,
            cache_write: 14,
            output: 10,
            reasoning: 0
        }
    );
    assert_eq!(s.cost_usd.to_string(), "0.75");
    assert_eq!(s.segments_open, 1);
}
