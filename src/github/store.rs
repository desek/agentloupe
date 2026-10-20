//! @agents-index SQL of the GitHub tables: writes one fetch in one transaction, reads the fetch status, and lists labels with no ok fetch.

use rusqlite::{Connection, OptionalExtension, params};

/// One pull request of a fetch, by hash; a merged one carries its lines.
pub struct Pr {
    pub hash: String,
    pub merged: Option<(i64, i64)>,
}

/// The status of the fetch row of `day` and `label`, or None.
pub fn status(conn: &Connection, day: &str, label: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT status FROM github_fetches WHERE day = ?1 AND label = ?2",
        params![day, label],
        |r| r.get(0),
    )
    .optional()
}

/// The labels of `labels` that have no fetch row with the status ok for `day`.
pub fn not_ok(conn: &Connection, day: &str, labels: &[String]) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::new();
    for l in labels {
        if status(conn, day, l)?.as_deref() != Some("ok") {
            out.push(l.clone());
        }
    }
    Ok(out)
}

/// Replaces the fetch row of a failed fetch; no pull request row changes.
pub fn failed(
    conn: &Connection,
    day: &str,
    label: &str,
    cause: &str,
    now: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO github_fetches (day, label, status, cause, fetched_at) VALUES (?1, ?2, 'failed', ?3, ?4)",
        params![day, label, cause, now],
    )
    .map(|_| ())
}

fn set_day(
    conn: &Connection,
    col: &str,
    hash: &str,
    day: &str,
    now: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!(
            "INSERT INTO github_prs (pr_hash, {col}, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT (pr_hash) DO UPDATE SET {col} = excluded.{col}, updated_at = excluded.updated_at \
             WHERE {col} IS NOT excluded.{col}"
        ),
        params![hash, day, now],
    )
}

fn set_lines(
    conn: &Connection,
    hash: &str,
    add: i64,
    del: i64,
    now: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE github_prs SET additions = ?2, deletions = ?3, updated_at = ?4 \
         WHERE pr_hash = ?1 AND (additions IS NOT ?2 OR deletions IS NOT ?3)",
        params![hash, add, del, now],
    )
}

/// Writes an ok fetch in one transaction: the opened and merged rows, the label rows, and the
/// fetch row. Returns true when a pull request row changed or the fetch row became ok, so the
/// day needs a new build. Rows that hold the same values are not written, so a second fetch
/// gives the same rows.
pub fn ok(
    conn: &Connection,
    day: &str,
    label: &str,
    opened: &[String],
    merged: &[Pr],
    now: &str,
) -> rusqlite::Result<bool> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let run = || -> rusqlite::Result<bool> {
        let mut changed = status(conn, day, label)?.as_deref() != Some("ok");
        for h in opened {
            changed |= set_day(conn, "opened_day", h, day, now)? > 0;
        }
        for p in merged {
            changed |= set_day(conn, "merged_day", &p.hash, day, now)? > 0;
            if let Some((a, d)) = p.merged {
                changed |= set_lines(conn, &p.hash, a, d, now)? > 0;
            }
        }
        for h in opened.iter().chain(merged.iter().map(|p| &p.hash)) {
            conn.execute(
                "INSERT OR IGNORE INTO github_pr_labels (pr_hash, label) VALUES (?1, ?2)",
                params![h, label],
            )?;
        }
        conn.execute(
            "INSERT OR REPLACE INTO github_fetches (day, label, status, cause, fetched_at) VALUES (?1, ?2, 'ok', NULL, ?3)",
            params![day, label, now],
        )?;
        Ok(changed)
    };
    match run() {
        Ok(v) => conn.execute_batch("COMMIT").map(|_| v),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
