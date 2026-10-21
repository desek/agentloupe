//! @agents-index Integration tests: the login form, the neutral failure, the failed-login limit, the Origin check, and logout.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::{ADMIN, client};

fn post(
    l: &loupe::Loupe,
    path: &str,
    body: &str,
    origin: Option<&str>,
) -> reqwest::blocking::Response {
    let mut r = client()
        .post(l.url(path))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body.to_string());
    if let Some(o) = origin {
        r = r.header("origin", o);
    }
    r.send().unwrap()
}

#[test]
fn form_origin_and_logout() {
    let (l, _) = private::start_with_fixture();
    let form = client()
        .get(l.url("/private/login"))
        .send()
        .unwrap()
        .text()
        .unwrap();
    assert_eq!(form.matches("<input").count(), 1);
    assert!(form.contains("type=\"password\""));
    let res = post(
        &l,
        "/private/login",
        &format!("token={ADMIN}"),
        Some("http://evil.example"),
    );
    assert_eq!(res.status(), 403);
    let same = format!("http://127.0.0.1:{}", l.port);
    assert_eq!(
        post(&l, "/private/login", &format!("token={ADMIN}"), Some(&same)).status(),
        303
    );
    let out = post(&l, "/private/logout", "", None);
    assert_eq!(out.status(), 303);
    assert_eq!(out.headers()["location"], "/private/login");
    assert!(
        out.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        post(&l, "/private/logout", "", Some("http://evil.example")).status(),
        403
    );
}

#[test]
fn failed_login_limit() {
    let (l, _) = private::start_with_fixture();
    for i in 0..10 {
        let res = post(&l, "/private/login", "token=wrong-token", None);
        assert_eq!(res.status(), 401, "try {i}");
        let text = res.text().unwrap();
        assert!(text.contains("The token is not correct."));
    }
    assert_eq!(
        post(&l, "/private/login", "token=wrong-token", None).status(),
        429
    );
    // The block holds also for the correct token.
    assert_eq!(
        post(&l, "/private/login", &format!("token={ADMIN}"), None).status(),
        429
    );
}
