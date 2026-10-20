//! @agents-index Integration test: the parser worker prices each call in its own transaction, and loupe price backfill is idempotent and has a dry run.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;
use std::process::{Command, Output};

/// Sends one Anthropic SSE call of claude-haiku-5-5 through the fake upstream (provider anthropic), then parses it.
async fn one_parsed_call() -> Loupe {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_PARSE_INTERVAL_SECS", "86400")]);
    let body = serde_json::json!({"model":"claude-haiku-5-5","stream":true,
        "metadata":{"user_id":"user_00ff_account_acc-p_session_ses-p"},
        "messages":[{"role":"user","content":"hi"}]});
    let r = reqwest::Client::new()
        .post(l.url("/claude-code/fake/v1/messages"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.bytes().await.unwrap();
    l.wait_rows(1);
    let out = loupe::parse(l.data.path(), &["--once"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    l
}

type Cost = (Option<String>, Option<String>, Option<String>);

fn cost(l: &Loupe) -> Cost {
    l.db()
        .query_row(
            "SELECT cost_usd, price_commit, unpriced_reason FROM calls",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

fn backfill(l: &Loupe, args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_loupe"))
        .args(["price", "backfill"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", l.data.path())
        .env("LOUPE_UPSTREAMS_FILE", l.data.path().join("upstreams.toml"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn forget_price(l: &Loupe) {
    l.db()
        .execute(
            "UPDATE calls SET cost_usd = NULL, price_commit = NULL, unpriced_reason = NULL",
            [],
        )
        .unwrap();
}

/// 10*0.0000001 + 20*0.00000001 + 4*0.000000125 + 3*0.0000002 + 5*0.0000005, the counts of the fake upstream.
const EXPECTED: &str = "0.0000048";

#[tokio::test(flavor = "multi_thread")]
async fn worker_writes_cost_in_same_transaction() {
    let l = one_parsed_call().await;
    let (cost_usd, commit, reason) = cost(&l);
    assert_eq!(cost_usd.as_deref(), Some(EXPECTED));
    let commit = commit.unwrap();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(reason, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn backfill_twice_prices_nothing_new() {
    let l = one_parsed_call().await;
    forget_price(&l);
    let first = backfill(&l, &[]);
    assert_eq!(loupe::count(&first, "calls_priced"), 1);
    assert_eq!(loupe::count(&first, "calls_unpriced"), 0);
    assert!(String::from_utf8_lossy(&first.stdout).contains("price_commit: "));
    assert_eq!(cost(&l).0.as_deref(), Some(EXPECTED));
    let second = backfill(&l, &[]);
    assert_eq!(loupe::count(&second, "calls_priced"), 0);
    assert_eq!(loupe::count(&second, "calls_unpriced"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn backfill_dry_run_writes_nothing() {
    let l = one_parsed_call().await;
    forget_price(&l);
    let out = backfill(&l, &["--dry-run"]);
    assert_eq!(loupe::count(&out, "calls_priced"), 1);
    assert_eq!(cost(&l), (None, None, None));
}
