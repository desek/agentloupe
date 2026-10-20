//! @agents-index Integration test: no credential or session header value and no key appears in the database files or the log file.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn no_secret_in_db_or_log() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let r = reqwest::Client::new()
        .post(l.url("/pi/fake/echo/v1/messages"))
        .header("x-api-key", "sk-known-secret-777")
        .header("authorization", "Bearer tok-known-888")
        .header("chatgpt-account-id", "acct-known-999")
        .header("x-claude-code-session-id", "sess-known-555")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.bytes().await.unwrap();
    l.wait_rows(1);
    let db = l.db_bytes();
    let log = l.log();
    assert!(log.contains("exchange"), "{log}");
    let key_raw: Vec<u8> = loupe::KEY_BYTES.to_vec();
    for needle in [
        "sk-known-secret-777",
        "tok-known-888",
        "acct-known-999",
        "sess-known-555",
        loupe::KEY,
    ] {
        assert!(
            !loupe::contains(&db, needle.as_bytes()),
            "{needle} in the database"
        );
        assert!(!log.contains(needle), "{needle} in the log");
    }
    assert!(!loupe::contains(&db, &key_raw), "raw key in the database");
}
