//! @agents-index Integration test: an SSE call through the proxy with a token becomes one call row with token, session, and account.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn proxied_sse_call_becomes_call_row() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let token = loupe::create_token(data.path(), "e2e");
    let l = Loupe::start_in(data, fake, &[], &[("LOUPE_PARSE_INTERVAL_SECS", "86400")]);
    let body = serde_json::json!({"model":"claude-haiku-5-5","stream":true,
        "metadata":{"user_id":"user_00ff_account_acc-e2e_session_ses-e2e"},
        "messages":[{"role":"user","content":"hi"}]});
    let r = reqwest::Client::new()
        .post(l.url(&format!("/{token}/claude-code/fake/v1/messages")))
        .header("authorization", "Bearer sk-e2e")
        .header("x-claude-code-session-id", "hint-e2e")
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
    assert_eq!(loupe::count(&out, "calls_added"), 1);
    assert_eq!(loupe::count(&out, "sessions_opened"), 1);
    type Row = (
        i64,
        Option<i64>,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        String,
        String,
    );
    let row: Row = l
        .db()
        .query_row(
            "SELECT c.proxy_token_id, c.account_id, c.agent_label, c.model, s.key_source, a.source, c.input, c.cache_read, c.cache_write_5m, c.cache_write_1h, c.output, s.state, c.upstream \
             FROM calls c JOIN sessions s ON s.id = c.session_id JOIN accounts a ON a.id = c.account_id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?)),
        )
        .unwrap();
    let token_id: i64 = l
        .db()
        .query_row("SELECT id FROM proxy_tokens WHERE name = 'e2e'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(row.0, token_id);
    assert!(row.1.is_some());
    assert_eq!(
        (row.2.as_str(), row.3.as_str()),
        ("claude-code", "claude-haiku-5-5")
    );
    assert_eq!(
        (row.4.as_str(), row.5.as_str()),
        ("header", "metadata.user_id")
    );
    assert_eq!((row.6, row.7, row.8, row.9, row.10), (10, 20, 4, 3, 5));
    assert_eq!((row.11.as_str(), row.12.as_str()), ("open", "fake"));
    let commit: Option<String> = l
        .db()
        .query_row("SELECT price_commit FROM calls", [], |r| r.get(0))
        .unwrap();
    assert_eq!(commit.map(|c| c.len()), Some(40));
}
