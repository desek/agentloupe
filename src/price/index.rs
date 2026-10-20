//! @agents-index The price index: the embedded price file parsed one time into entries keyed by model name and provider.

use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::OnceLock;

/// One price entry: its key in the price file, its provider, and every numeric cost field as an exact decimal.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub key: String,
    pub provider: String,
    pub fields: BTreeMap<String, Decimal>,
}

/// Entries keyed by (model name, litellm_provider).
#[derive(Debug, Default)]
pub struct Index {
    map: HashMap<(String, String), Entry>,
}

/// The JSON number as an exact decimal; serde_json prints 1e-7 style text, so scientific text is accepted.
fn decimal(n: &serde_json::Number) -> Option<Decimal> {
    let text = n.to_string();
    if text.contains(['e', 'E']) {
        Decimal::from_scientific(&text).ok()
    } else {
        Decimal::from_str(&text).ok()
    }
}

impl Index {
    /// Parses a price file text. The key `sample_spec` is not a model and is skipped.
    pub fn parse(text: &str) -> Result<Index, String> {
        let root: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text)
            .map_err(|e| format!("the price file is not a JSON object: {e}"))?;
        let mut map = HashMap::with_capacity(root.len());
        for (key, value) in root {
            if key == "sample_spec" {
                continue;
            }
            let Some(obj) = value.as_object() else {
                continue;
            };
            let Some(provider) = obj.get("litellm_provider").and_then(|p| p.as_str()) else {
                continue;
            };
            let fields = obj
                .iter()
                .filter(|(k, _)| k.contains("cost"))
                .filter_map(|(k, v)| v.as_number().and_then(decimal).map(|d| (k.clone(), d)))
                .collect();
            let entry = Entry {
                key: key.clone(),
                provider: provider.to_string(),
                fields,
            };
            map.insert((key, provider.to_string()), entry);
        }
        Ok(Index { map })
    }

    #[cfg(test)]
    /// Builds an index from entries, for tests that need a small hand-made index.
    pub fn from_entries(entries: Vec<Entry>) -> Index {
        Index {
            map: entries
                .into_iter()
                .map(|e| ((e.key.clone(), e.provider.clone()), e))
                .collect(),
        }
    }

    /// The index of the embedded price file, parsed on the first call only.
    pub fn embedded() -> &'static Index {
        static INDEX: OnceLock<Index> = OnceLock::new();
        INDEX.get_or_init(|| {
            Index::parse(super::embed::PRICE_FILE).expect("the embedded price file is valid JSON")
        })
    }

    pub fn get(&self, model: &str, provider: &str) -> Option<&Entry> {
        self.map.get(&(model.to_string(), provider.to_string()))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[allow(dead_code, reason = "clippy pairs len with is_empty")]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
