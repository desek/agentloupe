//! @agents-index Forwards one request to its upstream with streamed bodies and timeouts, and returns 502 if unreachable.

use super::stream::{CaptureStream, Pending};
use super::{ProxyState, error, headers, route::Route};
use crate::capture::record::{ExchangeRecord, new_id};
use crate::capture::tap::Tap;
use crate::capture::{credential, redact, session_hint};
use crate::token::gate::Accepted;
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::Response;
use futures_util::StreamExt;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// True if the client request carries a body; a request with none is sent with none.
fn has_body(h: &axum::http::HeaderMap) -> bool {
    h.contains_key(header::TRANSFER_ENCODING)
        || h.get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v != "0")
}

/// The cause chain of a send error, with no URL, so neither the query nor a header reaches the text.
fn cause(e: reqwest::Error) -> String {
    let what = if e.is_connect() {
        "connect, DNS, or TLS failure"
    } else {
        "request failure"
    };
    let e = e.without_url();
    let mut text = format!("{what}: {e}");
    let mut src = std::error::Error::source(&e);
    while let Some(s) = src {
        text.push_str(&format!(": {s}"));
        src = s.source();
    }
    text
}

/// Sends the request and streams the response back; every outcome submits one exchange record.
pub async fn forward(
    state: Arc<ProxyState>,
    route: Route,
    base: String,
    token: Option<Accepted>,
    req: Request<Body>,
) -> Response {
    let t0 = Instant::now();
    state.hub.exchange_start();
    let (parts, body) = req.into_parts();
    let mut h = parts.headers;
    let cred = credential::credential(&state.account_key, &h);
    let acct = credential::account_id(&state.account_key, &h);
    let hint = session_hint::session_hint(&state.account_key, &h);
    let with_body = has_body(&h);
    headers::for_request(&mut h);
    let url = if route.query.is_empty() {
        format!("{base}{}", route.rest)
    } else {
        format!("{base}{}?{}", route.rest, route.query)
    };
    if let Some(host) = reqwest::Url::parse(&url).ok().and_then(|u| {
        let host = u.host_str()?.to_string();
        Some(u.port().map_or(host.clone(), |p| format!("{host}:{p}")))
    }) && let Ok(v) = HeaderValue::from_str(&host)
    {
        h.insert(header::HOST, v);
    }
    let request_tap = Arc::new(Mutex::new(Tap::new(state.capture_max_bytes)));
    let mut record = ExchangeRecord {
        id: new_id(),
        started_ms: chrono::Utc::now().timestamp_millis(),
        ended_ms: 0,
        ttfb_ms: None,
        agent: route.agent.clone(),
        upstream: route.upstream.clone(),
        method: parts.method.to_string(),
        path: route.rest.clone(),
        query: route.query.clone(),
        status: 0,
        request_size: 0,
        response_size: 0,
        request_headers: redact::request(&h),
        response_headers: serde_json::json!([]),
        request_body: Vec::new(),
        response_body: Vec::new(),
        request_truncated: false,
        response_truncated: false,
        credential_hash: cred.as_ref().map(|c| c.0.clone()),
        credential_source: cred.map(|c| c.1),
        account_id_hash: acct.as_ref().map(|c| c.0.clone()),
        account_id_source: acct.map(|c| c.1),
        session_hint_hash: hint.as_ref().map(|c| c.0.clone()),
        session_hint_source: hint.map(|c| c.1),
        outcome: String::new(),
        error: None,
        proxy_token_id: token.as_ref().map(|t| t.id),
    };
    let token_name = token.map_or_else(|| "none".to_string(), |t| t.name);
    let mut builder = state.client.request(parts.method, &url).headers(h);
    if with_body {
        let tap = request_tap.clone();
        let stream = body.into_data_stream().map(move |r| {
            if let Ok(b) = &r {
                tap.lock().unwrap_or_else(|p| p.into_inner()).push(b);
            }
            r
        });
        builder = builder.body(reqwest::Body::wrap_stream(stream));
    }
    let sent: Result<reqwest::Response, String> = match builder.build() {
        Ok(r) => match tokio::time::timeout(state.headers_timeout, state.client.execute(r)).await {
            Ok(Ok(r)) => Ok(r),
            Ok(Err(e)) => Err(cause(e)),
            Err(_) => Err(format!(
                "timeout: the upstream sent no response headers within {} seconds",
                state.headers_timeout.as_secs()
            )),
        },
        Err(e) => Err(cause(e)),
    };
    let resp = match sent {
        Ok(r) => r,
        Err(why) => {
            record.status = 502;
            let pending = Pending {
                record,
                request_tap,
                capture: state.capture.clone(),
                token_name,
                hub: state.hub.clone(),
            };
            pending.finish(Tap::new(0), "upstream_unreachable", Some(why.clone()));
            return error::json(
                StatusCode::BAD_GATEWAY,
                serde_json::json!({"error": "upstream_unreachable", "upstream": route.upstream, "cause": why}),
            );
        }
    };
    record.ttfb_ms = Some(t0.elapsed().as_millis() as i64);
    record.status = resp.status().as_u16();
    let mut out_headers = resp.headers().clone();
    headers::strip_hop_by_hop(&mut out_headers);
    record.response_headers = redact::response(&out_headers);
    let status = resp.status();
    // A response with no body is never polled, so its expected length is zero.
    let bodiless = record.method == "HEAD"
        || status.is_informational()
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED;
    let expected = if bodiless {
        Some(0)
    } else {
        resp.content_length()
    };
    let pending = Pending {
        record,
        request_tap,
        capture: state.capture.clone(),
        token_name,
        hub: state.hub.clone(),
    };
    let stream = CaptureStream::new(
        Box::pin(resp.bytes_stream()),
        state.capture_max_bytes,
        expected,
        pending,
    );
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    *response.headers_mut() = out_headers;
    response
}
