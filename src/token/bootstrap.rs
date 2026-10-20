//! @agents-index The bootstrap step of loupe serve: makes, replaces, or revokes the token named bootstrap from LOUPE_BOOTSTRAP_TOKEN.

use super::{format, store};
use rusqlite::{Connection, OptionalExtension, params};

/// What the step did; the start logs it, never the value.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Created,
    Replaced,
    Revoked,
    Unchanged,
}

/// Runs once before the port opens. `raw` was validated by the configuration; it is checked again here.
/// The step is idempotent: the same input a second time changes nothing.
pub fn run(conn: &Connection, raw: Option<&str>) -> Result<Outcome, String> {
    let row: Option<(String, bool)> = conn
        .query_row(
            "SELECT token_hash, revoked_at IS NOT NULL FROM proxy_tokens WHERE name = ?1",
            [store::BOOTSTRAP],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let exec = |sql: &str, p: &[&dyn rusqlite::ToSql]| {
        conn.execute(sql, p).map(|_| ()).map_err(|e| e.to_string())
    };
    match (raw, row) {
        (Some(raw), row) => {
            format::parse(raw).map_err(|why| format!("LOUPE_BOOTSTRAP_TOKEN {why}"))?;
            let h = format::hash(raw);
            match row {
                None => {
                    exec(
                        "INSERT INTO proxy_tokens (name, token_hash, tags, created_at) VALUES (?1, ?2, '{}', ?3)",
                        params![store::BOOTSTRAP, h, store::now()],
                    )?;
                    Ok(Outcome::Created)
                }
                Some((old, revoked)) if old != h || revoked => {
                    exec(
                        "UPDATE proxy_tokens SET token_hash = ?1, revoked_at = NULL WHERE name = ?2",
                        params![h, store::BOOTSTRAP],
                    )?;
                    Ok(Outcome::Replaced)
                }
                Some(_) => Ok(Outcome::Unchanged),
            }
        }
        (None, Some((_, false))) => {
            exec(
                "UPDATE proxy_tokens SET revoked_at = ?1 WHERE name = ?2",
                params![store::now(), store::BOOTSTRAP],
            )?;
            Ok(Outcome::Revoked)
        }
        (None, _) => Ok(Outcome::Unchanged),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_create_replace_revoke() {
        let (_d, conn) = store::tests::db();
        let a = format::generate();
        let b = format::generate();
        let row = || -> (i64, String, Option<String>) {
            conn.query_row(
                "SELECT id, token_hash, revoked_at FROM proxy_tokens WHERE name = 'bootstrap'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };
        assert_eq!(run(&conn, Some(&a)).unwrap(), Outcome::Created);
        let first = row();
        assert_eq!(run(&conn, Some(&a)).unwrap(), Outcome::Unchanged);
        assert_eq!(row(), first);
        assert_eq!(run(&conn, Some(&b)).unwrap(), Outcome::Replaced);
        let (id, h, rev) = row();
        assert_eq!(
            (id, h.as_str(), rev),
            (first.0, format::hash(&b).as_str(), None)
        );
        assert_eq!(run(&conn, None).unwrap(), Outcome::Revoked);
        assert!(row().2.is_some());
        let revoked = row();
        assert_eq!(run(&conn, None).unwrap(), Outcome::Unchanged);
        assert_eq!(row(), revoked);
        assert_eq!(run(&conn, Some(&a)).unwrap(), Outcome::Replaced);
        assert_eq!(row(), (first.0, format::hash(&a), None));
        assert!(run(&conn, Some("lpt_short")).is_err());
    }
}
