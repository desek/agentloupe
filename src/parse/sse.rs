//! @agents-index Splits a server-sent events body into the JSON values of its data fields, one per event.

use serde_json::Value;

/// The JSON data of each event; events whose data is not JSON, such as `[DONE]`, are skipped.
pub fn data_values(body: &[u8]) -> Vec<Value> {
    let text = String::from_utf8_lossy(body);
    let mut out = Vec::new();
    let mut data = String::new();
    for line in text.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Ok(v) = serde_json::from_str::<Value>(&data) {
                out.push(v);
            }
            data.clear();
        } else if let Some(d) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(d.strip_prefix(' ').unwrap_or(d));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_events_and_skips_done() {
        let v = super::data_values(b"event: a\ndata: {\"n\":1}\n\ndata: [DONE]\n\ndata: {\"n\":2}");
        assert_eq!(v.len(), 2);
        assert_eq!(v[1]["n"], 2);
    }
}
