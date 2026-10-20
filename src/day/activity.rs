//! @agents-index The activity source interface (has_activity, first_activity_day) and the calls source, the only registered source.

use super::boundary;
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::Connection;

/// A source of activity. A day gets a record only when one or more sources report activity.
/// A source that gets new data for a day also puts the day in the queue with its own name as
/// the reason.
pub trait Source: Send + Sync {
    /// The name, also the queue reason of the source.
    fn name(&self) -> &'static str;
    fn has_activity(&self, conn: &Connection, day: NaiveDate, tz: Tz) -> rusqlite::Result<bool>;
    /// The oldest day with activity, or None.
    fn first_activity_day(&self, conn: &Connection, tz: Tz) -> rusqlite::Result<Option<NaiveDate>>;
}

/// Activity from the calls table only.
pub struct Calls;

impl Source for Calls {
    fn name(&self) -> &'static str {
        "calls"
    }

    fn has_activity(&self, conn: &Connection, day: NaiveDate, tz: Tz) -> rusqlite::Result<bool> {
        let (from, to) = boundary::bounds(day, tz);
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM calls WHERE started_at >= ?1 AND started_at < ?2)",
            [from, to],
            |r| r.get(0),
        )
    }

    fn first_activity_day(&self, conn: &Connection, tz: Tz) -> rusqlite::Result<Option<NaiveDate>> {
        let first: Option<String> =
            conn.query_row("SELECT min(started_at) FROM calls", [], |r| r.get(0))?;
        Ok(first.map(|t| boundary::day_of_text(&t, tz)))
    }
}

/// The registered sources. A later source adds itself here, with no change to the publisher.
pub fn registered() -> Vec<Box<dyn Source>> {
    vec![Box::new(Calls)]
}

/// True when one or more of `sources` report activity on `day`.
pub fn any(
    sources: &[Box<dyn Source>],
    conn: &Connection,
    day: NaiveDate,
    tz: Tz,
) -> rusqlite::Result<bool> {
    for s in sources {
        if s.has_activity(conn, day, tz)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The oldest first activity day of all `sources`.
pub fn first(
    sources: &[Box<dyn Source>],
    conn: &Connection,
    tz: Tz,
) -> rusqlite::Result<Option<NaiveDate>> {
    let mut out: Option<NaiveDate> = None;
    for s in sources {
        if let Some(d) = s.first_activity_day(conn, tz)? {
            out = Some(out.map_or(d, |o| o.min(d)));
        }
    }
    Ok(out)
}
