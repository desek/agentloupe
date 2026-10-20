//! @agents-index Integration test: GET /v1/days lists a sorted range, rejects bad input, 404 for a missing day, CORS, 405, no token path.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

/// A loupe serve over a store with published records on three days and one pending day.
async fn start() -> loupe::Loupe {
    let (dir, c) = days::db();
    for (day, state) in [
        ("2026-10-12", "published"),
        ("2026-10-10", "published"),
        ("2026-10-11", "published"),
        ("2026-10-13", "pending_review"),
    ] {
        c.execute(
            "INSERT INTO days (day, record, content_hash, revision, built_at, state, published_at) VALUES (?1, ?2, 'h', 1, '2026-10-14T01:00:00.000Z', ?3, NULL)",
            rusqlite::params![day, format!(r#"{{"day":"{day}","calls":1}}"#), state],
        )
        .unwrap();
    }
    drop(c);
    let fake = fake::spawn().await;
    loupe::Loupe::start_in(dir, fake, &[], &[("LOUPE_TZ", "UTC")])
}

#[tokio::test(flavor = "multi_thread")]
async fn list_returns_range_sorted() {
    let l = start().await;
    let r = reqwest::get(l.url("/v1/days?from=2026-10-01&to=2026-10-31"))
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "application/json");
    assert_eq!(r.headers()["cache-control"], "public, max-age=60");
    let v: serde_json::Value = r.json().await.unwrap();
    let got: Vec<&str> = v["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["day"].as_str().unwrap())
        .collect();
    assert_eq!(got, ["2026-10-10", "2026-10-11", "2026-10-12"]);
    assert_eq!(v["from"], "2026-10-01");
    assert_eq!(v["updated_at"], "2026-10-14T01:00:00.000Z");
    assert_eq!(v["subscription_cost_usd"], 0);
    let one: serde_json::Value = reqwest::get(l.url("/v1/days?from=2026-10-11&to=2026-10-11"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(one["days"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_rejects_bad_input() {
    let l = start().await;
    for (q, code) in [
        ("from=x", "invalid_date"),
        ("to=2026-02-30", "invalid_date"),
        ("from=2026-10-12&to=2026-10-01", "inverted_range"),
        ("from=2025-01-01&to=2026-10-01", "range_too_long"),
        ("day=2026-10-01", "unknown_parameter"),
    ] {
        let r = reqwest::get(l.url(&format!("/v1/days?{q}"))).await.unwrap();
        assert_eq!(r.status(), 400, "{q}");
        assert_eq!(r.headers()["cache-control"], "no-store");
        assert_eq!(r.headers()["access-control-allow-origin"], "*");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"]["code"], code, "{q}");
        assert!(
            v["error"]["fix"]
                .as_str()
                .unwrap()
                .contains("/v1/days?from=")
        );
    }
    let r = reqwest::get(l.url("/v1/days/2026-1-1")).await.unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_missing_day_is_404() {
    let l = start().await;
    for day in ["2026-10-01", "2026-10-13"] {
        let r = reqwest::get(l.url(&format!("/v1/days/{day}")))
            .await
            .unwrap();
        assert_eq!(r.status(), 404);
        assert_eq!(r.headers()["cache-control"], "no-store");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"]["code"], "not_found");
    }
    let r = reqwest::get(l.url("/v1/days/2026-10-11")).await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), r#"{"day":"2026-10-11","calls":1}"#);
}

#[tokio::test(flavor = "multi_thread")]
async fn cors_headers_present() {
    let l = start().await;
    let c = reqwest::Client::new();
    for path in ["/v1/days", "/v1/days/2026-10-11", "/healthz"] {
        let r = c.get(l.url(path)).send().await.unwrap();
        assert_eq!(r.headers()["access-control-allow-origin"], "*", "{path}");
        assert!(r.headers().get("set-cookie").is_none());
    }
    let r = c
        .request(reqwest::Method::OPTIONS, l.url("/v1/days"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    assert_eq!(r.headers()["access-control-allow-origin"], "*");
    assert_eq!(r.headers()["access-control-allow-methods"], "GET, OPTIONS");
    assert_eq!(r.headers()["access-control-allow-headers"], "content-type");
}

#[tokio::test(flavor = "multi_thread")]
async fn post_returns_405() {
    let l = start().await;
    let c = reqwest::Client::new();
    for path in ["/v1/days", "/v1/days/2026-10-11"] {
        let r = c.post(l.url(path)).send().await.unwrap();
        assert_eq!(r.status(), 405);
        assert_eq!(r.headers()["allow"], "GET, OPTIONS");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn token_prefixed_api_path_not_served() {
    let l = start().await;
    let raw = loupe::create_token(l.data.path(), "api");
    let r = reqwest::get(l.url(&format!("/{raw}/v1/days")))
        .await
        .unwrap();
    assert_ne!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(!body.contains("\"days\""), "{body}");
}
