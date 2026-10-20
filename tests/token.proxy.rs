//! @agents-index Integration test: the token gate forwards a valid token with its ID, refuses with 401, and follows revokes live.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::{Loupe, create_token, revoke_token};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn valid_token_forwards_and_records_id() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let t = create_token(data.path(), "laptop");
    let l = Loupe::start_in(data, fake, &[], &[]);
    let resp = reqwest::get(l.url(&format!("/{t}/pi/fake/json")))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    l.wait_rows(1);
    let (id, path): (Option<i64>, String) = l
        .db()
        .query_row("SELECT proxy_token_id, path FROM exchanges", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    let want: i64 = l
        .db()
        .query_row(
            "SELECT id FROM proxy_tokens WHERE name = 'laptop'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(id, Some(want));
    assert_eq!(path, "/json");
}

#[tokio::test(flavor = "multi_thread")]
async fn refused_request_is_401_without_exchange() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    create_token(data.path(), "live");
    let old = create_token(data.path(), "old");
    revoke_token(data.path(), "old");
    let unknown =
        String::from_utf8(loupe::cmd(data.path(), &["token", "generate"]).stdout).unwrap();
    let unknown = unknown.trim().to_string();
    let l = Loupe::start_in(data, fake, &[], &[]);
    for (path, code, tok) in [
        ("/pi/fake/json".to_string(), "token_missing", None),
        (
            format!("/{unknown}/pi/fake/json"),
            "token_unknown",
            Some(&unknown),
        ),
        (format!("/{old}/pi/fake/json"), "token_revoked", Some(&old)),
    ] {
        let resp = reqwest::get(l.url(&path)).await.unwrap();
        assert_eq!(resp.status(), 401, "{path}");
        let text = resp.text().await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error"], code);
        assert!(
            v["fix"]
                .as_str()
                .unwrap()
                .contains("/<token>/<agent>/<upstream>")
        );
        assert_eq!(v["check"], "loupe token list");
        if let Some(t) = tok {
            assert!(!text.contains(t.as_str()));
        }
        assert!(!text.contains("lpt_"));
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(l.rows(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn revoke_applies_without_restart() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let t = create_token(data.path(), "a");
    create_token(data.path(), "b");
    let l = Loupe::start_in(data, fake, &[], &[]);
    let url = l.url(&format!("/{t}/pi/fake/json"));
    assert_eq!(reqwest::get(&url).await.unwrap().status(), 200);
    revoke_token(l.data.path(), "a");
    tokio::time::sleep(Duration::from_secs(5)).await;
    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), 401);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"], "token_revoked");
}

#[tokio::test(flavor = "multi_thread")]
async fn no_token_mode_warns_and_forwards() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    assert_eq!(
        reqwest::get(l.url("/pi/fake/json")).await.unwrap().status(),
        200
    );
    l.wait_rows(1);
    let id: Option<i64> = l
        .db()
        .query_row("SELECT proxy_token_id FROM exchanges", [], |r| r.get(0))
        .unwrap();
    assert_eq!(id, None);
    let warn: Vec<String> = l
        .log()
        .lines()
        .filter(|x| x.contains("\"WARN\"") && x.contains("open to every client"))
        .map(String::from)
        .collect();
    assert_eq!(warn.len(), 1, "{}", l.log());
    assert!(warn[0].contains("loupe token create") && warn[0].contains("LOUPE_BOOTSTRAP_TOKEN"));
    let g = String::from_utf8(loupe::cmd(l.data.path(), &["token", "generate"]).stdout).unwrap();
    let resp = reqwest::get(l.url(&format!("/{}/pi/fake/json", g.trim())))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"], "token_mode_off");
}

#[tokio::test(flavor = "multi_thread")]
async fn last_revoke_keeps_proxy_closed() {
    let fake = fake::spawn().await;
    let data = tempfile::tempdir().unwrap();
    let t = create_token(data.path(), "only");
    let l = Loupe::start_in(data, fake, &[], &[]);
    assert_eq!(
        reqwest::get(l.url(&format!("/{t}/pi/fake/json")))
            .await
            .unwrap()
            .status(),
        200
    );
    l.wait_rows(1);
    revoke_token(l.data.path(), "only");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let resp = reqwest::get(l.url("/pi/fake/json")).await.unwrap();
    assert_eq!(resp.status(), 401);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"], "token_missing");
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert!(
        l.log().contains("refuses all proxied requests"),
        "{}",
        l.log()
    );
    assert_eq!(l.rows(), 1);
}
