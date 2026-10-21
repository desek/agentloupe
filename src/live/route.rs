//! @agents-index The live route GET /live: the origin rule, the upgrade or 426, the connection limit or 503, the snapshot, then the client loop.

use super::config::LiveConfig;
use super::hub::Hub;
use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::Response;
use rusqlite::Connection;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The largest client message that the socket reads before the client loop closes it with 1009.
const READ_LIMIT: usize = 64 * 1024;

/// What the live route needs; built once in serve when the live feed is enabled.
pub struct LiveState {
    pub hub: Hub,
    pub cfg: LiveConfig,
    pub tz: chrono_tz::Tz,
    /// Reads the calls and the sessions for the snapshot only.
    conn: Mutex<Connection>,
    connections: AtomicUsize,
    /// Drives the heartbeat and the write timeout of each client.
    clock: Arc<dyn crate::clock::Clock>,
}

impl LiveState {
    /// The state of the binary, on the system clock.
    pub fn new(hub: Hub, cfg: LiveConfig, tz: chrono_tz::Tz, conn: Connection) -> Arc<LiveState> {
        Self::with_clock(hub, cfg, tz, conn, crate::clock::system())
    }

    /// The state on `clock`, so a test can assert the heartbeat without a wait.
    pub fn with_clock(
        hub: Hub,
        cfg: LiveConfig,
        tz: chrono_tz::Tz,
        conn: Connection,
        clock: Arc<dyn crate::clock::Clock>,
    ) -> Arc<LiveState> {
        Arc::new(LiveState {
            hub,
            cfg,
            tz,
            conn: Mutex::new(conn),
            connections: AtomicUsize::new(0),
            clock,
        })
    }
}

/// One accepted socket; the drop frees its place under the connection limit.
pub struct Slot(Arc<LiveState>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

fn take_slot(s: &Arc<LiveState>) -> Option<Slot> {
    let before = s.connections.fetch_add(1, Ordering::AcqRel);
    if before >= s.cfg.max_connections {
        s.connections.fetch_sub(1, Ordering::AcqRel);
        return None;
    }
    Some(Slot(s.clone()))
}

fn error(status: StatusCode, body: serde_json::Value) -> Response {
    crate::proxy::error::json(status, body)
}

/// Handles `GET /live`. The route reads no cookie, no token, and no query.
pub async fn handle(state: Arc<LiveState>, req: Request<Body>) -> Response {
    let h = req.headers();
    // An origin that is not text cannot equal the host, so it is refused.
    let origin = h.get(header::ORIGIN).map(|v| v.to_str().unwrap_or("\u{0}"));
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok());
    if !super::origin::accepted(origin, host, state.cfg.allowed_origin.as_deref()) {
        return error(
            StatusCode::FORBIDDEN,
            serde_json::json!({"error": "forbidden_origin", "fix": "open the dashboard at the origin of this server, or set LOUPE_LIVE_ALLOWED_ORIGIN to the origin of the front proxy"}),
        );
    }
    let (mut parts, _) = req.into_parts();
    let Ok(ws) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
        let mut r = error(
            StatusCode::UPGRADE_REQUIRED,
            serde_json::json!({"error": "upgrade_required", "fix": "send GET /live with the WebSocket upgrade headers, for example from new WebSocket(\"ws://<host>/live\")"}),
        );
        r.headers_mut()
            .insert(header::UPGRADE, HeaderValue::from_static("websocket"));
        return r;
    };
    let Some(slot) = take_slot(&state) else {
        let mut r = error(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error": "too_many_live_clients", "fix": "try again after 30 seconds, or raise LOUPE_LIVE_MAX_CONNECTIONS"}),
        );
        r.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
        return r;
    };
    // Subscribe before the snapshot, so no event between the two is lost.
    let Some(rx) = state.hub.subscribe() else {
        return error(
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "not_found", "path": "/live"}),
        );
    };
    let s = state.clone();
    let snap = tokio::task::spawn_blocking(move || {
        let conn = s.conn.lock().unwrap_or_else(|p| p.into_inner());
        super::snapshot::read(
            &conn,
            s.tz,
            chrono::Utc::now().timestamp_millis(),
            s.hub.in_flight(),
        )
    })
    .await;
    let snap = match snap {
        Ok(Ok(v)) => v,
        _ => {
            tracing::warn!(
                "the live snapshot read failed. Fix: check the database file. Verify: a new socket on /live gets a snapshot frame"
            );
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error": "snapshot_failed", "fix": "try again; check the database file of the server"}),
            );
        }
    };
    let (hub, rate, clock) = (state.hub.clone(), state.cfg.max_rate, state.clock.clone());
    ws.max_message_size(READ_LIMIT)
        .max_frame_size(READ_LIMIT)
        .on_upgrade(move |socket| {
            super::client::run(socket, rx, snap.frame(), hub, rate, clock, slot)
        })
}
