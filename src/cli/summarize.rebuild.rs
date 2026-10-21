//! @agents-index loupe summarize --rebuild-days FROM..TO: resets the session texts of a day range to pending, then rebuilds them and the day lines.

use crate::summary::{day, session_day, store_session_day, worker};
use chrono::NaiveDate;
use rusqlite::Connection;

/// The actionable error of a missing or bad range; the caller exits with code 2.
pub fn bad_range(text: &str) -> String {
    format!(
        "error: --rebuild-days {text} is not a day range. The valid form is YYYY-MM-DD..YYYY-MM-DD, FROM on or before TO, days in LOUPE_TZ. Fix 1: pass --rebuild-days 2026-10-20..2026-10-21. Fix 2: pass one day as --rebuild-days 2026-10-20..2026-10-20. Reference: docs/specification/summaries.md, section loupe summarize. Verify: the command prints summaries_reset."
    )
}

/// The inclusive range of FROM..TO; None when the form is wrong or FROM is after TO.
pub fn parse(text: &str) -> Option<(NaiveDate, NaiveDate)> {
    let (a, b) = text.split_once("..")?;
    let from = NaiveDate::parse_from_str(a, "%Y-%m-%d").ok()?;
    let to = NaiveDate::parse_from_str(b, "%Y-%m-%d").ok()?;
    (from <= to).then_some((from, to))
}

/// Resets the rows of the range, then (not in a dry run) runs the proxied session texts and rebuilds
/// the day line of each day. Backfilled rows stay pending until the next backfill run reads the logs.
/// Returns the count of rows made or reset.
pub fn run(
    conn: &Connection,
    s: &worker::Settings,
    range: (NaiveDate, NaiveDate),
    dry_run: bool,
) -> rusqlite::Result<usize> {
    let now = crate::session::time::text(chrono::Utc::now().timestamp_millis());
    let n = store_session_day::reset_days(conn, s.tz, range.0, range.1, &now, dry_run)?;
    if dry_run {
        return Ok(n);
    }
    let rt = worker::runtime()?;
    let src = session_day::Proxied {
        conn,
        data_key: s.data_key,
    };
    session_day::pass(conn, s, &rt, &src, None)?;
    let ds = day::Settings {
        client: s.client.as_ref(),
        public: &s.public,
        max_attempts: s.max_attempts,
        tz: s.tz,
    };
    let mut d = range.0;
    while d <= range.1 {
        day::build_day_line(conn, &ds, d, false)?;
        d = d.succ_opt().unwrap_or(d + chrono::Days::new(1));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_only_the_valid_form() {
        assert!(super::parse("2026-10-20..2026-10-21").is_some());
        for bad in [
            "yesterday",
            "2026-10-21..2026-10-20",
            "2026-10-20",
            "2026-10-20..x",
        ] {
            assert!(super::parse(bad).is_none(), "{bad}");
        }
    }
}
