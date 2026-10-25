//! @agents-index Integration test of one GitHub fetch against the fake: pages, the search limit, idempotence, de-duplication, failures, retry, isolation.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_github.rs"]
mod fake_github;

use agent_loupe::config::secret::Secret;
use agent_loupe::github::Settings;
use agent_loupe::github::fetch::{fetch_all, fetch_label};
use fake_github::{Fake, pr, world};

const D: &str = "2026-10-18";

fn settings(f: &Fake, tokens: &[(&str, &str)]) -> Settings {
    Settings {
        api_url: f.url.clone(),
        account_key: [1; 32],
        tokens: tokens
            .iter()
            .map(|(l, t)| (l.to_string(), Secret::new(t.to_string())))
            .collect(),
    }
}

fn one(c: &rusqlite::Connection, s: &Settings) -> agent_loupe::github::fetch::Fetched {
    let (l, t) = &s.tokens[0];
    fetch_label(c, s, l, t, days::date(D), chrono_tz::UTC, "t").unwrap()
}

fn rows(c: &rusqlite::Connection) -> Vec<String> {
    c.prepare("SELECT pr_hash || '|' || ifnull(opened_day,'') || '|' || ifnull(merged_day,'') || '|' || ifnull(additions,'') || '|' || ifnull(deletions,'') || '|' || updated_at FROM github_prs ORDER BY pr_hash")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn fetch_follows_all_pages() {
    let mut w = world("tok", "u");
    w.prs = (1..=230).map(|n| pr("u", n, D, None)).collect();
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let r = one(&c, &settings(&f, &[("A", "tok")]));
    assert!(r.ok, "{r:?}");
    assert_eq!(
        days::scalar(&c, "SELECT count(DISTINCT pr_hash) FROM github_prs"),
        230
    );
    assert!(f.log().iter().any(|l| l.contains("page=3")));
}

#[test]
fn fetch_fails_over_search_limit() {
    let mut w = world("tok", "u");
    w.prs = vec![pr("u", 1, D, Some(D))];
    w.total_count = Some(1001);
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let r = one(&c, &settings(&f, &[("A", "tok")]));
    assert_eq!(r.cause.as_deref(), Some("search_limit"));
    assert_eq!(
        days::text(&c, "SELECT status FROM github_fetches").as_deref(),
        Some("failed")
    );
    assert_eq!(
        days::text(&c, "SELECT cause FROM github_fetches").as_deref(),
        Some("search_limit")
    );
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_prs"), 0);
}

#[test]
fn fetch_is_idempotent() {
    let mut w = world("tok", "u");
    w.prs = vec![pr("u", 1, D, Some(D)), pr("u", 2, D, None)];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let s = settings(&f, &[("A", "tok")]);
    assert!(one(&c, &s).changed);
    let first = rows(&c);
    let again = one(&c, &s);
    assert!(again.ok && !again.changed);
    assert_eq!(rows(&c), first);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_pr_labels"), 2);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_fetches"), 1);
}

#[test]
fn two_labels_same_pr_count_once() {
    let mut w = world("ta", "u");
    w.logins.insert("tb".into(), "u".into());
    w.prs = vec![pr("u", 5, D, Some(D))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let s = settings(&f, &[("A", "ta"), ("B", "tb")]);
    fetch_all(&c, &s, days::date(D), chrono_tz::UTC, "t").unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_prs"), 1);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_pr_labels"), 2);
    let r = agent_loupe::day::build::build_day(&c, days::date(D), chrono_tz::UTC, &[], &s.labels())
        .unwrap();
    assert_eq!(r.prs_merged, Some(1));
    assert_eq!(r.lines_added, Some(10));
}

#[test]
fn unauthorized_token_is_recorded() {
    let mut w = world("tok", "u");
    w.user_status.insert("tok".into(), 401);
    let f = fake_github::start(w);
    let (dir, c) = days::db();
    let out = days::loupe(
        dir.path(),
        &["aggregate", "--date", D],
        &[
            ("LOUPE_GITHUB_TOKEN_ALPHA", "tok"),
            ("LOUPE_GITHUB_API_URL", &f.url),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        days::text(&c, "SELECT status FROM github_fetches").as_deref(),
        Some("failed")
    );
    assert_eq!(
        days::text(&c, "SELECT cause FROM github_fetches").as_deref(),
        Some("unauthorized")
    );
    let err = String::from_utf8_lossy(&out.stderr);
    let line = err
        .lines()
        .find(|l| l.contains("\"cause\":\"unauthorized\""))
        .expect(&err);
    assert!(line.contains("\"WARN\"") || line.contains("warn"), "{line}");
    assert!(line.contains("\"github_label\":\"ALPHA\""), "{line}");
    assert!(line.contains(&format!("\"day\":\"{D}\"")), "{line}");
    assert!(
        line.contains("loupe aggregate --date")
            && line.contains("docs/guide/github.md, section GitHub"),
        "{line}"
    );
}

#[test]
fn rate_limit_retries_then_fails() {
    let mut w = world("tok", "u");
    w.rate_limited = 4;
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let r = one(&c, &settings(&f, &[("A", "tok")]));
    assert_eq!(r.cause.as_deref(), Some("rate_limited"));
    assert_eq!(f.requests(), 4);
}

#[test]
fn one_failed_label_does_not_stop_others() {
    let mut w = world("ta", "u");
    w.logins.insert("tb".into(), "u".into());
    w.user_status.insert("ta".into(), 401);
    w.prs = vec![pr("u", 9, D, Some(D))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let s = settings(&f, &[("A", "ta"), ("B", "tb")]);
    let r = fetch_all(&c, &s, days::date(D), chrono_tz::UTC, "t").unwrap();
    assert!(!r[0].ok && r[1].ok);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_prs"), 1);
    let mut ctx = days::ctx(agent_loupe::day::state::Review::Auto, vec![]);
    ctx.github_labels = s.labels();
    agent_loupe::day::rebuild::rebuild(&c, &ctx, days::date(D), "t", true, false).unwrap();
    let rec = days::record(&c, D).unwrap();
    assert_eq!(rec["prs_merged"], 1);
    assert_eq!(rec["detail"]["github"]["failed_tokens"], 1);
}
