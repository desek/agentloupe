//! @agents-index Gets the added and removed lines of one merged pull request from the pull request detail endpoint.

use super::client::{Cause, Client};
use super::search::Item;

/// (additions, deletions) of `item`.
pub fn lines(c: &Client, item: &Item) -> Result<(i64, i64), Cause> {
    let v = c.get(
        &format!("/repos/{}/{}/pulls/{}", item.owner, item.repo, item.number),
        &[],
    )?;
    Ok((
        v["additions"].as_i64().unwrap_or(0),
        v["deletions"].as_i64().unwrap_or(0),
    ))
}
