//! @agents-index Integration test: the day of an instant uses LOUPE_TZ, and the day of the time change in autumn has 25 hours.

#[path = "common/days.rs"]
mod days;

use agent_loupe::day::boundary;

#[test]
fn day_of_utc_instant_uses_local_zone() {
    let tz: chrono_tz::Tz = "Europe/Stockholm".parse().unwrap();
    let d = boundary::day_of(days::ms("2026-10-17T22:30:00Z"), tz);
    assert_eq!(d.to_string(), "2026-10-18");
}

#[test]
fn dst_day_has_25_hours() {
    let tz: chrono_tz::Tz = "Europe/Stockholm".parse().unwrap();
    assert_eq!(boundary::span_hours(days::date("2026-10-25"), tz), 25);
    assert_eq!(boundary::span_hours(days::date("2026-10-24"), tz), 24);
    assert_eq!(boundary::span_hours(days::date("2026-03-29"), tz), 23);
}
