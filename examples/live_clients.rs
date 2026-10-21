//! @agents-index Opens N live clients on a running loupe serve, reads each frame, and holds them until killed; for proxy.latency.sh.
//!
//! Usage: live_clients <port> <count>. Prints `live_clients_open: <count>` when every socket has its snapshot.

use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(port), Some(n)) = (
        args.get(1),
        args.get(2).and_then(|n| n.parse::<usize>().ok()),
    ) else {
        eprintln!(
            "error: missing arguments. Fix: run live_clients <port> <count>, for example live_clients 8787 32. Verify: it prints live_clients_open."
        );
        std::process::exit(2);
    };
    let mut readers = Vec::new();
    for _ in 0..n {
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/live"))
            .await
            .unwrap_or_else(|e| {
                eprintln!("error: the live socket did not open: {e}. Fix: start loupe serve with LOUPE_LIVE_ENABLED=true on port {port}. Verify: curl -i /live returns 426.");
                std::process::exit(1);
            });
        ws.next().await;
        // Each client keeps reading, as a browser does, so the route never sees a slow client.
        readers.push(tokio::spawn(
            async move { while ws.next().await.is_some() {} },
        ));
    }
    println!("live_clients_open: {n}");
    for r in readers {
        let _ = r.await;
    }
}
