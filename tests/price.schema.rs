//! @agents-index Integration test: the embedded price file is valid against the embedded schema, and every read field is a schema property.

use jsonschema::PatternOptions;
use serde_json::Value;

const DATA: &str = include_str!("../vendor/litellm/model_prices_and_context_window.json");
const SCHEMA: &str = include_str!("../vendor/litellm/model_prices_and_context_window.schema.json");

/// Every `pattern` string of the schema, found by a walk over all objects.
fn patterns(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(p)) = m.get("pattern") {
                out.push(p.clone());
            }
            m.values().for_each(|x| patterns(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| patterns(x, out)),
        _ => {}
    }
}

fn options() -> jsonschema::ValidationOptions<'static> {
    // The Rust regex syntax accepts the Python inline flag (?i) that the schema uses.
    jsonschema::options().with_pattern_options(PatternOptions::regex())
}

#[test]
fn price_file_matches_schema() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let mut all = Vec::new();
    patterns(&schema, &mut all);
    assert!(
        all.iter().any(|p| p.contains("(?i)")),
        "the schema has no (?i) pattern to check"
    );
    for p in &all {
        if let Err(e) = options().build(&serde_json::json!({"type": "string", "pattern": p})) {
            panic!(
                "error: the schema pattern {p:?} does not compile with the regex engine of the jsonschema crate: {e}. Fix 1: pin a jsonschema version whose engine accepts the pattern. Fix 2: report the pattern upstream; do not skip or rewrite it. Reference: docs/specification/cost.md, section Validation. Verify: cargo test price_file_matches_schema passes."
            );
        }
    }
    // The inline flag must keep its meaning, not only compile.
    let day = options()
        .build(&serde_json::json!({"type": "string", "pattern": "(?i)^(mon|monday)$"}))
        .unwrap();
    assert!(day.is_valid(&serde_json::json!("MONDAY")));
    let validator = options().build(&schema).unwrap();
    let data: Value = serde_json::from_str(DATA).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&data)
        .take(10)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "error: the vendored price file does not match the vendored schema: {errors:#?}. Fix: run .agents/scripts/prices.vendor.sh to vendor a matching pair; do not edit vendor/litellm/ by hand. Verify: cargo test price_file_matches_schema passes."
    );
}

#[test]
fn read_fields_exist_in_schema() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let props = schema["$defs"]["modelEntry"]["properties"]
        .as_object()
        .expect("the schema has $defs.modelEntry.properties");
    let mut names = vec![
        "input_cost_per_token".to_string(),
        "cache_read_input_token_cost".into(),
        "cache_creation_input_token_cost".into(),
        "cache_creation_input_token_cost_above_1hr".into(),
        "output_cost_per_token".into(),
    ];
    for base in [
        "input_cost_per_token",
        "cache_read_input_token_cost",
        "cache_creation_input_token_cost",
        "output_cost_per_token",
    ] {
        names.push(format!("{base}_priority"));
        names.push(format!("{base}_flex"));
        names.push(format!("{base}_above_200k_tokens"));
    }
    for n in names {
        assert!(
            props.contains_key(&n),
            "error: the price field {n} that the cost function reads is not a property of the vendored schema. Fix: rename the field in src/price/ to the schema name. Verify: cargo test read_fields_exist_in_schema passes."
        );
    }
}
