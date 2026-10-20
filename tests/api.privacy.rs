//! @agents-index Integration test: the public routes under /v1/ return no proxy token name, tag, ID, or hash.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn public_routes_have_no_token_data() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let out = loupe::cmd(
        data.path(),
        &[
            "token",
            "create",
            "--name",
            "secretname",
            "--tag",
            "owner=tagvalue",
        ],
    );
    assert!(out.status.success());
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let raw = raw["token"].as_str().unwrap().to_string();
    let hash: String = {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(raw.as_bytes()))
    };
    let l = Loupe::start_in(data, fake, &[], &[]);
    for path in [
        "/v1/days",
        "/v1/days/2026-10-10",
        "/v1/days?from=bad",
        "/v1/sessions",
        "/v1/calls",
        "/v1/projects",
        "/v1/summary",
        "/healthz",
    ] {
        let body = reqwest::get(l.url(path))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        for secret in [
            "secretname",
            "owner",
            "tagvalue",
            hash.as_str(),
            "lpt_",
            "proxy_token",
        ] {
            assert!(!body.contains(secret), "{path}: {body}");
        }
    }
}
