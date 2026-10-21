//! @agents-index The pass-through proxy: route parse, token gate, allowlist check, then the streamed forward with capture.

pub mod error;
pub mod forward;
pub mod headers;
pub mod route;
pub mod stream;
pub mod upstreams;

use crate::capture::writer::Capture;
use crate::token::gate::Decision;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use std::sync::Arc;
use std::time::Duration;

/// What every proxied request needs; built once in serve.
pub struct ProxyState {
    pub upstreams: upstreams::Upstreams,
    /// The path of the allowlist file, for the 404 body.
    pub upstreams_file: String,
    pub client: reqwest::Client,
    pub capture: Capture,
    pub account_key: [u8; 32],
    pub capture_max_bytes: usize,
    pub headers_timeout: Duration,
    pub gate: crate::token::gate::Gate,
    /// The live hub; the forward sends exchange.start and exchange.end without a wait.
    pub hub: crate::live::hub::Hub,
}

/// Makes the upstream client: no redirects followed, no decompression, the connect timeout covers TLS.
pub fn client(connect_timeout: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

/// Handles one request that no reserved route took.
pub async fn handle(state: Arc<ProxyState>, req: Request<Body>) -> Response {
    let route = match route::parse(req.uri().path(), req.uri().query()) {
        Ok(r) => r,
        Err(e) => {
            return error::json(
                StatusCode::NOT_FOUND,
                serde_json::json!({"error": "bad_route", "detail": e.0}),
            );
        }
    };
    let token = match state.gate.check(route.token.as_ref().map(|t| t.0.as_str())) {
        Decision::Accept(t) => t,
        Decision::Refuse(code) => return error::token(code),
    };
    let Some(base) = state
        .upstreams
        .get(&route.upstream)
        .map(|u| u.base_url.clone())
    else {
        let known: Vec<&String> = state.upstreams.keys().collect();
        return error::json(
            StatusCode::NOT_FOUND,
            serde_json::json!({
                "error": "unknown_upstream",
                "upstream": route.upstream,
                "known": known,
                "allowlist": state.upstreams_file,
                "fix": format!("use one of the known upstream names, or add an entry for {} to {} and restart loupe serve", route.upstream, state.upstreams_file),
            }),
        );
    };
    forward::forward(state, route, base, token, req).await
}
