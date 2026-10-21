//! @agents-index Integration test of GET /private/api/providers: authorization, the shape, no CORS header, and no route without the admin token.

#[path = "common/fake_provider.rs"]
mod fake_provider;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;
#[path = "common/providers.rs"]
mod providers;

#[test]
fn provider_status_route_shape_and_auth() {
    let f = fake_provider::start(fake_provider::Mode::Ok);
    let l = loupe::Loupe::start(
        1,
        &[],
        &[
            ("LOUPE_ADMIN_TOKEN", private::ADMIN),
            ("CLAUDE_CODE_OAUTH_TOKEN", providers::OAUTH_TOKEN),
            ("ANTHROPIC_BASE_URL", &f.url()),
        ],
    );
    let url = l.url("/private/api/providers");
    assert_eq!(private::client().get(&url).send().unwrap().status(), 401);
    let bad = private::client()
        .get(&url)
        .bearer_auth("x".repeat(40))
        .send()
        .unwrap();
    assert_eq!(bad.status(), 401);
    let body = providers::wait_status(&l, "anthropic", "ok");
    let list = body["providers"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1]["name"], "openai");
    assert_eq!(list[1]["status"], "not_configured");
    for p in list {
        let keys: Vec<&String> = p.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["checked_at", "name", "status"]);
        chrono::DateTime::parse_from_rfc3339(p["checked_at"].as_str().unwrap()).unwrap();
    }
    let res = private::client()
        .get(&url)
        .bearer_auth(private::ADMIN)
        .send()
        .unwrap();
    assert!(res.headers().get("access-control-allow-origin").is_none());
    assert_eq!(res.headers()["cache-control"], "no-store");
    let text = res.text().unwrap();
    assert!(providers::clean(&text) && !text.contains(&f.url()) && !text.contains("http"));
    assert_eq!(f.paths(), ["GET /v1/models"]);
    let off = loupe::Loupe::start(1, &[], &[]);
    let res = private::client()
        .get(off.url("/private/api/providers"))
        .send()
        .unwrap();
    assert_eq!(res.status(), 404);
}
