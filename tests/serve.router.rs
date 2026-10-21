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
    check_reserved(&l, "/pi/fake/json").await;
    // With an active token, reserved routes still need no token.
    let data = tempfile::tempdir().unwrap();
    let t = loupe::create_token(data.path(), "r");
    let l = Loupe::start_in(data, fake, &[], &[]);
    check_reserved(&l, &format!("/{t}/pi/fake/json")).await;
}

/// `proxied` is a path that the proxy forwards, sent last so its row ends the check.
async fn check_reserved(l: &Loupe, proxied: &str) {
    let hits = fake::HITS.load(std::sync::atomic::Ordering::SeqCst);
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
    for path in ["/v1/other", "/assets/app.js", "/live/x"] {
        assert_eq!(
            reqwest::get(l.url(path)).await.unwrap().status(),
            404,
            "{path}"
        );
    }
    // The live route takes /live, so the proxy never gets it.
    assert_eq!(reqwest::get(l.url("/live")).await.unwrap().status(), 426);
    assert_eq!(fake::HITS.load(std::sync::atomic::Ordering::SeqCst), hits);
    // A proxied request after the reserved routes: its row is the end state, and it must be the only row.
    assert_eq!(reqwest::get(l.url(proxied)).await.unwrap().status(), 200);
    l.wait_rows(1);
    assert_eq!(l.rows(), 1);
}
