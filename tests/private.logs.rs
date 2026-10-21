//! @agents-index Integration test: the admin token and the cookie value never occur in the log file or stderr; a failed login logs the address.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::{ADMIN, client};

#[test]
fn no_secret_in_logs() {
    let (l, _) = private::start_with_fixture();
    let form = |t: &str| {
        client()
            .post(l.url("/private/login"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("token={t}"))
            .send()
            .unwrap()
    };
    let near = &ADMIN[..ADMIN.len() - 1];
    assert_eq!(form(near).status(), 401);
    let ok = form(ADMIN);
    let cookie = ok.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let value = cookie.trim_start_matches("loupe_admin=").to_string();
    client()
        .get(l.url("/private/api/v1/report?group=token"))
        .bearer_auth(ADMIN)
        .send()
        .unwrap();
    client()
        .get(l.url("/private/"))
        .header("cookie", &cookie)
        .send()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let all = format!("{}{}", l.log(), l.stderr());
    assert!(all.contains("private login failed"), "{all}");
    assert!(all.contains("127.0.0.1"));
    for secret in [ADMIN, near, value.as_str()] {
        assert!(!all.contains(secret));
    }
    assert!(!loupe::contains(&l.db_bytes(), ADMIN.as_bytes()));
}
