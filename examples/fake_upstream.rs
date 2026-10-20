//! @agents-index Runs the fake upstream of the tests on 127.0.0.1 and the port from the first argument, for the latency script.

#[path = "../tests/common/fake_upstream.rs"]
mod fake_upstream;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| {
            eprintln!("error: the port is missing. Fix: pass it as the first argument, for example cargo run --example fake_upstream -- 9100. Verify: curl http://127.0.0.1:9100/json returns JSON.");
            std::process::exit(2);
        });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap_or_else(|e| {
        eprintln!("error: cannot bind 127.0.0.1:{port}: {e}. Fix: pass a free port. Verify: the program keeps running.");
        std::process::exit(1);
    });
    fake_upstream::serve(listener).await;
}
