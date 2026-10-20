//! @agents-index Integration test of the GitHub fields of a day: null with no token and no request, lines on the merge day, failed tokens with no label name.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_github.rs"]
mod fake_github;

use agent_loupe::config::secret::Secret;
use agent_loupe::day::build::build_day;
use agent_loupe::github::Settings;
use agent_loupe::github::fetch::fetch_all;

fn settings(url: &str, tokens: &[(&str, &str)]) -> Settings {
    Settings {
        api_url: url.into(),
        account_key: [2; 32],
        tokens: tokens
            .iter()
            .map(|(l, t)| (l.to_string(), Secret::new(t.to_string())))
            .collect(),
    }
}

fn json(c: &rusqlite::Connection, day: &str, labels: &[String]) -> serde_json::Value {
    let r = build_day(c, days::date(day), chrono_tz::UTC, &[], labels).unwrap();
    serde_json::from_str(&r.json()).unwrap()
}

#[test]
fn fields_null_without_token() {
    let mut w = fake_github::world("tok", "u");
    w.prs = vec![fake_github::pr("u", 1, "2026-10-18", Some("2026-10-18"))];
    let f = fake_github::start(w);
    let (dir, c) = days::db();
    days::simple(&c, "2026-10-18T10:00:00.000Z", "pi");
    let out = days::loupe(
        dir.path(),
        &["aggregate", "--date", "2026-10-18"],
        &[("LOUPE_GITHUB_API_URL", &f.url)],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = days::record(&c, "2026-10-18").unwrap();
    for k in ["prs_opened", "prs_merged", "lines_added", "lines_removed"] {
        assert!(r[k].is_null(), "{k}");
    }
    assert!(r["detail"].get("github").is_none());
    assert_eq!(f.requests(), 0);
}

#[test]
fn lines_count_on_merge_day() {
    let mut w = fake_github::world("tok", "u");
    w.prs = vec![fake_github::pr("u", 1, "2026-10-17", Some("2026-10-18"))];
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let s = settings(&f.url, &[("A", "tok")]);
    for d in ["2026-10-17", "2026-10-18"] {
        fetch_all(&c, &s, days::date(d), chrono_tz::UTC, "t").unwrap();
    }
    let one = json(&c, "2026-10-17", &s.labels());
    assert_eq!(one["prs_opened"], 1);
    assert_eq!(one["prs_merged"], 0);
    assert_eq!(one["lines_added"], 0);
    let two = json(&c, "2026-10-18", &s.labels());
    assert_eq!(two["prs_opened"], 0);
    assert_eq!(two["prs_merged"], 1);
    assert_eq!(two["lines_added"], 10);
    assert_eq!(two["lines_removed"], 3);
    assert_eq!(two["detail"]["github"]["failed_tokens"], 0);
}

#[test]
fn failed_tokens_in_detail() {
    let mut w = fake_github::world("tb", "u");
    w.user_status.insert("ta".into(), 401);
    let f = fake_github::start(w);
    let (_d, c) = days::db();
    let s = settings(&f.url, &[("ALPHALABEL", "ta"), ("BETALABEL", "tb")]);
    fetch_all(&c, &s, days::date("2026-10-18"), chrono_tz::UTC, "t").unwrap();
    let r = json(&c, "2026-10-18", &s.labels());
    assert_eq!(r["detail"]["github"]["failed_tokens"], 1);
    let text = r.to_string().to_lowercase();
    assert!(
        !text.contains("alphalabel") && !text.contains("betalabel"),
        "{text}"
    );
    assert!(agent_loupe::day::allowlist::violations(&r).is_empty());
}
