//! @agents-index Integration tests of the live poller of the role public with a set clock: snapshot source, differences, stale rule, day change, and delay.

#[path = "common/days.rs"]
mod days;
#[path = "common/live.rs"]
mod live;

use agent_loupe::live::batch::Pending;
use agent_loupe::live::event::{ALLOWLIST, Event, Snapshot, Tokens};
use agent_loupe::live::hub::Hub;
use agent_loupe::live::poll::{Poller, run};
use agent_loupe::live::route::{LiveState, Source, handle};
use agent_loupe::mirror::{self, live::Row};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const T0: i64 = 1_790_000_000_000;

fn row(date: &str, calls: i64, segments: i64, ended: i64, in_flight: i64) -> Row {
    Row {
        snapshot: Snapshot {
            date: date.into(),
            calls,
            tokens: Tokens {
                input: calls * 10,
                output: calls * 2,
                cache_read: calls,
                cache_write: 0,
                reasoning: 0,
            },
            cost_usd: Decimal::new(calls, 2),
            segments_open: segments,
            in_flight,
        },
        ended,
    }
}

/// A mirror with no row, its writer, and a poller on a read-only connection.
fn setup(
    hub: &Hub,
) -> (
    tempfile::TempDir,
    rusqlite::Connection,
    Poller,
    Arc<AtomicUsize>,
) {
    let d = tempfile::tempdir().unwrap();
    let w = mirror::schema::open(d.path()).unwrap();
    let checks = Arc::new(AtomicUsize::new(0));
    let c = checks.clone();
    let p = Poller::new(
        mirror::open::open(d.path()).unwrap(),
        hub.clone(),
        Box::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    );
    (d, w, p, checks)
}

/// Each frame has the envelope and only the keys of the allowlist of its kind.
fn allowed(frame: &str) -> serde_json::Value {
    let f: serde_json::Value = serde_json::from_str(frame).unwrap();
    assert_eq!(f["v"], 1);
    let kind = f["kind"].as_str().unwrap();
    let (_, keys) = ALLOWLIST.iter().find(|(k, _)| *k == kind).unwrap();
    let mut want: Vec<&str> = keys.to_vec();
    want.sort();
    assert_eq!(live::keys(&f), want, "{kind}");
    f
}

fn frames(events: &[Event]) -> Vec<serde_json::Value> {
    events.iter().map(|e| allowed(&e.frame())).collect()
}

#[test]
fn calls_segments_and_exchanges_differences() {
    let hub = Hub::new(16);
    let (_d, mut w, mut p, checks) = setup(&hub);
    assert!(p.pass(T0).is_empty(), "no row, no event");
    mirror::live::write(&mut w, &row("2026-09-21", 2, 1, 5, 1), T0).unwrap();
    assert!(p.pass(T0 + 1_000).is_empty(), "the first row is the base");
    assert_eq!(hub.in_flight(), 1);
    mirror::live::write(&mut w, &row("2026-09-21", 5, 2, 8, 3), T0 + 1_500).unwrap();
    let f = frames(&p.pass(T0 + 2_000));
    assert_eq!(f.len(), 3, "{f:?}");
    assert_eq!(f[0]["kind"], "calls.batch");
    assert_eq!(f[0]["data"]["ts"], T0 + 2_000);
    assert_eq!(f[0]["data"]["calls"], 3);
    assert_eq!(f[0]["data"]["input"], 30);
    assert_eq!(f[0]["data"]["output"], 6);
    assert_eq!(f[0]["data"]["cost_usd"], "0.03");
    assert_eq!(f[1]["kind"], "segments.open");
    assert_eq!(f[1]["data"]["count"], 2);
    assert_eq!(f[2]["kind"], "exchanges.batch");
    assert_eq!(
        (
            &f[2]["data"]["ended"],
            &f[2]["data"]["in_flight"],
            &f[2]["data"]["started"]
        ),
        (&3.into(), &3.into(), &5.into())
    );
    assert_eq!(hub.in_flight(), 3);
    assert!(p.pass(T0 + 3_000).is_empty(), "no change, no event");
    assert_eq!(
        checks.load(Ordering::SeqCst),
        4,
        "the check runs at each pass"
    );
}

#[test]
fn stale_row_gives_in_flight_zero() {
    let hub = Hub::new(16);
    let (_d, mut w, mut p, _) = setup(&hub);
    mirror::live::write(&mut w, &row("2026-09-21", 1, 0, 0, 2), T0).unwrap();
    p.pass(T0 + 1_000);
    assert_eq!(hub.in_flight(), 2);
    let f = frames(&p.pass(T0 + 16_000));
    assert_eq!(f.len(), 1);
    assert_eq!(f[0]["kind"], "exchanges.batch");
    assert_eq!(f[0]["data"]["in_flight"], 0);
    assert_eq!(f[0]["data"]["started"], 0, "never below zero");
    assert_eq!(hub.in_flight(), 0);
}

#[test]
fn day_change_sends_a_snapshot_and_no_difference() {
    let hub = Hub::new(16);
    let (_d, mut w, mut p, _) = setup(&hub);
    mirror::live::write(&mut w, &row("2026-09-21", 9, 2, 4, 0), T0).unwrap();
    p.pass(T0);
    mirror::live::write(&mut w, &row("2026-09-22", 1, 1, 6, 0), T0 + 1_000).unwrap();
    let f = frames(&p.pass(T0 + 1_000));
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0]["kind"], "snapshot");
    assert_eq!(f[0]["data"]["date"], "2026-09-22");
    assert_eq!(f[0]["data"]["calls"], 1);
}

#[test]
fn counters_down_on_the_same_date_give_no_negative_difference() {
    let hub = Hub::new(16);
    let (_d, mut w, mut p, _) = setup(&hub);
    mirror::live::write(&mut w, &row("2026-09-21", 9, 2, 40, 0), T0).unwrap();
    p.pass(T0);
    mirror::live::write(&mut w, &row("2026-09-21", 9, 2, 0, 0), T0 + 1_000).unwrap();
    assert!(p.pass(T0 + 1_000).is_empty(), "ended went down");
    mirror::live::write(&mut w, &row("2026-09-21", 10, 2, 1, 0), T0 + 2_000).unwrap();
    let f = frames(&p.pass(T0 + 2_000));
    assert_eq!(f[0]["data"]["calls"], 1, "the lower row is the new base");
    assert_eq!(f[1]["data"]["ended"], 1);
}

#[test]
fn coalescing_takes_poller_events() {
    let ts = T0;
    let mut p = Pending::default();
    let mk = |e: &Event| e.clone();
    let calls = Event::CallsBatch {
        ts,
        batch: agent_loupe::live::event::CallsBatch {
            calls: 2,
            tokens: Tokens::default(),
            cost_usd: Some(Decimal::new(1, 2)),
        },
    };
    let ex = Event::ExchangesBatch {
        ts,
        batch: agent_loupe::live::event::ExchangesBatch {
            started: 2,
            ended: 1,
            in_flight: 9,
        },
    };
    p.add(&mk(&calls));
    p.add(&mk(&calls));
    p.add(&ex);
    p.add(&ex);
    let x = allowed(&p.take(ts, 4).unwrap());
    assert_eq!(
        (
            &x["data"]["started"],
            &x["data"]["ended"],
            &x["data"]["in_flight"]
        ),
        (&4.into(), &2.into(), &4.into())
    );
    let c = allowed(&p.take(ts, 4).unwrap());
    assert_eq!(c["data"]["calls"], 4);
    assert_eq!(c["data"]["cost_usd"], "0.02");
    p.add(&calls);
    p.add(&Event::Snapshot(row("2026-09-22", 0, 0, 0, 0).snapshot));
    let s = allowed(&p.take(ts, 0).unwrap());
    assert_eq!(s["kind"], "snapshot");
    assert!(p.is_empty(), "the snapshot dropped the pending sum");
}

async fn serve(d: &std::path::Path, hub: Hub) -> u16 {
    let state = LiveState::with_source(
        hub,
        live::config(10, 64),
        chrono_tz::UTC,
        mirror::open::open(d).unwrap(),
        agent_loupe::clock::system(),
        Source::Mirror,
    );
    let app = axum::Router::new().fallback(move |req| {
        let s = state.clone();
        async move { handle(s, req).await }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn route_snapshot_from_the_row_absent_row_and_stale_row() {
    let hub = Hub::new(16);
    let (d, mut w, _p, _) = setup(&hub);
    let port = serve(d.path(), hub).await;
    let (mut ws, _) = live::open(port, "/live", &[]).await.unwrap();
    let f = allowed(&live::frame(&mut ws, 5).await.to_string());
    assert_eq!(f["data"]["calls"], 0);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    assert_eq!(f["data"]["date"], today.as_str());
    let now = chrono::Utc::now().timestamp_millis();
    mirror::live::write(&mut w, &row("2026-09-21", 7, 3, 1, 2), now).unwrap();
    let (mut ws, _) = live::open(port, "/live", &[]).await.unwrap();
    let f = live::frame(&mut ws, 5).await;
    assert_eq!(
        (
            &f["data"]["date"],
            &f["data"]["calls"],
            &f["data"]["segments_open"],
            &f["data"]["in_flight"]
        ),
        (&"2026-09-21".into(), &7.into(), &3.into(), &2.into())
    );
    mirror::live::write(&mut w, &row("2026-09-21", 7, 3, 1, 2), now - 16_000).unwrap();
    let (mut ws, _) = live::open(port, "/live", &[]).await.unwrap();
    assert_eq!(live::frame(&mut ws, 5).await["data"]["in_flight"], 0);
}

struct Wall(Arc<agent_loupe::publish::clock::Fixed>);
impl agent_loupe::publish::clock::Clock for Wall {
    fn now_ms(&self) -> i64 {
        agent_loupe::publish::clock::Clock::now_ms(&*self.0)
    }
}

/// A call stored by the role full reaches a calls.batch within one mirror write interval, one
/// poll interval, and one frame interval of set-clock time.
#[tokio::test(flavor = "multi_thread")]
async fn a_stored_call_reaches_a_calls_batch_within_the_delay() {
    let (dir, c) = days::db();
    let t0 = days::ms("2026-10-01T12:00:00Z");
    let wall = Arc::new(agent_loupe::publish::clock::Fixed::new(t0));
    let mut task = mirror::worker::Task::new(
        agent_loupe::store::open::open(dir.path()).unwrap(),
        mirror::schema::open(dir.path()).unwrap(),
        chrono_tz::UTC,
        Hub::off(),
        Box::new(Wall(wall.clone())),
    );
    assert!(task.pass().live_written);
    let hub = Hub::new(16);
    let mut rx = hub.subscribe().unwrap();
    let poller = Poller::new(
        mirror::open::open(dir.path()).unwrap(),
        hub,
        Box::new(|_| {}),
    );
    let clock = agent_loupe::clock::Set::new();
    tokio::spawn(run(poller, clock.clone(), Arc::new(Wall(wall.clone()))));
    tokio::time::sleep(Duration::from_millis(200)).await;
    // The call commits right after a mirror write: the worst case of the write interval.
    days::simple(&c, "2026-10-01T11:00:00Z", "claude-code");
    wall.set(t0 + 1_000);
    assert!(task.pass().live_written);
    // The poll pass comes at most one interval after the write.
    wall.set(t0 + 2_000);
    clock.advance(Duration::from_secs(1));
    let e = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let f = allowed(&e.frame());
    assert_eq!(f["kind"], "calls.batch");
    assert_eq!(f["data"]["calls"], 1);
    let frame_gap = 1_000 / live::config(10, 64).max_rate as i64;
    let delay = f["data"]["ts"].as_i64().unwrap() + frame_gap - t0;
    assert!(delay <= 2_100, "delay {delay} ms");
}
