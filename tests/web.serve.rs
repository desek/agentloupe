//! @agents-index Integration tests of the static routes: page, site title, content types, path checks, ETag, headers, method, cookies.

#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;
use std::io::{Read, Write};
use std::process::Command;

fn start(env: &[(&str, &str)]) -> Loupe {
    Loupe::start(1, &[], env)
}

/// Sends a raw request line, so the client cannot normalize the path; returns the status code.
fn raw_status(l: &Loupe, path: &str) -> u16 {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", l.port)).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out.split(' ').nth(1).unwrap().parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn root_serves_index_html() {
    let l = start(&[]);
    let res = reqwest::get(l.url("/")).await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "text/html; charset=utf-8");
    let body = res.text().await.unwrap();
    assert!(body.contains("<title>AI agent activity</title>"), "{body}");
    assert!(body.contains(">AI agent activity</h1>"));
    assert!(!body.contains("Agentloupe"));
    assert!(!body.contains("{{site_title}}"));
}

#[tokio::test(flavor = "multi_thread")]
async fn site_title_is_escaped() {
    let plain = start(&[]);
    let l = start(&[("LOUPE_SITE_TITLE", "<b>A&B</b>")]);
    let res = reqwest::get(l.url("/")).await.unwrap();
    let etag = res.headers()["etag"].clone();
    let body = res.text().await.unwrap();
    assert!(
        body.contains("<title>&lt;b&gt;A&amp;B&lt;/b&gt;</title>"),
        "{body}"
    );
    assert!(!body.contains("<b>A"));
    let default = reqwest::get(plain.url("/")).await.unwrap();
    assert_ne!(default.headers()["etag"], etag);
}

#[test]
fn invalid_site_title_is_actionable() {
    let long = "x".repeat(81);
    for title in ["   ", long.as_str()] {
        let data = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_loupe"))
            .arg("serve")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env_clear()
            .env("LOUPE_LISTEN", format!("127.0.0.1:{}", loupe::free_port()))
            .env("LOUPE_DATA_DIR", data.path())
            .env("LOUPE_DATA_KEY", loupe::KEY)
            .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
            .env("LOUPE_SITE_TITLE", title)
            .output()
            .unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("LOUPE_SITE_TITLE"), "{err}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn asset_content_types() {
    let l = start(&[]);
    for (path, ct) in [
        ("/", "text/html; charset=utf-8"),
        ("/assets/js/app.js", "text/javascript; charset=utf-8"),
        ("/assets/css/tokens.css", "text/css; charset=utf-8"),
        ("/assets/fonts/Geist-Regular.woff2", "font/woff2"),
        ("/favicon.ico", "image/x-icon"),
    ] {
        let res = reqwest::get(l.url(path)).await.unwrap();
        assert_eq!(res.status(), 200, "{path}");
        assert_eq!(res.headers()["content-type"], ct, "{path}");
        assert_eq!(res.headers()["cache-control"], "no-cache", "{path}");
    }
    let head = reqwest::Client::new()
        .head(l.url("/assets/js/app.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(head.status(), 200);
    assert!(head.headers().contains_key("etag"));
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_asset_is_404() {
    let l = start(&[]);
    let res = reqwest::get(l.url("/assets/none.js")).await.unwrap();
    assert_eq!(res.status(), 404);
    assert!(res.bytes().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn traversal_rejected() {
    let l = start(&[]);
    for path in [
        "/assets/../index.html",
        "/assets/js/..%2fapp.js",
        "/assets%2fjs/app.js",
        "/assets/js%2Fapp.js",
        "/assets/js\\app.js",
        "/assets/%2e%2e/index.html",
    ] {
        assert_eq!(raw_status(&l, path), 404, "{path}");
    }
    assert_eq!(raw_status(&l, "/assets/js/app.js"), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn etag_and_304() {
    let l = start(&[]);
    let c = reqwest::Client::new();
    for path in ["/", "/assets/css/page.css"] {
        let etag = c.get(l.url(path)).send().await.unwrap().headers()["etag"].clone();
        let res = c
            .get(l.url(path))
            .header("if-none-match", etag)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 304, "{path}");
        assert!(res.bytes().await.unwrap().is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn security_headers_on_root() {
    let l = start(&[]);
    let res = reqwest::get(l.url("/")).await.unwrap();
    let h = res.headers();
    assert_eq!(
        h["content-security-policy"],
        "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    );
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["referrer-policy"], "no-referrer");
    assert!(!h.contains_key("access-control-allow-origin"));
}

/// The live section opens a same-origin socket under the unchanged policy of the page.
#[tokio::test(flavor = "multi_thread")]
async fn csp_connect_src_self_unchanged() {
    let l = start(&[]);
    let res = reqwest::get(l.url("/")).await.unwrap();
    let csp = res.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(
        csp,
        "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    );
    let html = res.text().await.unwrap();
    assert!(html.contains("/assets/css/live.css"));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_is_405() {
    let l = start(&[]);
    let c = reqwest::Client::new();
    for path in ["/", "/assets/js/app.js", "/favicon.ico"] {
        let res = c.post(l.url(path)).send().await.unwrap();
        assert_eq!(res.status(), 405, "{path}");
        assert_eq!(res.headers()["allow"], "GET, HEAD");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn token_prefix_does_not_serve_page() {
    let data = tempfile::tempdir().unwrap();
    let token = loupe::create_token(data.path(), "w");
    let l = Loupe::start_in(data, 1, &[], &[]);
    for path in [format!("/{token}/"), format!("/{token}/assets/js/app.js")] {
        let res = reqwest::get(l.url(&path)).await.unwrap();
        let ct = res
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        let body = res.text().await.unwrap();
        assert!(!ct.starts_with("text/html"), "{path}");
        assert!(!ct.starts_with("text/javascript"), "{path}");
        assert!(!body.contains("<title>"), "{path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn no_cookie_set() {
    let l = start(&[]);
    let c = reqwest::Client::new();
    for path in ["/", "/assets/js/app.js", "/favicon.ico", "/assets/none.js"] {
        let res = c
            .get(l.url(path))
            .header("cookie", "a=b")
            .header("authorization", "Bearer x")
            .send()
            .await
            .unwrap();
        assert!(!res.headers().contains_key("set-cookie"), "{path}");
    }
}

/// The asterisk sentence of the footer links LiteLLM, says list price, and names the short commit from vendor/litellm/SOURCE.md.
#[tokio::test(flavor = "multi_thread")]
async fn footer_sentence_has_price_commit() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/vendor/litellm/SOURCE.md"
    ))
    .unwrap();
    let line = source
        .lines()
        .find(|l| l.starts_with("| Commit |"))
        .unwrap();
    let commit = line.split('`').nth(1).unwrap();
    let l = start(&[]);
    let body = reqwest::get(l.url("/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let sentence = format!(
        r#"* <a href="https://github.com/BerriAI/litellm" rel="noopener noreferrer">LiteLLM</a> list prices at <code>{}</code>, not money paid."#,
        &commit[..7]
    );
    assert!(body.contains(&sentence), "{body}");
    assert!(body.contains(r#"<template id="price-note">"#));
    assert!(!body.contains("{{price_commit}}"));
}
