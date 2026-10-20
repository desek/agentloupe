//! @agents-index The two pull request searches of a day: the query text, every page of 100 items, and the 1000-result limit.

use super::client::{Cause, Client};

/// The search API returns at most this many results; a larger total gives a wrong count.
pub const LIMIT: u64 = 1000;
const PER_PAGE: usize = 100;

/// One search hit: the parts of the pull request reference, in memory only.
#[derive(Debug, Clone)]
pub struct Item {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

/// `created` for the opened pull requests, `merged` for the merged ones.
pub fn query(kind: &str, login: &str, range: &str) -> String {
    format!("is:pr author:{login} {kind}:{range}")
}

/// Owner and repository from a `repository_url` such as `https://api.github.com/repos/o/r`.
fn item(v: &serde_json::Value) -> Option<Item> {
    let mut parts = v["repository_url"].as_str()?.rsplit('/');
    let repo = parts.next()?.to_string();
    let owner = parts.next()?.to_string();
    Some(Item {
        owner,
        repo,
        number: v["number"].as_u64()?,
    })
}

/// Every item of the search `q`, page by page until a page has fewer than 100 items.
pub fn all(c: &Client, q: &str) -> Result<Vec<Item>, Cause> {
    let mut out = Vec::new();
    for page in 1.. {
        let v = c.get(
            "/search/issues",
            &[
                ("q", q.to_string()),
                ("per_page", PER_PAGE.to_string()),
                ("page", page.to_string()),
            ],
        )?;
        if v["total_count"].as_u64().unwrap_or(0) > LIMIT {
            return Err(Cause::SearchLimit);
        }
        let items = v["items"].as_array().cloned().unwrap_or_default();
        out.extend(items.iter().filter_map(item));
        if items.len() < PER_PAGE {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_text_for_opened_and_merged() {
        let r = "2026-10-17T00:00:00+03:00..2026-10-17T23:59:59+03:00";
        assert_eq!(
            query("created", "u", r),
            format!("is:pr author:u created:{r}")
        );
        assert_eq!(
            query("merged", "u", r),
            format!("is:pr author:u merged:{r}")
        );
    }

    #[test]
    fn item_from_repository_url() {
        let v = serde_json::json!({"repository_url": "https://x/repos/O/R", "number": 3});
        let i = item(&v).unwrap();
        assert_eq!((i.owner.as_str(), i.repo.as_str(), i.number), ("O", "R", 3));
    }
}
