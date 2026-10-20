//! @agents-index Integration test: the upsert keeps the revision for equal content and adds one for new content, published_at stays.

#[path = "common/days.rs"]
mod days;

use agent_loupe::day::rebuild::rebuild;
use agent_loupe::day::state::Review;

const D: &str = "2026-10-18";

#[test]
fn same_content_keeps_revision() {
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-18T10:00:00.000Z", "pi");
    let ctx = days::ctx(Review::Auto, vec![]);
    rebuild(&c, &ctx, days::date(D), "t1", false, false).unwrap();
    let b = rebuild(&c, &ctx, days::date(D), "t2", false, false).unwrap();
    assert!(!b.written);
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 1);
    assert_eq!(days::text(&c, "SELECT built_at FROM days").unwrap(), "t1");
}

#[test]
fn changed_content_bumps_revision() {
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-18T10:00:00.000Z", "pi");
    let ctx = days::ctx(Review::Auto, vec![]);
    rebuild(&c, &ctx, days::date(D), "t1", false, false).unwrap();
    days::simple(&c, "2026-10-18T11:00:00.000Z", "pi");
    let b = rebuild(&c, &ctx, days::date(D), "t2", false, false).unwrap();
    assert!(b.written);
    assert_eq!(days::scalar(&c, "SELECT revision FROM days"), 2);
    assert_eq!(
        days::text(&c, "SELECT published_at FROM days").unwrap(),
        "t1"
    );
    assert_eq!(days::text(&c, "SELECT built_at FROM days").unwrap(), "t2");
    assert_eq!(
        days::text(&c, "SELECT state FROM days").unwrap(),
        "published"
    );
}
