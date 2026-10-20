//! @agents-index Integration tests of the embed: every servable file of web/ is served, and no mockup runtime file is present.

#[path = "common/loupe.rs"]
mod loupe;

use std::path::{Path, PathBuf};

const EXT: &[&str] = &["html", "js", "css", "woff2", "svg", "ico"];

/// Every file under web/, the unit tests in web/test excluded.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            if !p.ends_with("web/test") {
                files(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

fn web_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
    let mut out = Vec::new();
    files(&root, &mut out);
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn every_web_file_is_embedded() {
    let l = loupe::Loupe::start(1, &[], &[]);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
    let mut served = 0;
    for p in web_files() {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !EXT.contains(&ext) {
            continue;
        }
        let rel = p
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let url = if rel == "index.html" {
            "/".to_string()
        } else {
            format!("/{rel}")
        };
        let res = reqwest::get(l.url(&url)).await.unwrap();
        assert_eq!(res.status(), 200, "{url}");
        if rel != "index.html" {
            assert_eq!(
                res.bytes().await.unwrap(),
                std::fs::read(&p).unwrap(),
                "{url}"
            );
        }
        served += 1;
    }
    assert!(served > 20, "only {served} files");
}

#[test]
fn no_mockup_runtime() {
    for p in web_files() {
        let s = p.to_string_lossy();
        assert!(!s.ends_with("support.js"), "{s}");
        assert!(!s.contains("/vendor/"), "{s}");
        if s.ends_with(".html") || s.ends_with(".js") {
            let text = std::fs::read_to_string(&p).unwrap();
            assert!(!text.contains("support.js"), "{s}");
            assert!(!text.contains("vendor/react"), "{s}");
        }
    }
}
