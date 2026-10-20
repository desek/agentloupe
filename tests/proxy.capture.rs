//! @agents-index Integration test: the stored exchange matches the traffic, the credential is only a hash, a large body is truncated.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;
#[allow(dead_code)]
#[path = "../src/capture/seal.rs"]
mod seal;

use hmac::{Hmac, Mac};

fn hmac_hex(input: &[u8]) -> String {
    let mut m = Hmac::<sha2::Sha256>::new_from_slice(&loupe::KEY_BYTES).unwrap();
    m.update(input);
    hex::encode(m.finalize().into_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn credential_hash_stored_raw_absent() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let resp = reqwest::Client::new()
        .post(l.url("/pi/fake/json"))
        .header("x-api-key", "sk-test-123")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.bytes().await.unwrap();
    l.wait_rows(1);
    let (hash, source): (String, String) = l
        .db()
        .query_row(
            "SELECT credential_hash, credential_source FROM exchanges",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(hash, hmac_hex(b"sk-test-123"));
    assert_eq!(source, "x-api-key");
    assert!(!loupe::contains(&l.db_bytes(), b"sk-test-123"));
}

#[tokio::test(flavor = "multi_thread")]
async fn large_body_truncated_client_gets_all() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_CAPTURE_MAX_BYTES", "1048576")]);
    let body = reqwest::get(l.url("/pi/fake/bytes?n=3145728"))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(body.len(), 3145728);
    l.wait_rows(1);
    let (flag, size, blob, id): (bool, i64, Vec<u8>, String) = l
        .db()
        .query_row(
            "SELECT response_truncated, response_size, response_body, id FROM exchanges",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert!(flag);
    assert_eq!(size, 3145728);
    assert_eq!(
        seal::open(&loupe::KEY_BYTES, &id, "response", &blob)
            .unwrap()
            .len(),
        1048576
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_exchange_matches_traffic() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let sent = br#"{"model":"claude","messages":[]}"#;
    let resp = reqwest::Client::new()
        .post(l.url("/claude-code/fake/json?beta=true"))
        .header("x-api-key", "sk-abc")
        .body(sent.to_vec())
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let got = resp.bytes().await.unwrap();
    l.wait_rows(1);
    type Row = (
        String,
        i64,
        i64,
        i64,
        String,
        Vec<u8>,
        Vec<u8>,
        String,
        String,
        String,
        String,
        String,
    );
    let r: Row = l
        .db()
        .query_row(
            "SELECT id, status, request_size, response_size, request_headers, request_body, response_body, agent, upstream, path, query, outcome FROM exchanges",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?)),
        )
        .unwrap();
    assert_eq!(r.1, status as i64);
    assert_eq!(r.2, sent.len() as i64);
    assert_eq!(r.3, got.len() as i64);
    let headers: serde_json::Value = serde_json::from_str(&r.4).unwrap();
    let key = headers
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p[0] == "x-api-key")
        .unwrap();
    assert_eq!(key[1], "redacted");
    assert_eq!(
        seal::open(&loupe::KEY_BYTES, &r.0, "request", &r.5).unwrap(),
        sent
    );
    assert_eq!(
        seal::open(&loupe::KEY_BYTES, &r.0, "response", &r.6).unwrap(),
        got.to_vec()
    );
    assert_eq!(
        (
            r.7.as_str(),
            r.8.as_str(),
            r.9.as_str(),
            r.10.as_str(),
            r.11.as_str()
        ),
        ("claude-code", "fake", "/json", "beta=true", "complete")
    );
}
