//! @agents-index Fake upstream for tests and scripts: JSON, SSE, abort, slow, echo, bytes, model calls, and summary replies by mode.
#![allow(dead_code)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Query, Request};
use axum::http::header;
use axum::response::Response;
use axum::routing::{any, get, post};
use futures_util::stream;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

type Params = Query<HashMap<String, String>>;

/// The count of requests that reached the fake upstream, so a test can prove that none did.
pub static HITS: AtomicUsize = AtomicUsize::new(0);

/// The count of summary requests per mode, so a test can prove that a disabled worker sent none.
static SUMMARY_HITS: std::sync::Mutex<Vec<(String, usize)>> = std::sync::Mutex::new(Vec::new());

/// The count of summary requests to `mode` in this process; a test uses a mode name of its own.
pub fn summary_hits(mode: &str) -> usize {
    SUMMARY_HITS
        .lock()
        .unwrap()
        .iter()
        .find(|(m, _)| m == mode)
        .map_or(0, |(_, n)| *n)
}

/// The request bodies of the summary modes that start with `rec`, so a test can read what was sent.
static SUMMARY_BODIES: std::sync::Mutex<Vec<(String, serde_json::Value)>> =
    std::sync::Mutex::new(Vec::new());

/// The request bodies sent to a `rec` mode in this process, in arrival order.
pub fn summary_bodies(mode: &str) -> Vec<serde_json::Value> {
    SUMMARY_BODIES
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, _)| m == mode)
        .map(|(_, b)| b.clone())
        .collect()
}

fn hit(mode: &str) {
    let mut h = SUMMARY_HITS.lock().unwrap();
    match h.iter_mut().find(|(m, _)| m == mode) {
        Some((_, n)) => *n += 1,
        None => h.push((mode.to_string(), 1)),
    }
}

/// The text that the `line` summary mode returns: identifying tokens that the scrubber must remove.
pub const SUMMARY_LINE: &str =
    "Fixed the parser in src/main.rs per https://x.io/a for bob@corp.com in 42 places";

/// The text of the `day` mode: four sentences, one with a URL.
pub const SUMMARY_DAY: &str = "Worked on the parser. Read https://example.com/docs today. Fixed the session close. Added tests for the scrubber. Reviewed the retention rules.";

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

fn sse_body(events: Vec<String>) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from(events.concat()))
        .unwrap()
}

fn event(name: &str, data: serde_json::Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

async fn request_json(req: Request) -> serde_json::Value {
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
}

/// Anthropic Messages: JSON, or SSE when the request has `"stream": true`.
async fn messages(req: Request) -> Response {
    let r = request_json(req).await;
    let model = r["model"]
        .as_str()
        .unwrap_or("claude-haiku-5-5")
        .to_string();
    let usage = serde_json::json!({"input_tokens":10,"cache_read_input_tokens":20,
        "cache_creation_input_tokens":7,"cache_creation":{"ephemeral_5m_input_tokens":4,"ephemeral_1h_input_tokens":3},
        "output_tokens":1,"service_tier":"standard"});
    if r["stream"] != true {
        let mut u = usage;
        u["output_tokens"] = 5.into();
        return json(serde_json::json!({"id":"msg_1","type":"message","model":model,"content":[{"type":"text","text":"hi"}],"usage":u}).to_string());
    }
    sse_body(vec![
        event(
            "message_start",
            serde_json::json!({"type":"message_start","message":{"id":"msg_1","model":model,"usage":usage}}),
        ),
        event(
            "content_block_delta",
            serde_json::json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}),
        ),
        event(
            "message_delta",
            serde_json::json!({"type":"message_delta","usage":{"output_tokens":3}}),
        ),
        event(
            "message_delta",
            serde_json::json!({"type":"message_delta","usage":{"output_tokens":5}}),
        ),
        event("message_stop", serde_json::json!({"type":"message_stop"})),
    ])
}

async fn count_tokens() -> Response {
    json(r#"{"input_tokens":12}"#.into())
}

/// OpenAI Responses: the top-level usage of a response object.
async fn responses() -> Response {
    json(r#"{"id":"resp_1","model":"gpt-5","usage":{"input_tokens":100,"input_tokens_details":{"cached_tokens":40},"output_tokens":9}}"#.into())
}

/// OpenAI Chat Completions: JSON, or SSE with usage in the last chunk.
async fn chat(req: Request) -> Response {
    let r = request_json(req).await;
    let usage = serde_json::json!({"prompt_tokens":50,"prompt_tokens_details":{"cached_tokens":10},"completion_tokens":6});
    if r["stream"] != true {
        return json(serde_json::json!({"id":"c1","model":"gpt-5","usage":usage}).to_string());
    }
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from(format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            serde_json::json!({"id":"c1","model":"gpt-5","choices":[{"delta":{"content":"hi"}}],"usage":null}),
            serde_json::json!({"id":"c1","model":"gpt-5","choices":[],"usage":usage})
        )))
        .unwrap()
}

fn message(text: &str) -> Response {
    json(serde_json::json!({"id":"msg_s","type":"message","role":"assistant","model":"claude-haiku-5-5",
        "content":[{"type":"text","text":text}],"usage":{"input_tokens":1,"output_tokens":1}}).to_string())
}

fn status(code: u16, retry_after: Option<&str>) -> Response {
    let mut b = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(r) = retry_after {
        b = b.header("retry-after", r);
    }
    b.body(Body::from(
        r#"{"type":"error","error":{"type":"fake","message":"fake"}}"#,
    ))
    .unwrap()
}

/// A summary reply chosen by the path segment: line, day, day_short, skip, 429, 429_retry, 500, 401, bad.
/// A mode that starts with `rec` records the request body: `rec_word` replies with one word,
/// `rec_401` with a 401, and any other `rec` mode with the line text.
async fn summary(axum::extract::Path(mode): axum::extract::Path<String>, req: Request) -> Response {
    hit(&mode);
    if mode.starts_with("rec") {
        let body = request_json(req).await;
        SUMMARY_BODIES.lock().unwrap().push((mode.clone(), body));
        return if mode.contains("_word") {
            message("Fixed")
        } else if mode.contains("_401") {
            status(401, None)
        } else {
            message(SUMMARY_LINE)
        };
    }
    match mode.as_str() {
        "line" | "line_counted" | "line_nocred" => message(SUMMARY_LINE),
        "day" => message(SUMMARY_DAY),
        "day_short" => message("Worked on the parser."),
        "skip" => message(" SKIP\n"),
        "429" => status(429, None),
        "429_retry" => status(429, Some("7200")),
        "500" => status(500, None),
        "401" => status(401, None),
        _ => json(r#"{"not":"a message"}"#.into()),
    }
}

pub fn app() -> Router {
    Router::new()
        .route("/v1/messages", post(messages))
        .route("/summary/{mode}/v1/messages", post(summary))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/responses", post(responses))
        .route("/v1/chat/completions", post(chat))
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
