//! @agents-index Integration test: a running loupe serve answers GET /healthz with 200 and {"status":"ok"}.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start(port: u16, data: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_LISTEN", format!("127.0.0.1:{port}"))
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_DATA_KEY", KEY)
        .env("LOUPE_ACCOUNT_KEY", KEY)
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Polls until the route answers; returns the raw HTTP response.
fn get_healthz(port: u16) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) {
            s.write_all(b"GET /healthz HTTP/1.0\r\nHost: x\r\n\r\n")
                .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            return out;
        }
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn healthz_returns_ok() {
    let data = tempfile::tempdir().unwrap();
    let port = free_port();
    let mut child = start(port, data.path());
    let resp = get_healthz(port);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        resp.starts_with("HTTP/1.0 200") || resp.starts_with("HTTP/1.1 200"),
        "{resp}"
    );
    assert!(resp.ends_with(r#"{"status":"ok"}"#), "{resp}");
    // The health route is public, so other sites can read it.
    assert!(
        resp.to_ascii_lowercase()
            .contains("access-control-allow-origin: *")
    );
}
