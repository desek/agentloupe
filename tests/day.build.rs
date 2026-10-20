//! @agents-index Integration test: build_day sums, cost rounding, unpriced calls, PR fields null without token, sessions per day, subscriptions, overrides, shape.

#[path = "common/days.rs"]
mod days;

use agent_loupe::day::build::build_day;
use agent_loupe::day::rebuild::rebuild;
use agent_loupe::day::state::Review;
use agent_loupe::subscription::config::Subscription;
use days::{Call, call, date, session};
use rust_decimal::Decimal;

const D: &str = "2026-10-18";
const AT: &str = "2026-10-18T10:00:00.000Z";

fn json(c: &rusqlite::Connection, day: &str) -> serde_json::Value {
    let r = build_day(c, date(day), chrono_tz::UTC, &[], &[]).unwrap();
    serde_json::from_str(&r.json()).unwrap()
}

fn one(c: &rusqlite::Connection, tokens: [i64; 5], cost: Option<&str>, model: &str) {
    let s = session(c, "claude-code", AT, false);
    call(
        c,
        &Call {
            at: AT,
            agent: "claude-code",
            model,
            session: s,
            tokens,
            cost,
        },
    );
}

#[test]
fn record_matches_readme_example_sums() {
    let (_d, c) = days::db();
    one(&c, [21000, 1250000, 12200, 0, 48000], Some("1"), "m");
    let v = json(&c, D);
    assert_eq!(v["input_total"], 1283200);
    assert_eq!(v["input"], 21000);
    assert_eq!(v["cache_read"], 1250000);
    assert_eq!(v["cache_write"], 12200);
    assert_eq!(v["output"], 48000);
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), agent_loupe::day::allowlist::RECORD.len());
}

#[test]
fn cache_write_is_sum_of_both_columns() {
    let (_d, c) = days::db();
    one(&c, [0, 0, 100, 50, 0], Some("0"), "m");
    assert_eq!(json(&c, D)["cache_write"], 150);
}

#[test]
fn cost_rounds_half_even_to_four_digits() {
    let (_d, c) = days::db();
    for cost in ["0.100000000025", "0.100000000025", "0.000050000000"] {
        one(&c, [1, 0, 0, 0, 1], Some(cost), "m");
    }
    // The exact sum is 0.20005000005; 4 digits give 0.2001. A half case: 0.00005 rounds to 0.0000.
    let r = build_day(&c, date(D), chrono_tz::UTC, &[], &[]).unwrap();
    assert_eq!(r.cost_usd, Decimal::new(2001, 4));
    assert_eq!(json(&c, D)["cost_usd"], 0.2001);
    assert_eq!(
        agent_loupe::day::sum::round4(Decimal::new(5, 5)),
        Decimal::ZERO
    );
    assert_eq!(
        agent_loupe::day::sum::round4(Decimal::new(15, 5)),
        Decimal::new(2, 4)
    );
}

#[test]
fn unpriced_calls_counted_and_excluded_from_cost() {
    let (_d, c) = days::db();
    one(&c, [1, 0, 0, 0, 1], Some("0.5"), "m");
    one(&c, [1, 0, 0, 0, 1], None, "m");
    let v = json(&c, D);
    assert_eq!(v["unpriced_calls"], 1);
    assert_eq!(v["cost_usd"], 0.5);
    assert_eq!(v["calls"], 2);
}

#[test]
fn pr_fields_null_without_token_numbers_with_token() {
    let (_d, c) = days::db();
    one(&c, [1, 0, 0, 0, 1], None, "m");
    let v = json(&c, D);
    for k in ["prs_opened", "prs_merged", "lines_added", "lines_removed"] {
        assert!(v.as_object().unwrap().contains_key(k));
        assert!(v[k].is_null(), "{k}");
    }
    assert!(v["detail"].get("github").is_none());
    assert_eq!(v["cost_usd"], 0);
    c.execute_batch(&format!(
        "INSERT INTO github_prs (pr_hash, opened_day, merged_day, additions, deletions, updated_at) VALUES ('h1', '{D}', '{D}', 7, 2, 't'); \
         INSERT INTO github_fetches (day, label, status, cause, fetched_at) VALUES ('{D}', 'A', 'ok', NULL, 't')"
    ))
    .unwrap();
    let r = build_day(&c, date(D), chrono_tz::UTC, &[], &["A".to_string()]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r.json()).unwrap();
    assert_eq!(
        [
            &v["prs_opened"],
            &v["prs_merged"],
            &v["lines_added"],
            &v["lines_removed"]
        ],
        [1, 1, 7, 2]
    );
    assert_eq!(v["detail"]["github"]["failed_tokens"], 0);
}

#[test]
fn session_counts_on_each_day_with_a_call() {
    let (_d, c) = days::db();
    let s = session(&c, "pi", "2026-10-18T23:50:00.000Z", false);
    for at in ["2026-10-18T23:50:00.000Z", "2026-10-19T00:10:00.000Z"] {
        call(
            &c,
            &Call {
                at,
                agent: "pi",
                model: "m",
                session: s,
                tokens: [1, 0, 0, 0, 1],
                cost: None,
            },
        );
    }
    assert_eq!(json(&c, "2026-10-18")["sessions"], 1);
    assert_eq!(json(&c, "2026-10-19")["sessions"], 1);
}

#[test]
fn subscription_cost_is_prorated() {
    let (_d, c) = days::db();
    let subs = vec![Subscription {
        name: "claude_max".into(),
        usd_per_month: Decimal::from(200),
    }];
    let r = build_day(&c, date(D), chrono_tz::UTC, &subs, &[]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&r.json()).unwrap();
    assert_eq!(v["subscription_cost_usd"], 6.4516);
    assert!(!r.json().contains("claude_max"));
    assert_eq!(json(&c, D)["subscription_cost_usd"], 0);
}

#[test]
fn override_summary_survives_rebuild() {
    let (_d, c) = days::db();
    one(&c, [1, 0, 0, 0, 1], None, "m");
    c.execute(
        "INSERT INTO day_summaries (day, status, line, session_lines, built_at) VALUES (?1, 'done', 'model line', 1, 'x')",
        [D],
    )
    .unwrap();
    agent_loupe::day::overrides::set_summary(&c, D, "edited", "t").unwrap();
    let ctx = days::ctx(Review::Auto, vec![]);
    rebuild(&c, &ctx, date(D), "t1", false, false).unwrap();
    one(&c, [1, 0, 0, 0, 1], None, "m");
    let b = rebuild(&c, &ctx, date(D), "t2", false, false).unwrap();
    assert_eq!(b.revision, 2);
    assert_eq!(days::record(&c, D).unwrap()["summary"], "edited");
}

#[test]
fn by_model_has_no_sessions_field() {
    let (_d, c) = days::db();
    one(&c, [1, 0, 0, 0, 1], None, "model-b");
    one(&c, [1, 0, 0, 0, 1], None, "model-a");
    let v = json(&c, D);
    let m = v["detail"]["by_model"].as_object().unwrap();
    assert_eq!(m.len(), 2);
    for (_, x) in m {
        assert!(x.get("sessions").is_none());
    }
    assert_eq!(v["detail"]["by_agent"]["claude-code"]["sessions"], 2);
}

#[test]
fn record_bytes_are_deterministic() {
    let hash = |order: &[(&str, &str)]| {
        let (_d, c) = days::db();
        for (agent, model) in order {
            let s = session(&c, agent, AT, false);
            call(
                &c,
                &Call {
                    at: AT,
                    agent,
                    model,
                    session: s,
                    tokens: [1, 2, 3, 4, 5],
                    cost: Some("0.1"),
                },
            );
        }
        let r = build_day(&c, date(D), chrono_tz::UTC, &[], &[]).unwrap();
        agent_loupe::day::record::content_hash(&r.json())
    };
    let a = hash(&[("pi", "z"), ("claude-code", "a"), ("codex", "m")]);
    let b = hash(&[("codex", "m"), ("pi", "z"), ("claude-code", "a")]);
    assert_eq!(a, b);
}

#[test]
fn summary_not_ready_requeues_day() {
    let (_d, c) = days::db();
    let s = session(&c, "pi", AT, true);
    call(
        &c,
        &Call {
            at: AT,
            agent: "pi",
            model: "m",
            session: s,
            tokens: [1, 0, 0, 0, 1],
            cost: None,
        },
    );
    let ctx = days::ctx(Review::Auto, vec![]);
    let b = rebuild(&c, &ctx, date(D), "t", true, false).unwrap();
    assert!(b.not_ready);
    assert!(days::record(&c, D).unwrap()["summary"].is_null());
    assert_eq!(
        days::text(&c, "SELECT reason FROM agg_queue WHERE day = '2026-10-18'").unwrap(),
        "summary_not_ready"
    );
}
