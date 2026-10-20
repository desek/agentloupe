//! @agents-index Integration test: loupe serve without keys exits non-zero before it binds the port, naming the variable.

use std::net::{TcpListener, TcpStream};
use std::process::Command;

#[test]
fn serve_without_keys_exits_nonzero() {
    let data = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let out = Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_LISTEN", format!("127.0.0.1:{port}"))
        .env("LOUPE_DATA_DIR", data.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("LOUPE_DATA_KEY"), "{stderr}");
    assert!(
        TcpStream::connect(("127.0.0.1", port)).is_err(),
        "port was bound"
    );
    assert!(!data.path().join("loupe.db").exists());
}
