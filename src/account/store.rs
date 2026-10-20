//! @agents-index SQL of the accounts table: insert a (hash, source) row or widen its first and last seen times.

use super::source::Account;
use rusqlite::{Connection, params};

/// The id of the account row, made on its first call.
pub fn upsert(conn: &Connection, a: &Account, at: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "INSERT INTO accounts (hash, source, first_seen, last_seen) VALUES (?1, ?2, ?3, ?3) \
         ON CONFLICT (hash, source) DO UPDATE SET first_seen = min(first_seen, excluded.first_seen), last_seen = max(last_seen, excluded.last_seen) \
         RETURNING id",
        params![a.hash, a.source, at],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_row_per_hash_and_source() {
        let c = crate::session::segment::tests::db();
        let a = Account {
            hash: "h".into(),
            source: "metadata.user_id".into(),
        };
        let id = upsert(&c, &a, "2026-01-02T00:00:00.000Z").unwrap();
        assert_eq!(upsert(&c, &a, "2026-01-01T00:00:00.000Z").unwrap(), id);
        let seen: (String, String) = c
            .query_row("SELECT first_seen, last_seen FROM accounts", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            seen,
            (
                "2026-01-01T00:00:00.000Z".into(),
                "2026-01-02T00:00:00.000Z".into()
            )
        );
    }
}
