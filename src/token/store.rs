//! @agents-index The proxy_tokens store: name and tag rules, create, list, revoke, and the rows that the token gate reads.

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;

/// The name of the bootstrap token; `loupe token create` refuses it.
pub const BOOTSTRAP: &str = "bootstrap";
const MAX_TAGS: usize = 16;

/// The current time as stored: UTC, RFC 3339, milliseconds.
pub fn now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// A store failure, with the exit code that the token subcommands return for it.
#[derive(Debug)]
pub struct StoreError {
    pub code: u8,
    pub message: String,
}

impl StoreError {
    fn input(message: String) -> Self {
        StoreError { code: 2, message }
    }
    fn db(e: rusqlite::Error) -> Self {
        StoreError {
            code: 1,
            message: format!(
                "error: the proxy_tokens table cannot be read or written: {e}. Fix: check that <LOUPE_DATA_DIR>/loupe.db is writable and not locked for a long time. Reference: docs/specification/proxy.tokens.md. Verify: loupe token list exits 0."
            ),
        }
    }
}

/// The token name rule `^[a-z0-9][a-z0-9-]{0,31}$`.
pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn valid_key(k: &str) -> bool {
    let b = k.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'.' | b'-')
        })
}

fn valid_value(v: &str) -> bool {
    let n = v.chars().count();
    (1..=128).contains(&n) && v.chars().all(|c| !c.is_control())
}

/// Checks a name for `loupe token create`; the bootstrap name is reserved.
pub fn check_name(name: &str) -> Result<(), StoreError> {
    if !valid_name(name) {
        return Err(StoreError::input(format!(
            "error: the token name {name:?} is not valid. Fix: use a name that matches ^[a-z0-9][a-z0-9-]{{0,31}}$, for example laptop. Reference: docs/specification/proxy.tokens.md, section Names and tags. Verify: loupe token create --name <name> --dry-run exits 0."
        )));
    }
    if name == BOOTSTRAP {
        return Err(StoreError::input("error: the token name bootstrap is reserved for the bootstrap token. Fix 1: select another name. Fix 2: set LOUPE_BOOTSTRAP_TOKEN to a value from loupe token generate and restart loupe serve. Reference: docs/specification/proxy.tokens.md, section Bootstrap token. Verify: loupe token list.".into()));
    }
    Ok(())
}

/// Parses `KEY=VALUE` tags into a map; refuses a bad key, a bad value, a repeated key, or more than 16.
pub fn parse_tags(tags: &[String]) -> Result<BTreeMap<String, String>, StoreError> {
    let fail = |why: String| {
        StoreError::input(format!(
            "error: {why}. Fix: pass each tag as --tag KEY=VALUE, with a key that matches ^[a-z0-9][a-z0-9_.-]{{0,31}}$, a value of 1 to 128 printable characters, each key once, and at most {MAX_TAGS} tags. Reference: docs/specification/proxy.tokens.md, section Names and tags. Verify: loupe token create --name <name> --tag KEY=VALUE --dry-run exits 0."
        ))
    };
    if tags.len() > MAX_TAGS {
        return Err(fail(format!("{} tags were given", tags.len())));
    }
    let mut map = BTreeMap::new();
    for t in tags {
        let Some((k, v)) = t.split_once('=') else {
            return Err(fail(format!("the tag {t:?} has no =")));
        };
        if !valid_key(k) {
            return Err(fail(format!("the tag key {k:?} is not valid")));
        }
        if !valid_value(v) {
            return Err(fail(format!("the value of the tag key {k:?} is not valid")));
        }
        if map.insert(k.to_string(), v.to_string()).is_some() {
            return Err(fail(format!("the tag key {k:?} is repeated")));
        }
    }
    Ok(map)
}

/// True if a row with this name exists, active or revoked.
pub fn exists(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT count(*) > 0 FROM proxy_tokens WHERE name = ?1",
        [name],
        |r| r.get(0),
    )
    .map_err(StoreError::db)
}

fn name_taken(name: &str) -> StoreError {
    StoreError {
        code: 1,
        message: format!(
            "error: a token with the name {name} exists, active or revoked, and a name is never reused. Fix 1: select another name. Fix 2: revoke the token with loupe token revoke {name}, then create the new token under another name. Reference: docs/specification/proxy.tokens.md, section Names and tags. Verify: loupe token list."
        ),
    }
}

/// Stores a new active token by its hash; returns its `created_at`. The raw value never reaches this function.
pub fn create(
    conn: &Connection,
    name: &str,
    tags: &BTreeMap<String, String>,
    token_hash: &str,
) -> Result<String, StoreError> {
    if exists(conn, name)? {
        return Err(name_taken(name));
    }
    let created = now();
    conn.execute(
        "INSERT INTO proxy_tokens (name, token_hash, tags, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            name,
            token_hash,
            serde_json::to_string(tags).expect("a map of strings"),
            created
        ],
    )
    .map_err(StoreError::db)?;
    Ok(created)
}

/// One token as `loupe token list` shows it; it has no raw value and no hash.
#[derive(Debug, serde::Serialize)]
pub struct Listed {
    pub name: String,
    pub tags: serde_json::Value,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
    pub status: &'static str,
}

/// Every token in the order of creation.
pub fn list(conn: &Connection) -> Result<Vec<Listed>, StoreError> {
    let mut st = conn
        .prepare(
            "SELECT name, tags, created_at, last_used_at, revoked_at FROM proxy_tokens ORDER BY id",
        )
        .map_err(StoreError::db)?;
    st.query_map([], |r| {
        let tags: String = r.get(1)?;
        let revoked_at: Option<String> = r.get(4)?;
        Ok(Listed {
            name: r.get(0)?,
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            created_at: r.get(2)?,
            last_used_at: r.get(3)?,
            status: if revoked_at.is_some() {
                "revoked"
            } else {
                "active"
            },
            revoked_at,
        })
    })
    .and_then(|rows| rows.collect())
    .map_err(StoreError::db)
}

/// The `revoked_at` of a name: outer None if unknown, inner None if active.
pub fn revoked_at(conn: &Connection, name: &str) -> Result<Option<Option<String>>, StoreError> {
    conn.query_row(
        "SELECT revoked_at FROM proxy_tokens WHERE name = ?1",
        [name],
        |r| r.get(0),
    )
    .optional()
    .map_err(StoreError::db)
}

/// The error for a name that has no row.
pub fn unknown(name: &str) -> StoreError {
    StoreError {
        code: 1,
        message: format!(
            "error: no token has the name {name}. Fix: run loupe token list and use a name that it shows. Reference: docs/specification/proxy.tokens.md, section Subcommands. Verify: loupe token list shows the token as revoked."
        ),
    }
}

/// Sets `revoked_at` once; a second revoke keeps the first time. Returns the time and whether it changed.
pub fn revoke(conn: &Connection, name: &str) -> Result<(String, bool), StoreError> {
    match revoked_at(conn, name)? {
        None => Err(unknown(name)),
        Some(Some(at)) => Ok((at, false)),
        Some(None) => {
            let at = now();
            conn.execute(
                "UPDATE proxy_tokens SET revoked_at = ?1 WHERE name = ?2 AND revoked_at IS NULL",
                params![at, name],
            )
            .map_err(StoreError::db)?;
            Ok((at, true))
        }
    }
}

/// One row as the token gate needs it.
#[derive(Debug, Clone)]
pub struct GateRow {
    pub id: i64,
    pub name: String,
    pub token_hash: String,
    pub revoked: bool,
}

/// Every row for the token gate.
pub fn gate_rows(conn: &Connection) -> rusqlite::Result<Vec<GateRow>> {
    let mut st = conn
        .prepare_cached("SELECT id, name, token_hash, revoked_at IS NOT NULL FROM proxy_tokens")?;
    st.query_map([], |r| {
        Ok(GateRow {
            id: r.get(0)?,
            name: r.get(1)?,
            token_hash: r.get(2)?,
            revoked: r.get(3)?,
        })
    })?
    .collect()
}

/// Sets `last_used_at` of one token to now.
pub fn touch(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE proxy_tokens SET last_used_at = ?1 WHERE id = ?2",
        params![now(), id],
    )
    .map(|_| ())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::token::format;

    /// An open, migrated store in a temp dir.
    pub fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::store::open::open(dir.path()).unwrap();
        crate::store::migrate::migrate(&mut conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn create_stores_hash_only() {
        let (_d, conn) = db();
        let raw = format::generate();
        create(&conn, "laptop", &BTreeMap::new(), &format::hash(&raw)).unwrap();
        let (h, all): (String, String) = conn
            .query_row(
                "SELECT token_hash, name || tags || created_at || token_hash FROM proxy_tokens",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(h, format::hash(&raw));
        assert_eq!(h.len(), 64);
        assert!(!all.contains(&raw) && !all.contains("lpt_"));
    }

    #[test]
    fn duplicate_name_is_refused() {
        let (_d, conn) = db();
        let new = || format::hash(&format::generate());
        create(&conn, "ci", &BTreeMap::new(), &new()).unwrap();
        let e = create(&conn, "ci", &BTreeMap::new(), &new()).unwrap_err();
        assert!(e.message.contains("loupe token list"));
        revoke(&conn, "ci").unwrap();
        let e = create(&conn, "ci", &BTreeMap::new(), &new()).unwrap_err();
        assert!(e.message.contains("loupe token list") && e.code != 0);
        assert!(check_name(BOOTSTRAP).is_err());
    }

    #[test]
    fn tag_rules_hold() {
        let many: Vec<String> = (0..17).map(|i| format!("k{i}=v")).collect();
        assert!(parse_tags(&many).is_err());
        assert!(parse_tags(&many[..16]).is_ok());
        for bad in [
            vec!["a=1".to_string(), "a=2".to_string()],
            vec!["Bad=1".to_string()],
            vec!["k=".to_string()],
            vec!["k".to_string()],
            vec!["k=a\u{7}".to_string()],
        ] {
            assert!(parse_tags(&bad).is_err(), "{bad:?}");
        }
        let ok = parse_tags(&["team.x_y-z=Ops one".to_string()]).unwrap();
        assert_eq!(ok["team.x_y-z"], "Ops one");
    }

    #[test]
    fn revoke_is_idempotent() {
        let (_d, conn) = db();
        create(
            &conn,
            "a",
            &BTreeMap::new(),
            &format::hash(&format::generate()),
        )
        .unwrap();
        let (first, changed) = revoke(&conn, "a").unwrap();
        assert!(changed);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let (second, changed) = revoke(&conn, "a").unwrap();
        assert!(!changed);
        assert_eq!(first, second);
        assert!(
            revoke(&conn, "nope")
                .unwrap_err()
                .message
                .contains("loupe token list")
        );
    }
}
