//! @agents-index Integration test: reserved routes take precedence over the proxy and never reach an upstream.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn reserved_routes_take_precedence() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    check_reserved(&l).await;
    // With an active token, reserved routes still need no token.
    let data = tempfile::tempdir().unwrap();
    loupe::create_token(data.path(), "r");
    let l = Loupe::start_in(data, fake, &[], &[]);
    check_reserved(&l).await;
}

async fn check_reserved(l: &Loupe) {
    assert_eq!(reqwest::get(l.url("/healthz")).await.unwrap().status(), 200);
    for path in ["/v1/days", "/", "/assets/app.js", "/favicon.ico"] {
        assert_eq!(
            reqwest::get(l.url(path)).await.unwrap().status(),
            404,
            "{path}"
        );
    }
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(fake::HITS.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(l.rows(), 0);
}
