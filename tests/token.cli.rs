//! @agents-index Integration test: loupe token generate stores nothing, list never shows a token or hash, dry runs write nothing.

#[path = "common/loupe.rs"]
mod loupe;

use loupe::{cmd, create_token};

#[test]
fn generate_stores_nothing() {
    let data = tempfile::tempdir().unwrap();
    let out = cmd(data.path(), &["token", "generate"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
    let t = text.trim();
    assert!(t.starts_with("lpt_") && t.len() == 47, "{t}");
    assert_eq!(std::fs::read_dir(data.path()).unwrap().count(), 0);
}

#[test]
fn list_never_shows_token_or_hash() {
    let data = tempfile::tempdir().unwrap();
    let a = create_token(data.path(), "laptop");
    let out = cmd(
        data.path(),
        &[
            "token", "create", "--name", "ci", "--tag", "team=ops", "--tag", "env=prod",
        ],
    );
    assert!(out.status.success());
    let b: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(b["tags"]["team"], "ops");
    assert!(b["created_at"].is_string());
    let b = b["token"].as_str().unwrap().to_string();
    for human in [false, true] {
        let mut args = vec!["token", "list"];
        if human {
            args.push("--human");
        }
        let out = cmd(data.path(), &args);
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("laptop") && text.contains("ci") && text.contains("ops"));
        assert!(text.contains("active"));
        assert!(!text.contains("lpt_"), "{text}");
        for raw in [&a, &b] {
            let h = hex_sha256(raw);
            assert!(!text.contains(&h) && !text.contains(&h[..16]), "{text}");
        }
        if !human {
            assert_eq!(text.lines().count(), 2);
            for l in text.lines() {
                let v: serde_json::Value = serde_json::from_str(l).unwrap();
                assert!(v.get("created_at").is_some() && v.get("last_used_at").is_some());
                assert!(v.get("revoked_at").is_some() && v.get("status").is_some());
            }
        }
    }
    let again = cmd(data.path(), &["token", "create", "--name", "laptop"]);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("loupe token list"));
    assert!(
        !cmd(data.path(), &["token", "create", "--name", "bootstrap"])
            .status
            .success()
    );
}

#[test]
fn dry_run_writes_nothing() {
    let data = tempfile::tempdir().unwrap();
    create_token(data.path(), "keep");
    let snapshot = || String::from_utf8(cmd(data.path(), &["token", "list"]).stdout).unwrap();
    let before = snapshot();
    let out = cmd(
        data.path(),
        &["token", "create", "--name", "new", "--dry-run"],
    );
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("lpt_"));
    let out = cmd(data.path(), &["token", "revoke", "keep", "--dry-run"]);
    assert!(out.status.success());
    assert_eq!(snapshot(), before);
    assert!(
        !cmd(data.path(), &["token", "revoke", "nope", "--dry-run"])
            .status
            .success()
    );
    let missing = cmd(data.path(), &["token", "create"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--name"));
}

fn hex_sha256(s: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}
