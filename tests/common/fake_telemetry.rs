//! @agents-index Fake telemetry endpoint for tests: records each request and each connection, and answers a status, a redirect, or never.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How the fake answers each request.
#[derive(Clone)]
pub enum Mode {
    Status(u16),
    /// Reads the request and never answers; the connection stays open.
    Hold,
    /// Answers 302 with this location.
    Redirect(String),
}

/// One request as the fake read it: the method, the target with any query, the headers with lowercase
/// names, and the body bytes after the header block.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub struct Fake {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<Seen>>>,
    pub connections: Arc<Mutex<usize>>,
}

impl Fake {
    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
    pub fn requests(&self) -> Vec<Seen> {
        self.requests.lock().unwrap().clone()
    }
    pub fn connections(&self) -> usize {
        *self.connections.lock().unwrap()
    }
}

/// Starts the fake on a free port of 127.0.0.1 on the current runtime.
pub async fn start(mode: Mode) -> Fake {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let connections = Arc::new(Mutex::new(0));
    let (r, c) = (requests.clone(), connections.clone());
    tokio::spawn(async move {
        while let Ok((sock, _)) = l.accept().await {
            *c.lock().unwrap() += 1;
            tokio::spawn(serve(sock, mode.clone(), r.clone()));
        }
    });
    Fake {
        port,
        requests,
        connections,
    }
}

/// Starts the fake on its own runtime thread, for a test that runs a loupe process and no runtime.
pub fn start_blocking(mode: Mode) -> Fake {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            tx.send(start(mode).await).unwrap();
            std::future::pending::<()>().await
        })
    });
    rx.recv().unwrap()
}

async fn serve(mut sock: tokio::net::TcpStream, mode: Mode, seen: Arc<Mutex<Vec<Seen>>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match sock.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_string();
    let target = first.next().unwrap_or("").to_string();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    let want: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end..].to_vec();
    while body.len() < want {
        match sock.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    seen.lock().unwrap().push(Seen {
        method,
        target,
        headers,
        body,
    });
    let answer = match mode {
        Mode::Hold => {
            std::future::pending::<()>().await;
            return;
        }
        Mode::Status(s) => {
            format!("HTTP/1.1 {s} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
        }
        Mode::Redirect(to) => format!(
            "HTTP/1.1 302 Found\r\nlocation: {to}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        ),
    };
    let _ = sock.write_all(answer.as_bytes()).await;
    let _ = sock.shutdown().await;
}
