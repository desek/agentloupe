//! @agents-index Fake model provider for tests: the models lists, chat completions, and Messages, answering 200, 401, or never, and recording each path.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

/// How the fake answers every request.
#[derive(Clone, Copy)]
pub enum Mode {
    Ok,
    Unauthorized,
    /// Accepts the connection and never answers, for a ping timeout.
    Hang,
}

/// A running fake on its own thread; it ends with the test process.
pub struct Fake {
    pub port: u16,
    paths: Arc<Mutex<Vec<String>>>,
}

impl Fake {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every request path in arrival order, as "METHOD /path".
    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
}

async fn answer(mode: Mode, path: &str) -> axum::response::Response {
    use axum::response::IntoResponse;
    match mode {
        Mode::Hang => {
            std::future::pending::<()>().await;
            unreachable!()
        }
        Mode::Unauthorized => (
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": {"type": "authentication_error"}})),
        )
            .into_response(),
        Mode::Ok if path.ends_with("/chat/completions") => axum::Json(serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "Fixed the parser"}}]
        }))
        .into_response(),
        Mode::Ok => axum::Json(serde_json::json!({"data": []})).into_response(),
    }
}

/// Starts the fake on a free port.
pub fn start(mode: Mode) -> Fake {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let rec = paths.clone();
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let app = axum::Router::new().fallback(move |req: axum::extract::Request| {
                let rec = rec.clone();
                async move {
                    let path = req.uri().path().to_string();
                    rec.lock().unwrap().push(format!("{} {path}", req.method()));
                    answer(mode, &path).await
                }
            });
            let l = tokio::net::TcpListener::from_std(l).unwrap();
            axum::serve(l, app).await.unwrap();
        });
    });
    Fake { port, paths }
}
