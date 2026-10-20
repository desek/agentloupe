//! @agents-index Integration test: the proxy forwards method, path, query, headers, body unchanged, with or without a token; unknown is 404.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn forwards_request_unchanged() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    check_forward(&l, fake, "").await;
    let data = tempfile::tempdir().unwrap();
    let t = loupe::create_token(data.path(), "fwd");
    let l = Loupe::start_in(data, fake, &[], &[]);
    let echo = check_forward(&l, fake, &format!("/{t}")).await;
    assert!(!echo.to_string().contains("lpt_"), "{echo}");
}

/// Sends one request through `prefix` and checks the echo; returns it.
async fn check_forward(l: &Loupe, fake: u16, prefix: &str) -> serde_json::Value {
    let body = br#"{"x":[1,2,3],"k":"v"}"#.to_vec();
    let resp = reqwest::Client::new()
        .post(l.url(&format!("{prefix}/pi/fake/echo/a/b%20c?z=1&y=%2F&z=2")))
        .header("x-custom", "v1")
        .header("content-type", "application/json")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let echo: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(echo["method"], "POST");
    assert_eq!(echo["path"], "/echo/a/b%20c");
    assert_eq!(echo["query"], "z=1&y=%2F&z=2");
    assert_eq!(echo["body"].as_str().unwrap().as_bytes(), &body[..]);
    let headers = echo["headers"].as_array().unwrap();
    let has = |k: &str, v: &str| headers.iter().any(|p| p[0] == k && p[1] == v);
    assert!(has("x-custom", "v1"), "{echo}");
    assert!(has("content-type", "application/json"), "{echo}");
    assert!(has("host", &format!("127.0.0.1:{fake}")), "{echo}");
    assert!(!headers.iter().any(|p| p[0] == "accept-encoding"), "{echo}");
    echo
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_upstream_is_404() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let resp = reqwest::get(l.url("/claude-code/nope/x")).await.unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["upstream"], "nope");
    let known = body["known"].to_string();
    assert!(
        known.contains("anthropic") && known.contains("fake"),
        "{body}"
    );
    assert!(
        body["allowlist"]
            .as_str()
            .unwrap()
            .ends_with("upstreams.toml")
    );
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(l.rows(), 0);
}
