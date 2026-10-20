//! @agents-index Reads the account UUID and the session ID from metadata.user_id of a request, in its string or JSON shape.

use serde_json::Value;

/// The parts of `metadata.user_id`; a part is None when absent or empty.
#[derive(Debug, Default, PartialEq)]
pub struct UserId {
    pub account: Option<String>,
    pub session: Option<String>,
}

fn some(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

/// Two shapes exist: `user_<hex>_account_<uuid>_session_<uuid>`, and a JSON object string with
/// `account_uuid` and `session_id`.
pub fn user_id(request: &Value) -> UserId {
    let Some(raw) = request["metadata"]["user_id"].as_str() else {
        return UserId::default();
    };
    if let Ok(v) = serde_json::from_str::<Value>(raw)
        && v.is_object()
    {
        return UserId {
            account: v["account_uuid"].as_str().and_then(some),
            session: v["session_id"].as_str().and_then(some),
        };
    }
    let after = |tag: &str| raw.split_once(tag).map(|(_, rest)| rest);
    UserId {
        account: after("_account_")
            .map(|r| r.split("_session_").next().unwrap_or(""))
            .and_then(some),
        session: after("_session_").and_then(some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_both_shapes() {
        let a = user_id(&json!({"metadata":{"user_id":"user_ab12_account_acc-1_session_ses-1"}}));
        assert_eq!(
            a,
            UserId {
                account: Some("acc-1".into()),
                session: Some("ses-1".into())
            }
        );
        let b = user_id(
            &json!({"metadata":{"user_id":"{\"device_id\":\"d\",\"account_uuid\":\"acc-2\",\"session_id\":\"ses-2\"}"}}),
        );
        assert_eq!(
            b,
            UserId {
                account: Some("acc-2".into()),
                session: Some("ses-2".into())
            }
        );
        let c = user_id(&json!({"metadata":{"user_id":"user_ab12_account__session_ses-3"}}));
        assert_eq!(
            c,
            UserId {
                account: None,
                session: Some("ses-3".into())
            }
        );
        assert_eq!(user_id(&json!({})), UserId::default());
    }
}
