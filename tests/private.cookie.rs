//! @agents-index Integration tests: the admin session cookie flags, its use, its expiry, and its end when the admin token changes.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::{ADMIN, client};

fn login(l: &loupe::Loupe, token: &str, https: bool) -> reqwest::blocking::Response {
    let mut r = client()
        .post(l.url("/private/login"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("token={token}"));
    if https {
        r = r.header("x-forwarded-proto", "https");
    }
    r.send().unwrap()
}

fn cookie_of(res: &reqwest::blocking::Response) -> String {
    res.headers()["set-cookie"].to_str().unwrap().to_string()
}

fn report(l: &loupe::Loupe, cookie: &str) -> u16 {
    client()
        .get(l.url("/private/api/v1/report?group=token"))
        .header("cookie", cookie)
        .send()
        .unwrap()
        .status()
        .as_u16()
}

#[test]
fn cookie_flags_and_expiry() {
    let (l, _) = private::start_with_fixture();
    let res = login(&l, ADMIN, false);
    assert_eq!(res.status(), 303);
    assert_eq!(res.headers()["location"], "/private/");
    let set = cookie_of(&res);
    for flag in [
        "loupe_admin=",
        "HttpOnly",
        "SameSite=Strict",
        "Path=/private",
        "Max-Age=43200",
    ] {
        assert!(set.contains(flag), "{flag}: {set}");
    }
    assert!(!set.contains("Secure"));
    assert!(!set.contains(ADMIN));
    assert!(cookie_of(&login(&l, ADMIN, true)).contains("; Secure"));
    let pair = set.split(';').next().unwrap().to_string();
    assert_eq!(report(&l, &pair), 200);
    let key = agent_loupe::private::cookie_key(ADMIN);
    let now = chrono::Utc::now().timestamp();
    let expired = agent_loupe::private::cookie::value(&key, now - 43_201);
    assert_eq!(report(&l, &format!("loupe_admin={expired}")), 401);
    assert_eq!(report(&l, "loupe_admin=1.00"), 401);
}

#[test]
fn admin_token_change_invalidates_cookie() {
    let (a, _) = private::start_with_fixture();
    let pair = cookie_of(&login(&a, ADMIN, false))
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(report(&a, &pair), 200);
    let other = "anotherAdminTokenOfFortyCharacters-000001";
    let b = loupe::Loupe::start(1, &[], &[("LOUPE_ADMIN_TOKEN", other)]);
    assert_eq!(report(&b, &pair), 401);
}
