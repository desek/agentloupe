//! @agents-index Measurement test: the resident memory of loupe serve before and after 32 live clients; the difference is at most 8 MiB.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/live.rs"]
mod live;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;
use std::time::Duration;

const THRESHOLD_KIB: i64 = 8 * 1024;

/// The resident set size of `pid` in KiB, from ps.
fn rss_kib(pid: u32) -> i64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn rss_delta_32_clients() {
    let l = Loupe::start(fake::spawn().await, &[], &[]);
    // One socket first, so the lazy parts of the runtime and the route are in the baseline.
    drop(live::open(l.port, "/live", &[]).await.unwrap());
    tokio::time::sleep(Duration::from_millis(500)).await;
    let before = rss_kib(l.pid());
    let mut all = Vec::new();
    for _ in 0..32 {
        let (mut ws, _) = live::open(l.port, "/live", &[]).await.unwrap();
        live::frame(&mut ws, 5).await;
        all.push(ws);
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let after = rss_kib(l.pid());
    println!(
        "rss_before_kib: {before} rss_after_kib: {after} delta_kib: {} threshold_kib: {THRESHOLD_KIB}",
        after - before
    );
    assert!(after - before <= THRESHOLD_KIB);
    drop(all);
}
