//! @agents-index The GitHub activity source: a day has activity when a pull request was opened or merged on it.

use crate::day::activity::Source;
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::Connection;

/// Reads only the pull request table.
pub struct Github;

impl Source for Github {
    fn name(&self) -> &'static str {
        super::fetch::REASON
    }

    fn has_activity(&self, conn: &Connection, day: NaiveDate, _: Tz) -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM github_prs WHERE opened_day = ?1 OR merged_day = ?1)",
            [day.to_string()],
            |r| r.get(0),
        )
    }

    fn first_activity_day(&self, conn: &Connection, _: Tz) -> rusqlite::Result<Option<NaiveDate>> {
        let first: Option<String> = conn.query_row(
            "SELECT min(d) FROM (SELECT opened_day AS d FROM github_prs UNION ALL SELECT merged_day FROM github_prs)",
            [],
            |r| r.get(0),
        )?;
        Ok(first.and_then(|t| NaiveDate::parse_from_str(&t, "%Y-%m-%d").ok()))
    }
}
