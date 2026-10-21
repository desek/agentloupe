//! @agents-index The session log backfill: reads Claude Code or Pi logs into calls and segments of origin log, then summarizes them.

pub mod claude;
pub mod discover;
pub mod entry;
pub mod key;
pub mod pi;
pub mod processed;
pub mod read;
pub mod run;
pub mod summary;
pub mod transcript;

/// The log source: the agent whose session logs on disk the backfill reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    ClaudeCode,
    Pi,
}

impl Source {
    pub fn parse(s: &str) -> Option<Source> {
        match s {
            "claude-code" => Some(Source::ClaudeCode),
            "pi" => Some(Source::Pi),
            _ => None,
        }
    }

    /// The name of the source; also the agent label of its calls and part of each log key.
    pub fn name(self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude-code",
            Source::Pi => "pi",
        }
    }
}
