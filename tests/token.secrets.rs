//! @agents-index Integration test: no raw proxy token or token segment reaches the logs, stderr, exchanges.path, or the data dir.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::{Loupe, contains, create_token};

/// Every byte of every file under `dir`.
fn all_bytes(dir: &std::path::Path) -> Vec<u8> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(all_bytes(&p));
        } else {
            out.extend(std::fs::read(&p).unwrap_or_default());
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn token_segment_absent_from_logs_and_path() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let t = create_token(data.path(), "laptop");
    let l = Loupe::start_in(data, fake, &[], &[("RUST_LOG", "trace")]);
    let resp = reqwest::Client::new()
        .post(l.url(&format!("/{t}/claude-code/fake/v1/messages")))
        .body("{}")
        .send()
        .await
        .unwrap();
    let _ = resp.bytes().await;
    l.wait_rows(1);
    let path: String = l
        .db()
        .query_row("SELECT path FROM exchanges", [], |r| r.get(0))
        .unwrap();
    assert_eq!(path, "/v1/messages");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!l.log().contains("lpt_"));
    assert!(!l.stderr().contains("lpt_"));
    assert!(l.log().contains("\"token\":\"laptop\""), "{}", l.log());
}

#[tokio::test(flavor = "multi_thread")]
async fn raw_token_absent_from_disk() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let t = create_token(data.path(), "leak-test");
    let boot = String::from_utf8(loupe::cmd(data.path(), &["token", "generate"]).stdout).unwrap();
    let boot = boot.trim().to_string();
    let l = Loupe::start_in(data, fake, &[], &[("LOUPE_BOOTSTRAP_TOKEN", boot.as_str())]);
    for tok in [&t, &boot] {
        for _ in 0..5 {
            let r = reqwest::get(l.url(&format!("/{tok}/pi/fake/json")))
                .await
                .unwrap();
            assert_eq!(r.status(), 200);
        }
    }
    l.wait_rows(10);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let disk = all_bytes(l.data.path());
    let err = l.stderr();
    for tok in [&t, &boot] {
        assert!(!contains(&disk, tok.as_bytes()));
        assert!(!err.contains(tok.as_str()));
    }
    assert!(!contains(&disk, b"lpt_"));
}
