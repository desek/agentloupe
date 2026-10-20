//! @agents-index Fake upstream for the tests and the latency script: JSON, SSE with delay, mid-stream close, slow, echo, sized bytes.
#![allow(dead_code)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Query, Request};
use axum::http::header;
use axum::response::Response;
use axum::routing::{any, get};
use futures_util::stream;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

type Params = Query<HashMap<String, String>>;

/// The count of requests that reached the fake upstream, so a test can prove that none did.
pub static HITS: AtomicUsize = AtomicUsize::new(0);

fn num(p: &HashMap<String, String>, k: &str, d: u64) -> u64 {
    p.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn json(body: String) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

async fn json_route() -> Response {
    json(r#"{"id":"msg_1","usage":{"input_tokens":3,"output_tokens":5}}"#.into())
}

/// `chunks` events, `delay_ms` apart; with `abort=1` the stream fails after the last event.
fn events(p: &HashMap<String, String>, abort: bool) -> Response {
    let chunks = num(p, "chunks", 5);
    let delay = Duration::from_millis(num(p, "delay_ms", 200));
    let s = stream::unfold(0u64, move |i| async move {
        if i > chunks {
            return None;
        }
        if i == chunks {
            return abort.then(|| {
                (
                    Err(std::io::Error::other("fake upstream closed mid-stream")),
                    i + 1,
                )
            });
        }
        if i > 0 {
            tokio::time::sleep(delay).await;
        }
        Some((
            Ok::<Bytes, std::io::Error>(Bytes::from(format!("data: {{\"n\":{i}}}\n\n"))),
            i + 1,
        ))
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(s))
        .unwrap()
}

async fn sse(Query(p): Params) -> Response {
    events(&p, false)
}

async fn abort(Query(p): Params) -> Response {
    events(&p, true)
}

async fn slow(Query(p): Params) -> Response {
    tokio::time::sleep(Duration::from_millis(num(&p, "headers_ms", 1000))).await;
    json(r#"{"slow":true}"#.into())
}

/// Exactly `n` bytes of `a`, streamed in 64 KiB chunks.
async fn sized(Query(p): Params) -> Response {
    let n = num(&p, "n", 0) as usize;
    let chunks: Vec<Result<Bytes, std::io::Error>> = (0..n)
        .step_by(65536)
        .map(|at| Ok(Bytes::from(vec![b'a'; 65536.min(n - at)])))
        .collect();
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap()
}

async fn echo(req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap_or_default();
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).into()))
        .collect();
    json(
        serde_json::json!({
            "method": parts.method.as_str(),
            "path": parts.uri.path(),
            "query": parts.uri.query().unwrap_or(""),
            "headers": headers,
            "body": String::from_utf8_lossy(&body),
        })
        .to_string(),
    )
}

pub fn app() -> Router {
    Router::new()
        .route("/json", any(json_route))
        .route("/sse", get(sse))
        .route("/abort", get(abort))
        .route("/slow", any(slow))
        .route("/bytes", get(sized))
        .route("/echo/{*rest}", any(echo))
        .layer(axum::middleware::from_fn(
            |req, next: axum::middleware::Next| async move {
                HITS.fetch_add(1, Ordering::SeqCst);
                next.run(req).await
            },
        ))
}

/// Serves until the process ends.
pub async fn serve(listener: tokio::net::TcpListener) {
    axum::serve(listener, app()).await.unwrap();
}

/// Starts the fake upstream on a free port on the current runtime; returns the port.
pub async fn spawn() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(listener));
    port
}
