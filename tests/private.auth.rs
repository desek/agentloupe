//! @agents-index Integration tests: the bearer admin token authorizes the private API and a proxy token does not; private headers.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::{ADMIN, client};

#[test]
fn bearer_header_authorizes() {
    let (l, _) = private::start_with_fixture();
    let url = l.url("/private/api/v1/report?group=token");
    let res = client().get(&url).bearer_auth(ADMIN).send().unwrap();
    assert_eq!(res.status(), 200);
    let h = res.headers().clone();
    assert_eq!(h["cache-control"], "no-store");
    assert_eq!(h["x-frame-options"], "DENY");
    assert!(
        h["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'none'")
    );
    assert!(!h.contains_key("access-control-allow-origin"));
    let body: serde_json::Value = res.json().unwrap();
    assert!(!body["rows"].as_array().unwrap().is_empty());
    assert_eq!(client().get(&url).send().unwrap().status(), 401);
    assert_eq!(
        client()
            .get(&url)
            .bearer_auth("wrong")
            .send()
            .unwrap()
            .status(),
        401
    );
    let page = client().get(l.url("/private/")).send().unwrap();
    assert_eq!(page.status(), 303);
    assert_eq!(page.headers()["location"], "/private/login");
    let page = client()
        .get(l.url("/private/"))
        .bearer_auth(ADMIN)
        .send()
        .unwrap();
    assert_eq!(page.status(), 200);
    assert!(page.text().unwrap().contains("/private/assets/js/app.js"));
}

#[test]
fn proxy_token_is_refused() {
    let (l, raw) = private::start_with_fixture();
    let res = client()
        .get(l.url("/private/api/v1/report?group=token"))
        .bearer_auth(&raw)
        .send()
        .unwrap();
    assert_eq!(res.status(), 401);
    // The admin token is not a proxy token, so a proxied request with it is refused by the gate.
    let res = client()
        .get(l.url("/claude-code/fake/v1/models"))
        .bearer_auth(ADMIN)
        .send()
        .unwrap();
    assert_eq!(res.status(), 401);
}
