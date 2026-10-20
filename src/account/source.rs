//! @agents-index Selects the provider account of a call from the first present source: account ID header, metadata.user_id, credential.

use crate::capture::credential::hmac_hex;
use crate::parse::metadata::user_id;
use serde_json::Value;

/// The account hash as lowercase hex and the source name stored in accounts.source.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub hash: String,
    pub source: String,
}

/// The hashes that the capture wrote on the exchange; the parser never hashes a credential again.
pub struct Stored<'a> {
    pub account_id: Option<(&'a str, &'a str)>,
    pub credential: Option<(&'a str, &'a str)>,
}

/// The credential hash is always the last source.
pub fn select(key: &[u8; 32], stored: &Stored, request: &Value) -> Option<Account> {
    if let Some((hash, name)) = stored.account_id {
        return Some(Account {
            hash: hash.into(),
            source: format!("header:{name}"),
        });
    }
    if let Some(uuid) = user_id(request).account {
        return Some(Account {
            hash: hmac_hex(key, uuid.as_bytes()),
            source: "metadata.user_id".into(),
        });
    }
    stored.credential.map(|(hash, name)| Account {
        hash: hash.into(),
        source: format!("credential:{name}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const K: [u8; 32] = [7u8; 32];

    #[test]
    fn account_source_order() {
        let req = json!({"metadata":{"user_id":"user_x_account_acc-9_session_s"}});
        let all = Stored {
            account_id: Some(("h1", "chatgpt-account-id")),
            credential: Some(("c1", "authorization")),
        };
        assert_eq!(
            select(&K, &all, &req).unwrap().source,
            "header:chatgpt-account-id"
        );
        let no_header = Stored {
            account_id: None,
            credential: Some(("c1", "authorization")),
        };
        assert_eq!(
            select(&K, &no_header, &req),
            Some(Account {
                hash: hmac_hex(&K, b"acc-9"),
                source: "metadata.user_id".into()
            })
        );
        assert_eq!(
            select(&K, &no_header, &json!({})),
            Some(Account {
                hash: "c1".into(),
                source: "credential:authorization".into()
            })
        );
        assert_eq!(
            select(
                &K,
                &Stored {
                    account_id: None,
                    credential: None
                },
                &json!({})
            ),
            None
        );
    }
}
