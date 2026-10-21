//! @agents-index Model providers that the binary itself calls: the known providers, detection, base URLs, legacy names, ping, status cache.

pub mod detect;
pub mod endpoint;
pub mod legacy;
pub mod ping;
pub mod status;
pub mod worker;

use crate::config::loader::Source;
use crate::summary::credential::Credential;

/// A model provider with its own credential variables. A compatible provider is reached only
/// through a base URL override, so this list is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    Openai,
}

/// Every known provider, in the order of the selection rule and of the output lines.
pub const KNOWN: [Provider; 2] = [Provider::Anthropic, Provider::Openai];

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openai => "openai",
        }
    }

    /// The variable of the base URL; the vendor SDKs read the same name.
    pub fn base_var(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_BASE_URL",
            Provider::Openai => "OPENAI_BASE_URL",
        }
    }

    pub fn parse(name: &str) -> Option<Provider> {
        KNOWN.into_iter().find(|p| p.name() == name)
    }
}

/// One known provider as the process loaded it at start.
#[derive(Clone, Debug)]
pub struct Detected {
    pub provider: Provider,
    /// The active credential; None means the provider is not detected.
    pub credential: Option<Credential>,
    /// The base URL without a trailing slash.
    pub base_url: String,
    /// The configuration source of the base URL.
    pub source: Source,
}
