//! @agents-index The parser: finds model calls in stored exchanges by format, stores calls, accounts, and session segments.

#[path = "anthropic.json.rs"]
pub mod anthropic_json;
#[path = "anthropic.sse.rs"]
pub mod anthropic_sse;
pub mod backfill;
pub mod call;
pub mod cost;
pub mod cursor;
pub mod exchange;
pub mod format;
pub mod metadata;
#[path = "openai.chat.rs"]
pub mod openai_chat;
#[path = "openai.counts.rs"]
pub mod openai_counts;
#[path = "openai.responses.rs"]
pub mod openai_responses;
pub mod outcome;
pub mod sse;
pub mod usage;
pub mod worker;
