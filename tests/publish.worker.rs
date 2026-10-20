//! @agents-index Integration test: the publisher catches up missed days oldest first, updates a published day late, and waits for a future day.

#[path = "common/days.rs"]
mod days;

use agent_loupe::day::activity;
use agent_loupe::day::state::Review;
use agent_loupe::publish::clock::Fixed;
use agent_loupe::publish::worker::{Settings, wake};

fn settings() -> Settings {
    Settings {
        ctx: days::ctx(Review::Auto, vec![]),
        publish_at: chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
        sources: activity::registered(),
        github: None,
    }
}

#[test]
fn catch_up_publishes_missed_days_in_order() {
    let (_d, c) = days::db();
    for d in ["2026-10-10", "2026-10-11", "2026-10-12"] {
        days::simple(&c, &format!("{d}T10:00:00.000Z"), "pi");
    }
    let clock = Fixed::new(days::ms("2026-10-15T12:00:00Z"));
    let n = wake(&c, &settings(), &clock).unwrap();
    assert_eq!(n.published, 3);
    let rows: Vec<(String, String)> = c
        .prepare("SELECT day, built_at FROM days ORDER BY rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let order: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(order, ["2026-10-10", "2026-10-11", "2026-10-12"]);
    // A second wake (a restart) repeats no work.
    assert_eq!(wake(&c, &settings(), &clock).unwrap().published, 0);
}

#[test]
fn late_session_updates_published_day() {
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-10T10:00:00.000Z", "pi");
    let clock = Fixed::new(days::ms("2026-10-11T02:00:00Z"));
    wake(&c, &settings(), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 1);
    // A segment of the day closes after the publish and queues the day.
    let s = days::simple(&c, "2026-10-10T23:30:00.000Z", "pi");
    agent_loupe::day::queue::put_session_days(&c, s, chrono_tz::UTC, "session_closed", "t")
        .unwrap();
    clock.set(days::ms("2026-10-11T03:00:00Z"));
    assert_eq!(wake(&c, &settings(), &clock).unwrap().updated, 1);
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 2);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM agg_queue"), 0);
    let r = days::record(&c, "2026-10-10").unwrap();
    assert_eq!(r["calls"], 2);
    assert!(r.get("revision").is_none());
}

#[test]
fn queued_future_day_waits() {
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-20T10:00:00.000Z", "pi");
    agent_loupe::day::queue::put(&c, "2026-10-20", "call", "t").unwrap();
    let clock = Fixed::new(days::ms("2026-10-20T23:00:00Z"));
    wake(&c, &settings(), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 0);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM agg_queue"), 1);
    clock.set(days::ms("2026-10-21T01:00:00Z"));
    wake(&c, &settings(), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 1);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM agg_queue"), 0);
}
