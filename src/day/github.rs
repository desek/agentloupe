//! @agents-index The GitHub fields of a day record: the four counts and detail.github.failed_tokens from the private GitHub tables.

use rusqlite::{Connection, params};

/// The four counts and the number of failed labels of one day.
pub struct Fields {
    pub prs_opened: i64,
    pub prs_merged: i64,
    pub lines_added: i64,
    pub lines_removed: i64,
    pub failed_tokens: i64,
}

/// None when no label is set, so the four fields stay null and the detail has no github key.
pub fn fields(conn: &Connection, day: &str, labels: &[String]) -> rusqlite::Result<Option<Fields>> {
    if labels.is_empty() {
        return Ok(None);
    }
    let prs_opened = conn.query_row(
        "SELECT count(DISTINCT pr_hash) FROM github_prs WHERE opened_day = ?1",
        params![day],
        |r| r.get(0),
    )?;
    let (prs_merged, lines_added, lines_removed) = conn.query_row(
        "SELECT count(DISTINCT pr_hash), coalesce(sum(additions), 0), coalesce(sum(deletions), 0) \
         FROM github_prs WHERE merged_day = ?1",
        params![day],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let failed_tokens = crate::github::store::not_ok(conn, day, labels)?.len() as i64;
    Ok(Some(Fields {
        prs_opened,
        prs_merged,
        lines_added,
        lines_removed,
        failed_tokens,
    }))
}
