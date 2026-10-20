//! @agents-index The GitHub worker: the client, the day range, the pull request hash, the fetch and its store, the schedule, and the activity source.

pub mod client;
pub mod fetch;
pub mod hash;
pub mod pull;
pub mod range;
pub mod refetch;
pub mod retry;
pub mod scan;
pub mod search;
pub mod source;
pub mod store;
pub mod user;

use crate::config::secret::Secret;

/// What every fetch needs. The token values stay in memory only.
#[derive(Clone)]
pub struct Settings {
    pub api_url: String,
    pub account_key: [u8; 32],
    /// Each label in uppercase with its token.
    pub tokens: Vec<(String, Secret)>,
}

impl Settings {
    /// None when no token is set: the worker does not start and calls no network.
    pub fn from_config(cfg: &crate::config::Config) -> Option<Settings> {
        (!cfg.github.tokens.is_empty()).then(|| Settings {
            api_url: cfg.github.api_url.clone(),
            account_key: cfg.account_key,
            tokens: cfg.github.tokens.clone(),
        })
    }

    pub fn labels(&self) -> Vec<String> {
        self.tokens.iter().map(|(l, _)| l.clone()).collect()
    }
}
