//! @agents-index Integration test of the live route: upgrade and snapshot, 426, origin rule, 404 cases, frames from the client, limit, no cookie.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/live.rs"]
mod live;
#[path = "common/loupe.rs"]
mod loupe;

use live::{frame, next, open, send, status};
use loupe::Loupe;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

#[tokio::test(flavor = "multi_thread")]
async fn upgrade_returns_101_and_snapshot() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (mut ws, r) = open(l.port, "/live", &[]).await.unwrap();
    assert_eq!(r.status(), 101);
    let f = frame(&mut ws, 5).await;
    assert_eq!(f["v"], 1);
    assert_eq!(f["kind"], "snapshot");
    assert_eq!(f["data"]["live_enabled"], true);
    assert_eq!(f["data"]["calls"], 0);
    assert_eq!(f["data"]["date"].as_str().unwrap().len(), 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_get_is_426() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let r = reqwest::get(l.url("/live")).await.unwrap();
    assert_eq!(r.status(), 426);
    assert_eq!(r.headers()["upgrade"], "websocket");
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["error"], "upgrade_required");
}

#[tokio::test(flavor = "multi_thread")]
async fn foreign_origin_is_403() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (s, _) = status(l.port, "/live", &[("origin", "https://evil.example")]).await;
    assert_eq!(s, 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn absent_origin_is_accepted() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    assert_eq!(status(l.port, "/live", &[]).await.0, 101);
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_matches_host_any_scheme() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let o = format!("https://127.0.0.1:{}", l.port);
    assert_eq!(status(l.port, "/live", &[("origin", &o)]).await.0, 101);
    let l = Loupe::start(
        fake,
        &[],
        &[("LOUPE_LIVE_ALLOWED_ORIGIN", "https://front.example")],
    );
    assert_eq!(
        status(l.port, "/live", &[("origin", "https://front.example")])
            .await
            .0,
        101
    );
    assert_eq!(
        status(l.port, "/live", &[("origin", "https://other.example")])
            .await
            .0,
        403
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_subpath_is_404() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    assert_eq!(reqwest::get(l.url("/live/x")).await.unwrap().status(), 404);
    assert_eq!(status(l.port, "/live/x", &[]).await.0, 404);
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_label_live_refused() {
    let fake = fake::spawn().await;
    // As an agent label, the proxy refuses live: the path is reserved and never reaches the upstream.
    let l = Loupe::start(fake, &[], &[]);
    let before = fake::HITS.load(std::sync::atomic::Ordering::SeqCst);
    let r = reqwest::Client::new()
        .post(l.url("/live/fake/json"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    assert_eq!(fake::HITS.load(std::sync::atomic::Ordering::SeqCst), before);
    // As an upstream name in the allowlist file, the start refuses it.
    let data = tempfile::tempdir().unwrap();
    let up = data.path().join("upstreams.toml");
    std::fs::write(
        &up,
        "[live]\nbase_url = \"https://api.anthropic.com\"\nlitellm_provider = \"anthropic\"\n",
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .env_clear()
        .env("LOUPE_LISTEN", format!("127.0.0.1:{}", loupe::free_port()))
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_UPSTREAMS_FILE", &up)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("reserved"));
}

#[tokio::test(flavor = "multi_thread")]
async fn query_and_token_ignored() {
    let data = tempfile::tempdir().unwrap();
    let token = loupe::create_token(data.path(), "q");
    let l = Loupe::start_in(data, fake::spawn().await, &[], &[]);
    let (mut a, _) = open(l.port, "/live", &[]).await.unwrap();
    let (mut b, _) = open(l.port, &format!("/live?token={token}&x=1"), &[])
        .await
        .unwrap();
    let (fa, fb) = (frame(&mut a, 5).await, frame(&mut b, 5).await);
    assert_eq!(fa, fb);
    // The token was never read, so its last use stays empty.
    let used: Option<String> = l
        .db()
        .query_row(
            "SELECT last_used_at FROM proxy_tokens WHERE name = 'q'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(used, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_pong_and_close() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    send(&mut ws, Message::Ping(b"p".to_vec().into())).await;
    match next(&mut ws, 5).await {
        Some(Message::Pong(p)) => assert_eq!(&p[..], b"p"),
        m => panic!("no pong: {m:?}"),
    }
    send(&mut ws, Message::Close(None)).await;
    loop {
        match next(&mut ws, 5).await {
            Some(Message::Close(_)) => break,
            Some(_) => continue,
            None => panic!("no close"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn small_data_frame_discarded() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    send(&mut ws, Message::Text("a".repeat(100).into())).await;
    assert!(next(&mut ws, 1).await.is_none());
    send(&mut ws, Message::Ping(b"q".to_vec().into())).await;
    assert!(matches!(next(&mut ws, 5).await, Some(Message::Pong(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn large_data_frame_closes_1009() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (mut ws, _) = open(l.port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    send(&mut ws, Message::Binary(vec![0u8; 2048].into())).await;
    match next(&mut ws, 5).await {
        Some(Message::Close(Some(c))) => assert_eq!(c.code, CloseCode::Size),
        m => panic!("no close 1009: {m:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn disabled_is_404() {
    let l = Loupe::start(fake::spawn().await, &[], &[("LOUPE_LIVE_ENABLED", "false")]);
    assert_eq!(reqwest::get(l.url("/live")).await.unwrap().status(), 404);
    assert_eq!(status(l.port, "/live", &[]).await.0, 404);
    assert!(l.log().contains("\"live_enabled\":false"));
}

#[tokio::test(flavor = "multi_thread")]
async fn connection_limit_is_503() {
    let l = Loupe::start(
        fake::spawn().await,
        &[],
        &[("LOUPE_LIVE_MAX_CONNECTIONS", "32")],
    );
    let mut open_ones = Vec::new();
    for _ in 0..32 {
        open_ones.push(open(l.port, "/live", &[]).await.unwrap().0);
    }
    let req = reqwest::Client::new()
        .get(l.url("/live"))
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .send()
        .await
        .unwrap();
    assert_eq!(req.status(), 503);
    assert_eq!(req.headers()["retry-after"], "30");
    drop(open_ones);
    // The route frees the places when the sockets close; poll with the cap of the old fixed wait.
    let port = l.port;
    loupe::poll_async(Duration::from_millis(500), "status 101", || async move {
        match status(port, "/live", &[]).await.0 {
            101 => Ok(()),
            s => Err(format!("status {s}")),
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_cookie_set() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    let (_, r) = open(l.port, "/live", &[]).await.unwrap();
    assert!(!r.headers().contains_key("set-cookie"));
}
