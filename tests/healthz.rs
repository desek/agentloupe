//! @agents-index Integration tests of GET /healthz: the CORS header of the specification, and a status that no provider changes.

#[path = "common/fake_provider.rs"]
mod fake_provider;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;
#[path = "common/providers.rs"]
mod providers;

#[test]
fn healthz_cors_header_matches_spec() {
    let l = loupe::Loupe::start(1, &[], &[]);
    let res = reqwest::blocking::get(l.url("/healthz")).unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["access-control-allow-origin"], "*");
}

#[test]
fn healthz_status_independent_of_providers() {
    let f = fake_provider::start(fake_provider::Mode::Unauthorized);
    let l = loupe::Loupe::start(
        1,
        &[],
        &[
            ("LOUPE_ADMIN_TOKEN", private::ADMIN),
            ("ANTHROPIC_API_KEY", providers::ANTHROPIC_KEY),
            ("ANTHROPIC_BASE_URL", &f.url()),
        ],
    );
    providers::wait_status(&l, "anthropic", "unauthorized");
    let res = reqwest::blocking::get(l.url("/healthz")).unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().unwrap(), r#"{"status":"ok"}"#);
}

#[path = "common/public.rs"]
mod public;

#[test]
fn healthz_same_body_in_role_public() {
    let data = tempfile::tempdir().unwrap();
    let err = tempfile::tempdir().unwrap();
    public::seed_mirror(data.path());
    let p = public::Public::start(data.path(), err.path(), &[]);
    let res = reqwest::blocking::get(p.url("/healthz")).unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["access-control-allow-origin"], "*");
    assert_eq!(res.text().unwrap(), r#"{"status":"ok"}"#);
}
