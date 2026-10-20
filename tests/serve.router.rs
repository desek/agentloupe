//! @agents-index Integration test: reserved routes, the dashboard files included, take precedence over the proxy and never reach an upstream.

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
    assert_eq!(reqwest::get(l.url("/v1/days")).await.unwrap().status(), 200);
    // The static handler serves the page and its files without a token.
    for path in ["/", "/favicon.ico", "/assets/js/app.js"] {
        assert_eq!(
            reqwest::get(l.url(path)).await.unwrap().status(),
            200,
            "{path}"
        );
    }
    for path in ["/v1/other", "/assets/app.js"] {
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
