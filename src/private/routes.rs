//! @agents-index Dispatches a path under /private/: login, logout, page files, and the report, session, providers, and config API, behind authorization.

use super::{PrivateState, headers, login, query};
use crate::web::Site;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::net::{IpAddr, SocketAddr};

const FIX_AUTH: &str =
    "send Authorization: Bearer <LOUPE_ADMIN_TOKEN>, or log in at /private/login";

fn not_found(path: &str) -> Response {
    crate::proxy::error::json(
        StatusCode::NOT_FOUND,
        serde_json::json!({"error": "not_found", "path": path}),
    )
}

fn method_not_allowed(allow: &'static str) -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, allow)]).into_response()
}

/// A built-in file of web/private/; the path is checked before the lookup.
fn file(site: &Site, rel: &str) -> Response {
    let bad = ["..", "\\", "%"];
    if bad.iter().any(|b| rel.contains(b)) {
        return not_found(rel);
    }
    match site.get(&format!("private/{rel}")) {
        Some(f) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, f.content_type)],
            Body::from(f.body.clone().into_owned()),
        )
            .into_response(),
        None => not_found(rel),
    }
}

/// Answers every path under /private/ while the admin token is set, with the private headers.
pub async fn handle(state: &PrivateState, site: &Site, req: Request<Body>) -> Response {
    headers::apply(route(state, site, req).await)
}

async fn route(state: &PrivateState, site: &Site, req: Request<Body>) -> Response {
    let path = req.uri().path().to_string();
    let https = req.uri().scheme_str() == Some("https");
    let addr = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let m = req.method().clone();
    match path.as_str() {
        "/private/login" if m == Method::GET => return login::form(StatusCode::OK, false),
        "/private/login" | "/private/logout" if m != Method::POST => {
            return method_not_allowed(if path == "/private/login" {
                "GET, POST"
            } else {
                "POST"
            });
        }
        "/private/login" | "/private/logout" if !login::same_origin(req.headers()) => {
            return (
                StatusCode::FORBIDDEN,
                "the Origin of the request is not this origin\n",
            )
                .into_response();
        }
        "/private/login" => return login::post(state, req, addr, https).await,
        "/private/logout" => return login::logout(req.headers(), https),
        _ => {}
    }
    if let Some(rel) = path.strip_prefix("/private/assets/") {
        // The code of the page holds no data, so the login form can use the style sheet.
        if m != Method::GET {
            return method_not_allowed("GET");
        }
        return file(site, &format!("assets/{rel}"));
    }
    let api = path.starts_with("/private/api/");
    if !super::auth::authorized(state, req.headers(), chrono::Utc::now().timestamp()) {
        return if api {
            query::error(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "the request has no valid admin authorization".into(),
                FIX_AUTH,
            )
        } else {
            login::see_other("/private/login", None)
        };
    }
    if m != Method::GET {
        return method_not_allowed("GET");
    }
    match path.as_str() {
        "/private" => login::see_other("/private/", None),
        "/private/" => file(site, "index.html"),
        "/private/api/v1/report" | "/private/api/v1/sessions" => api_get(state, &req, &path),
        "/private/api/providers" => super::api_providers::get(state),
        "/private/api/config" => super::api_config::get(state),
        _ => not_found(&path),
    }
}

fn api_get(state: &PrivateState, req: &Request<Body>, path: &str) -> Response {
    let today = crate::api::today(state.tz);
    let p = match query::parse(req.uri().query(), today) {
        Ok(p) => p,
        Err(r) => return *r,
    };
    let conn = state.conn.lock().unwrap_or_else(|e| e.into_inner());
    let out = if path.ends_with("/report") {
        let Some(g) = p.group.as_ref() else {
            return query::error(
                StatusCode::BAD_REQUEST,
                "missing_group",
                "the report needs group=token or group=tag".into(),
                "send GET /private/api/v1/report?group=token",
            );
        };
        super::report::build(&conn, state.tz, p.from, p.to, g)
    } else {
        let Some(name) = p.token.as_deref() else {
            return query::error(
                StatusCode::BAD_REQUEST,
                "missing_token",
                "the session list needs token=<name>".into(),
                "send GET /private/api/v1/sessions?token=<name of a proxy token>",
            );
        };
        super::sessions::list(&conn, state.tz, p.from, p.to, name, p.cursor)
    };
    match out {
        Ok(v) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            v.to_string(),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "private report query failed");
            query::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "the store could not be read".into(),
                "try again later; read logs/loupe.log for the database error",
            )
        }
    }
}
