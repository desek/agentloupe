//! @agents-index Integration test: with the admin token set, the public API and the dashboard files hold no token name, tag, or ID.

#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;

use private::client;

#[test]
fn public_api_has_no_token_data() {
    let (l, _) = private::start_with_fixture();
    let ids: Vec<i64> = {
        let db = l.db();
        let mut st = db.prepare("SELECT id FROM proxy_tokens").unwrap();
        st.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    for path in [
        "/v1/days",
        "/v1/days?from=2026-01-01",
        "/v1/other",
        "/",
        "/assets/js/app.js",
        "/assets/js/api.days.js",
        "/healthz",
    ] {
        let body = client().get(l.url(path)).send().unwrap().text().unwrap();
        for secret in ["laptop", "alice", "owner", "proxy_token", "/private/api"] {
            assert!(!body.contains(secret), "{path} holds {secret}");
        }
        for id in &ids {
            assert!(
                !body.contains(&format!("\"proxy_token_id\":{id}")),
                "{path}"
            );
        }
    }
    // No file of the public dashboard reads the private API.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/assets/js");
    for e in std::fs::read_dir(root).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        assert!(!text.contains("/private/"));
    }
}
