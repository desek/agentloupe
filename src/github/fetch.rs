//! @agents-index One GitHub fetch for one day and one label: login, two searches, the lines of each merged pull request, then the store.

use super::client::{Cause, Client};
use super::store::{self, Pr};
use super::{Settings, hash, pull, range, search, user};
use crate::config::secret::Secret;
use crate::day::queue;
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::Connection;

/// The queue reason of the GitHub source.
pub const REASON: &str = "github";

/// The result of one fetch.
#[derive(Debug, PartialEq)]
pub struct Fetched {
    pub ok: bool,
    pub cause: Option<String>,
    /// A row changed, so the day is in the queue with the reason github.
    pub changed: bool,
}

/// The network part: every name, number, and the login stay in this function.
fn remote(
    s: &Settings,
    token: &Secret,
    day: NaiveDate,
    tz: Tz,
) -> Result<(Vec<String>, Vec<Pr>), Cause> {
    let c = Client::new(&s.api_url, token.expose());
    let login = user::login(&c)?;
    let r = range::range(day, tz);
    let key = &s.account_key;
    let opened = search::all(&c, &search::query("created", &login, &r))?
        .iter()
        .map(|i| hash::pr_hash(key, &i.owner, &i.repo, i.number))
        .collect();
    let mut merged = Vec::new();
    for i in search::all(&c, &search::query("merged", &login, &r))? {
        merged.push(Pr {
            hash: hash::pr_hash(key, &i.owner, &i.repo, i.number),
            merged: Some(pull::lines(&c, &i)?),
        });
    }
    Ok((opened, merged))
}

fn warn(label: &str, day: &str, cause: &str) {
    tracing::warn!(
        github_label = label,
        day,
        cause,
        "the GitHub fetch failed. Fix 1: renew the token of LOUPE_GITHUB_TOKEN_{label}, or approve it for the organisation. Fix 2: wait for the rate limit, then retry. Then run loupe aggregate --date {day}. Reference: docs/guide/github.md, section GitHub, for the token permissions. Verify: github_fetches has status ok for the day and the label"
    );
}

/// Fetches `day` for one label and writes the result. A failure writes only the fetch row.
pub fn fetch_label(
    conn: &Connection,
    s: &Settings,
    label: &str,
    token: &Secret,
    day: NaiveDate,
    tz: Tz,
    now: &str,
) -> rusqlite::Result<Fetched> {
    let key = day.to_string();
    match remote(s, token, day, tz) {
        Ok((opened, merged)) => {
            let changed = store::ok(conn, &key, label, &opened, &merged, now)?;
            if changed {
                queue::put(conn, &key, REASON, now)?;
            }
            Ok(Fetched {
                ok: true,
                cause: None,
                changed,
            })
        }
        Err(cause) => {
            let text = cause.text();
            store::failed(conn, &key, label, &text, now)?;
            warn(label, &key, &text);
            Ok(Fetched {
                ok: false,
                cause: Some(text),
                changed: false,
            })
        }
    }
}

/// Fetches `day` for each label; a failure of one label does not stop the others.
pub fn fetch_all(
    conn: &Connection,
    s: &Settings,
    day: NaiveDate,
    tz: Tz,
    now: &str,
) -> rusqlite::Result<Vec<Fetched>> {
    s.tokens
        .iter()
        .map(|(label, token)| fetch_label(conn, s, label, token, day, tz, now))
        .collect()
}
