//! @agents-index Integration test of the publisher with GitHub: a GitHub-only day gets a record, catch-up order, and the 7-day retry of a failed label.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_github.rs"]
mod fake_github;

use agent_loupe::config::secret::Secret;
use agent_loupe::day::activity;
use agent_loupe::day::state::Review;
use agent_loupe::publish::clock::Fixed;
use agent_loupe::publish::worker::{Settings, wake};

const NOW: &str = "2026-10-14T12:00:00Z";

fn settings(url: &str, tokens: &[(&str, &str)]) -> Settings {
    let g = agent_loupe::github::Settings {
        api_url: url.into(),
        account_key: [3; 32],
        tokens: tokens
            .iter()
            .map(|(l, t)| (l.to_string(), Secret::new(t.to_string())))
            .collect(),
    };
    let mut ctx = days::ctx(Review::Auto, vec![]);
    ctx.github_labels = g.labels();
    Settings {
        ctx,
        publish_at: chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
        sources: activity::registered(),
        github: Some(g),
    }
}

#[test]
fn github_only_day_gets_record() {
    let mut w = fake_github::world("tok", "u");
    w.prs = vec![fake_github::pr("u", 1, "2026-10-12", Some("2026-10-12"))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_prs"), 0);
    wake(
        &c,
        &settings(&f.url, &[("A", "tok")]),
        &Fixed::new(days::ms(NOW)),
    )
    .unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 1);
    let r = days::record(&c, "2026-10-12").unwrap();
    assert_eq!(r["calls"], 0);
    assert_eq!(r["prs_merged"], 1);
}

#[test]
fn day_without_calls_or_prs_gets_no_record() {
    let f = fake_github::start(fake_github::world("tok", "u"));
    let (_d, c) = days::db();
    wake(
        &c,
        &settings(&f.url, &[("A", "tok")]),
        &Fixed::new(days::ms(NOW)),
    )
    .unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 0);
    assert_eq!(
        days::scalar(
            &c,
            "SELECT count(*) FROM github_fetches WHERE day = '2026-10-13' AND status = 'ok'"
        ),
        1
    );
    // The lookback is 90 days, so the oldest fetched day is today minus 90.
    assert_eq!(
        days::text(&c, "SELECT min(day) FROM github_fetches").as_deref(),
        Some("2026-07-16")
    );
}

#[test]
fn catch_up_fetches_each_day_oldest_first() {
    let mut w = fake_github::world("tok", "u");
    w.prs = vec![fake_github::pr("u", 1, "2026-10-12", Some("2026-10-13"))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    for d in ["2026-10-11", "2026-10-12", "2026-10-13"] {
        days::simple(&c, &format!("{d}T10:00:00.000Z"), "pi");
    }
    wake(
        &c,
        &settings(&f.url, &[("A", "tok")]),
        &Fixed::new(days::ms(NOW)),
    )
    .unwrap();
    let created: Vec<String> = f
        .log()
        .iter()
        .filter_map(|l| l.split("created:").nth(1).map(|r| r[..10].to_string()))
        .collect();
    assert_eq!(created, ["2026-10-11", "2026-10-12", "2026-10-13"]);
    // Each first build already holds the GitHub counts.
    assert_eq!(days::record(&c, "2026-10-11").unwrap()["prs_opened"], 0);
    assert_eq!(days::record(&c, "2026-10-12").unwrap()["prs_opened"], 1);
    let last = days::record(&c, "2026-10-13").unwrap();
    assert_eq!(last["prs_merged"], 1);
    assert_eq!(days::scalar(&c, "SELECT max(revision) FROM days"), 1);
}

#[test]
fn retry_failed_label_within_7_days() {
    let mut w = fake_github::world("tok", "u");
    w.user_status.insert("tok".into(), 401);
    w.prs = vec![fake_github::pr("u", 1, "2026-10-13", Some("2026-10-13"))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    days::simple(&c, "2026-10-13T10:00:00.000Z", "pi");
    let s = settings(&f.url, &[("A", "tok")]);
    let clock = Fixed::new(days::ms(NOW));
    wake(&c, &s, &clock).unwrap();
    let r = days::record(&c, "2026-10-13").unwrap();
    assert_eq!(r["detail"]["github"]["failed_tokens"], 1);
    // The token is fixed; the retry of the next wake fetches the published day again.
    f.with(|w| w.user_status.clear());
    let g = s.github.as_ref().unwrap();
    agent_loupe::github::refetch::run(
        &c,
        g,
        chrono_tz::UTC,
        clock.0.load(std::sync::atomic::Ordering::SeqCst),
        "t2",
    )
    .unwrap();
    assert_eq!(
        agent_loupe::day::queue::reason(&c, "2026-10-13")
            .unwrap()
            .as_deref(),
        Some("github")
    );
    wake(&c, &s, &clock).unwrap();
    let r = days::record(&c, "2026-10-13").unwrap();
    assert_eq!(r["detail"]["github"]["failed_tokens"], 0);
    assert_eq!(r["prs_merged"], 1);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM agg_queue"), 0);
}
