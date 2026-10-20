//! @agents-index Integration test: the embedded price file holds the required model under its provider with every required price field.

const DATA: &str = include_str!("../vendor/litellm/model_prices_and_context_window.json");

/// The models that must have a complete entry, as (model, provider). The vendor script holds the same list.
const REQUIRED: &[(&str, &str)] = &[("claude-haiku-5-5", "anthropic")];

const FIELDS: &[&str] = &[
    "input_cost_per_token",
    "output_cost_per_token",
    "cache_read_input_token_cost",
    "cache_creation_input_token_cost",
    "cache_creation_input_token_cost_above_1hr",
];

#[test]
fn required_model_entry_is_complete() {
    let data: serde_json::Value = serde_json::from_str(DATA).unwrap();
    for (model, provider) in REQUIRED {
        let entry = &data[model];
        let fix = format!(
            "Fix: run .agents/scripts/prices.vendor.sh --ref <commit> with a LiteLLM commit that holds a complete entry for {model}, then rebuild. Reference: docs/specification/cost.md, section Required model. Verify: cargo test --test price_required passes."
        );
        assert_eq!(
            entry["litellm_provider"].as_str(),
            Some(*provider),
            "error: the embedded price file has no entry {model} with litellm_provider {provider}. {fix}"
        );
        for field in FIELDS {
            assert!(
                entry[field].is_number(),
                "error: the entry {model} has no price field {field}. {fix}"
            );
        }
    }
}
