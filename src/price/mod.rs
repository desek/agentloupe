//! @agents-index The API-equivalent cost: the embedded price file, its index, the lookup, the tiers, and the pure cost function.
//!
//! This module uses only `super::` paths and external crates, so integration tests can include it with `#[path]`.

pub mod compute;
pub mod embed;
pub mod index;
pub mod lookup;
pub mod tier;
pub mod unpriced;
