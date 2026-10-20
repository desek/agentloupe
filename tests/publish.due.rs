//! @agents-index Integration test: a day is not due at 00:59 local on the next date and is due at 01:00.

#[path = "common/days.rs"]
mod days;

use agent_loupe::publish::due::is_due;

fn at() -> chrono::NaiveTime {
    chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap()
}

#[test]
fn day_not_due_before_publish_at() {
    let tz: chrono_tz::Tz = "Europe/Stockholm".parse().unwrap();
    // 00:59 local on 2026-10-19 is 21:59 UTC on 2026-10-18.
    let now = days::ms("2026-10-18T21:59:00Z");
    assert!(!is_due(days::date("2026-10-18"), now, tz, at()));
}

#[test]
fn day_due_at_publish_at() {
    let tz: chrono_tz::Tz = "Europe/Stockholm".parse().unwrap();
    let now = days::ms("2026-10-18T22:00:00Z");
    assert!(is_due(days::date("2026-10-18"), now, tz, at()));
}
