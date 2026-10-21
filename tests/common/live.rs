//! @agents-index Test helper for the live route: a WebSocket client, frame reads with a timeout, and an in-process live server.
#![allow(dead_code)]

use agent_loupe::live::config::LiveConfig;
use agent_loupe::live::hub::Hub;
use agent_loupe::live::route::{LiveState, handle};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Resp = tokio_tungstenite::tungstenite::handshake::client::Response;

/// Opens a socket on `path` with the extra `headers`; the error keeps the HTTP status of a refusal.
pub async fn open(port: u16, path: &str, headers: &[(&str, &str)]) -> Result<(Ws, Resp), Error> {
    let mut req = format!("ws://127.0.0.1:{port}{path}")
        .into_client_request()
        .unwrap();
    for (k, v) in headers {
        req.headers_mut().insert(
            tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            HeaderValue::from_str(v).unwrap(),
        );
    }
    tokio_tungstenite::connect_async(req).await
}

/// The status of a refused upgrade, or 101.
pub async fn status(port: u16, path: &str, headers: &[(&str, &str)]) -> (u16, Option<Resp>) {
    match open(port, path, headers).await {
        Ok((_, r)) => (101, Some(r)),
        Err(Error::Http(r)) => (r.status().as_u16(), None),
        Err(e) => panic!("no HTTP answer: {e}"),
    }
}

/// The next message, or None after `secs` seconds.
pub async fn next(ws: &mut Ws, secs: u64) -> Option<Message> {
    tokio::time::timeout(Duration::from_secs(secs), ws.next())
        .await
        .ok()
        .flatten()
        .map(|m| m.unwrap())
}

/// The next text frame as JSON; panics after `secs` seconds.
pub async fn frame(ws: &mut Ws, secs: u64) -> serde_json::Value {
    loop {
        match next(ws, secs).await {
            Some(Message::Text(t)) => return serde_json::from_str(&t).unwrap(),
            Some(_) => continue,
            None => panic!("no frame in {secs} seconds"),
        }
    }
}

/// Reads frames until one of `kind`; returns it and every frame before it.
pub async fn until(
    ws: &mut Ws,
    kind: &str,
    secs: u64,
) -> (serde_json::Value, Vec<serde_json::Value>) {
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let left = deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .as_secs()
            .max(1);
        let f = frame(ws, left).await;
        if f["kind"] == kind {
            return (f, seen);
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no {kind} frame; saw {seen:?}"
        );
        seen.push(f);
    }
}

pub async fn send(ws: &mut Ws, m: Message) {
    ws.send(m).await.unwrap();
}

/// The sorted keys of `data` of a frame.
pub fn keys(f: &serde_json::Value) -> Vec<String> {
    let mut k: Vec<String> = f["data"].as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// A live configuration with the defaults, `rate`, and `buffer`.
pub fn config(rate: u32, buffer: usize) -> LiveConfig {
    LiveConfig {
        enabled: true,
        allowed_origin: None,
        max_rate: rate,
        max_connections: 32,
        buffer,
    }
}

/// Serves only the live route on a free port in this runtime, over a migrated empty store.
pub async fn serve(hub: Hub, cfg: LiveConfig) -> (u16, tempfile::TempDir) {
    serve_on(hub, cfg, agent_loupe::clock::system()).await
}

/// Like serve, with the timers of each client on `clock`.
pub async fn serve_on(
    hub: Hub,
    cfg: LiveConfig,
    clock: std::sync::Arc<dyn agent_loupe::clock::Clock>,
) -> (u16, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = agent_loupe::store::open::open(dir.path()).unwrap();
    agent_loupe::store::migrate::migrate(&mut c).unwrap();
    let state = LiveState::with_clock(hub, cfg, chrono_tz::UTC, c, clock);
    let app = axum::Router::new().fallback(move |req| {
        let s = state.clone();
        async move { handle(s, req).await }
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (port, dir)
}
