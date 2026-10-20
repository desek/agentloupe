//! @agents-index Integration test: a day with no activity gets no record, and a test activity source creates and updates a day with no call.

#[path = "common/days.rs"]
mod days;

use agent_loupe::day::activity::{self, Source};
use agent_loupe::day::state::Review;
use agent_loupe::publish::clock::Fixed;
use agent_loupe::publish::worker::{Settings, wake};
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::Connection;

/// A source that reports activity on one fixed day only.
struct OneDay(NaiveDate);

impl Source for OneDay {
    fn name(&self) -> &'static str {
        "test"
    }
    fn has_activity(&self, _: &Connection, day: NaiveDate, _: Tz) -> rusqlite::Result<bool> {
        Ok(day == self.0)
    }
    fn first_activity_day(&self, _: &Connection, _: Tz) -> rusqlite::Result<Option<NaiveDate>> {
        Ok(Some(self.0))
    }
}

fn settings(sources: Vec<Box<dyn Source>>) -> Settings {
    Settings {
        ctx: days::ctx(Review::Auto, vec![]),
        publish_at: chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
        sources,
    }
}

#[test]
fn day_without_activity_gets_no_record() {
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-10T10:00:00.000Z", "pi");
    agent_loupe::day::queue::put(&c, "2026-10-11", "call", "t").unwrap();
    let clock = Fixed::new(days::ms("2026-10-14T12:00:00Z"));
    wake(&c, &settings(activity::registered()), &clock).unwrap();
    assert_eq!(
        days::scalar(&c, "SELECT count(*) FROM days WHERE day = '2026-10-11'"),
        0
    );
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 1);
}

#[test]
fn source_activity_without_calls_publishes_day() {
    let (_d, c) = days::db();
    let src: Vec<Box<dyn Source>> = vec![Box::new(OneDay(days::date("2026-10-12")))];
    let clock = Fixed::new(days::ms("2026-10-14T12:00:00Z"));
    wake(&c, &settings(src), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 1);
    assert_eq!(days::record(&c, "2026-10-12").unwrap()["calls"], 0);
}

#[test]
fn source_queue_put_updates_published_day() {
    let (_d, c) = days::db();
    let day = days::date("2026-10-12");
    let clock = Fixed::new(days::ms("2026-10-14T12:00:00Z"));
    let make =
        || -> Vec<Box<dyn Source>> { vec![Box::new(OneDay(day)), Box::new(activity::Calls)] };
    wake(&c, &settings(make()), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 1);
    // New data of the day, then the source puts the day in the queue with its own name.
    days::simple(&c, "2026-10-12T10:00:00.000Z", "pi");
    agent_loupe::day::queue::put(&c, "2026-10-12", "test", "t").unwrap();
    wake(&c, &settings(make()), &clock).unwrap();
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 2);
}
