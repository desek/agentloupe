//! @agents-index Benchmark for cost.bench.sh: times the parse of the embedded price index and the median price of one call.

#[allow(dead_code)]
#[path = "../src/price/mod.rs"]
mod price;

use price::compute::{Call, price as compute};
use price::index::Index;
use std::time::Instant;

fn main() {
    let runs: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);
    let t0 = Instant::now();
    let index = Index::parse(price::embed::PRICE_FILE).expect("the embedded price file parses");
    let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let call = Call {
        model: "claude-haiku-5-5-20260101",
        provider: "anthropic",
        service_tier: Some("standard"),
        input: 1200,
        cache_read: 30_000,
        cache_write_5m: 800,
        cache_write_1h: 400,
        output: 900,
    };
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(compute(&index, std::hint::black_box(&call)).ok());
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    times.sort_by(f64::total_cmp);
    println!("entries: {}", index.len());
    println!("load_ms: {load_ms:.1}");
    println!("price_us_median: {:.2}", times[times.len() / 2]);
    println!("runs: {runs}");
}
