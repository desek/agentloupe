//! @agents-index The public API: GET /v1/days and GET /v1/days/{day}, served from the days table only.

pub mod cors;
#[path = "days.get.rs"]
pub mod days_get;
#[path = "days.list.rs"]
pub mod days_list;
pub mod error;
pub mod query;

use axum::Router;
use axum::routing::any;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// What the routes read. The connection reads only the days table; the subscriptions and the
/// time zone come from the configuration, for the envelope sum and the default range.
pub struct ApiState {
    pub conn: Mutex<Connection>,
    pub tz: chrono_tz::Tz,
    pub subscriptions: Vec<crate::subscription::config::Subscription>,
}

/// The two routes. Every method reaches the handler, which answers OPTIONS and refuses the rest.
pub fn router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/v1/days", any(days_list::handle))
        .route("/v1/days/{day}", any(days_get::handle))
        .with_state(state)
}

/// The current day in `tz`.
pub fn today(tz: chrono_tz::Tz) -> chrono::NaiveDate {
    crate::day::boundary::day_of(chrono::Utc::now().timestamp_millis(), tz)
}
