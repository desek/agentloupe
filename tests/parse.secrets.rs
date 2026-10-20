//! @agents-index Integration test: no raw session ID, account UUID, cache key, or credential reaches the database or the log after a parse.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn no_raw_ids_in_db_or_logs() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_PARSE_INTERVAL_SECS", "1")]);
    let c = reqwest::Client::new();
    let bodies = [
        serde_json::json!({"model":"claude-haiku-5-5","metadata":{"user_id":"user_00ff_account_acct-uuid-4242_session_sess-meta-4343"},"messages":[]}),
        serde_json::json!({"model":"gpt-5","prompt_cache_key":"cache-key-4444","messages":[]}),
    ];
    for (path, body) in [
        ("/v1/messages", &bodies[0]),
        ("/v1/chat/completions", &bodies[1]),
    ] {
        let r = c
            .post(l.url(&format!("/pi/fake{path}")))
            .header("x-api-key", "sk-parse-4545")
            .header("x-session-id", "sess-header-4646")
            .json(body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        r.bytes().await.unwrap();
    }
    l.wait_rows(2);
    let out = loupe::parse(l.data.path(), &["--once"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let calls: i64 = l
        .db()
        .query_row("SELECT count(*) FROM calls", [], |r| r.get(0))
        .unwrap();
    assert_eq!(calls, 2);
    let db = l.db_bytes();
    let log = format!(
        "{}{}{}",
        l.log(),
        l.stderr(),
        String::from_utf8_lossy(&out.stderr)
    );
    for needle in [
        "acct-uuid-4242",
        "sess-meta-4343",
        "cache-key-4444",
        "sk-parse-4545",
        "sess-header-4646",
    ] {
        assert!(
            !loupe::contains(&db, needle.as_bytes()),
            "{needle} in the database"
        );
        assert!(!log.contains(needle), "{needle} in the log");
    }
}
