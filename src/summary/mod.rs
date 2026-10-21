//! @agents-index Session summaries: the transcript, the rule text, the credential, the client, the scrubber, the worker, the session texts, and the day line.

pub mod body;
pub mod client;
pub mod credential;
pub mod day;
pub mod headers;
pub mod retry;
pub mod rules;
pub mod scrub;
pub mod session_day;
pub mod store;
#[path = "store.session_day.rs"]
pub mod store_session_day;
pub mod transcript;
#[path = "transcript.proxied.rs"]
pub mod transcript_proxied;
pub mod worker;

/// The worker settings from the configuration: a client only when a credential is set.
pub fn settings(cfg: &crate::config::Config) -> worker::Settings {
    let s = &cfg.summary;
    worker::Settings {
        data_key: cfg.data_key,
        client: s
            .credential
            .clone()
            .map(|c| client::Client::new(&s.base_url, &s.model, c)),
        public: s.public_projects.clone(),
        max_attempts: s.max_attempts,
        tz: s.tz,
        min_conversation: s.min_conversation_chars,
    }
}
