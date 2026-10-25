//! @agents-index The day range of a GitHub search: the start and end of the day in LOUPE_TZ, each with its own UTC offset.

use crate::day::boundary;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;

fn local(ms: i64, tz: Tz) -> String {
    tz.timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
        .unwrap_or_default()
}

/// `start..end`, where the end is one second before the start of the next day.
pub fn range(day: NaiveDate, tz: Tz) -> String {
    let next = day.succ_opt().unwrap_or(day);
    format!(
        "{}..{}",
        local(boundary::start_ms(day, tz), tz),
        local(boundary::start_ms(next, tz) - 1000, tz)
    )
}

#[cfg(test)]
mod tests {
    use super::range;

    fn d(s: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn range_has_offset_of_each_end() {
        assert_eq!(
            range(d("2026-10-17"), chrono_tz::Europe::Stockholm),
            "2026-10-17T00:00:00+02:00..2026-10-17T23:59:59+02:00"
        );
    }

    #[test]
    fn range_on_offset_change_day() {
        assert_eq!(
            range(d("2026-10-25"), chrono_tz::Europe::Stockholm),
            "2026-10-25T00:00:00+02:00..2026-10-25T23:59:59+01:00"
        );
    }
}
