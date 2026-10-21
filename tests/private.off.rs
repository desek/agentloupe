//! @agents-index Integration tests: without the admin token every /private/ path is 404; a short admin token stops the start.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::client;

#[test]
fn no_admin_token_means_404() {
    for env in [&[][..], &[("LOUPE_ADMIN_TOKEN", "")][..]] {
        let l = loupe::Loupe::start(1, &[], env);
        let unknown: serde_json::Value = client()
            .get(l.url("/v1/other"))
            .send()
            .unwrap()
            .json()
            .unwrap();
        for path in [
            "/private",
            "/private/",
            "/private/login",
            "/private/api/v1/report?group=token",
            "/private/assets/js/app.js",
            "/private/x",
        ] {
            let res = client().get(l.url(path)).send().unwrap();
            assert_eq!(res.status(), 404, "{path}");
            let body: serde_json::Value = res.json().unwrap();
            assert_eq!(body["error"], unknown["error"], "{path}");
        }
        let res = client()
            .post(l.url("/private/login"))
            .body("token=x")
            .send()
            .unwrap();
        assert_eq!(res.status(), 404);
    }
}

#[test]
fn short_admin_token_stops_start() {
    let data = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_LISTEN", format!("127.0.0.1:{}", loupe::free_port()))
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_ADMIN_TOKEN", "short-admin-token-1234")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    for part in [
        "LOUPE_ADMIN_TOKEN",
        "loupe token generate",
        ".env.example",
        "loupe config show",
    ] {
        assert!(err.contains(part), "{part}: {err}");
    }
    assert!(!err.contains("short-admin-token-1234"), "{err}");
}
