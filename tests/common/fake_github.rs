//! @agents-index Fake GitHub API for tests and the bench: /user per token, paged pull request search by day, pull request lines, failures, request log.
#![allow(dead_code)]

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One pull request of the fake; `created` and `merged` are days YYYY-MM-DD.
#[derive(Clone, Debug)]
pub struct Pr {
    pub author: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub created: String,
    pub merged: Option<String>,
    pub additions: i64,
    pub deletions: i64,
}

/// The state that a test changes while the fake runs.
#[derive(Default)]
pub struct World {
    /// Token to login.
    pub logins: HashMap<String, String>,
    /// Token to a forced status of GET /user, such as 401.
    pub user_status: HashMap<String, u16>,
    pub prs: Vec<Pr>,
    /// A forced total_count of each search.
    pub total_count: Option<u64>,
    /// The next this-many requests get 429 with retry-after: 1.
    pub rate_limited: usize,
    /// Each request as "path?query".
    pub log: Vec<String>,
}

pub type Shared = Arc<Mutex<World>>;

/// A running fake; the server thread ends with the process.
pub struct Fake {
    pub url: String,
    pub world: Shared,
}

impl Fake {
    pub fn requests(&self) -> usize {
        self.world.lock().unwrap().log.len()
    }
    pub fn log(&self) -> Vec<String> {
        self.world.lock().unwrap().log.clone()
    }
    pub fn with<T>(&self, f: impl FnOnce(&mut World) -> T) -> T {
        f(&mut self.world.lock().unwrap())
    }
}

/// A merged pull request of `author` with simple names, created and merged on `day`.
pub fn pr(author: &str, number: u64, created: &str, merged: Option<&str>) -> Pr {
    Pr {
        author: author.into(),
        owner: "Fake-Owner".into(),
        repo: "fake-repo".into(),
        number,
        title: format!("change {number}"),
        created: created.into(),
        merged: merged.map(str::to_string),
        additions: 10,
        deletions: 3,
    }
}

fn token(h: &HeaderMap) -> String {
    h.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string()
}

/// Logs the request and returns a 429 when the world says so.
fn enter(w: &mut World, line: String) -> Option<Response> {
    w.log.push(line);
    if w.rate_limited > 0 {
        w.rate_limited -= 1;
        return Some((StatusCode::TOO_MANY_REQUESTS, [("retry-after", "1")], "{}").into_response());
    }
    None
}

async fn user(State(s): State<Shared>, h: HeaderMap) -> Response {
    let mut w = s.lock().unwrap();
    if let Some(r) = enter(&mut w, "/user".into()) {
        return r;
    }
    let t = token(&h);
    if let Some(code) = w.user_status.get(&t) {
        return StatusCode::from_u16(*code).unwrap().into_response();
    }
    match w.logins.get(&t) {
        Some(l) => axum::Json(json!({"login": l})).into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// The value after `name:` in the query text.
fn term<'a>(q: &'a str, name: &str) -> Option<&'a str> {
    q.split(' ').find_map(|t| t.strip_prefix(name))
}

async fn search(State(s): State<Shared>, Query(p): Query<HashMap<String, String>>) -> Response {
    let mut w = s.lock().unwrap();
    let q = p.get("q").cloned().unwrap_or_default();
    let page: usize = p.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let per: usize = p.get("per_page").and_then(|v| v.parse().ok()).unwrap_or(30);
    if let Some(r) = enter(&mut w, format!("/search/issues?q={q}&page={page}")) {
        return r;
    }
    let author = term(&q, "author:").unwrap_or_default();
    let (merged, range) = match term(&q, "merged:") {
        Some(r) => (true, r),
        None => (false, term(&q, "created:").unwrap_or_default()),
    };
    let day = range.get(..10).unwrap_or_default();
    let hits: Vec<&Pr> = w
        .prs
        .iter()
        .filter(|x| x.author == author)
        .filter(|x| {
            if merged {
                x.merged.as_deref() == Some(day)
            } else {
                x.created == day
            }
        })
        .collect();
    let total = w.total_count.unwrap_or(hits.len() as u64);
    let items: Vec<_> = hits
        .iter()
        .skip((page - 1) * per)
        .take(per)
        .map(|x| {
            json!({
                "number": x.number,
                "title": x.title,
                "repository_url": format!("https://api.github.com/repos/{}/{}", x.owner, x.repo),
                "html_url": format!("https://github.com/{}/{}/pull/{}", x.owner, x.repo, x.number),
            })
        })
        .collect();
    axum::Json(json!({"total_count": total, "incomplete_results": false, "items": items}))
        .into_response()
}

async fn pull(
    State(s): State<Shared>,
    Path((owner, repo, n)): Path<(String, String, u64)>,
) -> Response {
    let mut w = s.lock().unwrap();
    if let Some(r) = enter(&mut w, format!("/repos/{owner}/{repo}/pulls/{n}")) {
        return r;
    }
    match w
        .prs
        .iter()
        .find(|x| x.owner == owner && x.repo == repo && x.number == n)
    {
        Some(x) => {
            axum::Json(json!({"additions": x.additions, "deletions": x.deletions})).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Starts the fake on `port` (0 picks a free port) in a background thread.
pub fn start_on(port: u16, world: World) -> Fake {
    let shared: Shared = Arc::new(Mutex::new(world));
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", std_listener.local_addr().unwrap());
    let app = Router::new()
        .route("/user", get(user))
        .route("/search/issues", get(search))
        .route("/repos/{owner}/{repo}/pulls/{n}", get(pull))
        .with_state(shared.clone());
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let l = tokio::net::TcpListener::from_std(std_listener).unwrap();
            axum::serve(l, app).await.unwrap();
        });
    });
    Fake { url, world: shared }
}

pub fn start(world: World) -> Fake {
    start_on(0, world)
}

/// A world with one token `token` of the login `login`.
pub fn world(token: &str, login: &str) -> World {
    let mut w = World::default();
    w.logins.insert(token.into(), login.into());
    w
}
