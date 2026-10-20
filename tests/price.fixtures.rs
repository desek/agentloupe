//! @agents-index Integration test: the fixture calls under tests/fixtures/price/ cost the hand-computed values at the vendored prices.

#[allow(dead_code)]
#[path = "../src/price/mod.rs"]
mod price;

use price::compute::{Call, price as compute, text};
use price::index::Index;

#[test]
fn fixture_calls_match_hand_costs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/price");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let f: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let n = |k: &str| f[k].as_i64().unwrap();
        let call = Call {
            model: f["model"].as_str().unwrap(),
            provider: f["provider"].as_str().unwrap(),
            service_tier: f["service_tier"].as_str(),
            input: n("input"),
            cache_read: n("cache_read"),
            cache_write_5m: n("cache_write_5m"),
            cache_write_1h: n("cache_write_1h"),
            output: n("output"),
        };
        let cost = compute(Index::embedded(), &call)
            .unwrap_or_else(|r| panic!("{} is unpriced: {}", path.display(), r.reason()));
        assert_eq!(
            text(cost),
            f["expected_cost_usd"].as_str().unwrap(),
            "{}",
            path.display()
        );
        seen += 1;
    }
    assert_eq!(seen, 3);
}
