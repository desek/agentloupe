//! @agents-index Test helper of the provider status: the test secrets, the env for a fake provider, and a wait for a status.
#![allow(dead_code)]

use super::{fake_provider, loupe, private};
use std::time::{Duration, Instant};

pub const ANTHROPIC_KEY: &str = "sk-ant-test-Qm7Vx2Lp9Rk4";
pub const OAUTH_TOKEN: &str = "sk-ant-oat-test-Hd3Nw8Zs1Fj6";
pub const OPENAI_KEY: &str = "sk-openai-test-Ty5Bc0Kg7Wq2";

/// Every test secret, for a search of a body or a log.
pub const SECRETS: [&str; 3] = [ANTHROPIC_KEY, OAUTH_TOKEN, OPENAI_KEY];

/// The two base URLs that point at `fake`, so no ping leaves the machine.
pub fn bases(fake: &fake_provider::Fake) -> (String, String) {
    (fake.url(), format!("{}/v1", fake.url()))
}

/// Waits until `GET /private/api/providers` shows `status` for `name`; panics after 10 seconds.
pub fn wait_status(l: &loupe::Loupe, name: &str, status: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (code, body) = private::get_json(l, "/private/api/providers");
        assert_eq!(code, 200);
        let now = body["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .map(|p| p["status"].clone());
        if now.as_ref().is_some_and(|s| s == status) {
            return body;
        }
        assert!(Instant::now() < deadline, "{name} is not {status}: {body}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// True when no test secret, and no 8-character part of one, is in `text`.
pub fn clean(text: &str) -> bool {
    SECRETS
        .iter()
        .all(|s| (0..=s.len() - 8).all(|i| !text.contains(&s[i..i + 8])))
}
