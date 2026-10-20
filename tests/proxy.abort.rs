//! @agents-index Integration test: a client abort and an upstream abort each store the received bytes with their outcome.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

use futures_util::StreamExt;

fn row(l: &Loupe) -> (String, i64) {
    l.wait_rows(1);
    l.db()
        .query_row("SELECT outcome, response_size FROM exchanges", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn client_abort_is_captured() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let r = reqwest::get(l.url("/pi/fake/sse?chunks=10&delay_ms=200"))
        .await
        .unwrap();
    let mut s = r.bytes_stream();
    let mut got = 0;
    for _ in 0..2 {
        got += s.next().await.unwrap().unwrap().len();
    }
    drop(s);
    let (outcome, size) = row(&l);
    assert_eq!(outcome, "client_aborted");
    assert!(size as usize >= got && size > 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn upstream_abort_is_captured() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let r = reqwest::get(l.url("/pi/fake/abort?chunks=3&delay_ms=50"))
        .await
        .unwrap();
    let mut s = r.bytes_stream();
    let mut failed = false;
    while let Some(c) = s.next().await {
        failed |= c.is_err();
    }
    assert!(failed, "the client stream did not end with an error");
    let (outcome, size) = row(&l);
    assert_eq!(outcome, "upstream_aborted");
    assert!(size > 0);
}
