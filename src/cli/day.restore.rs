//! @agents-index loupe day restore: builds and writes the day, clears the deletion mark, and sets published or pending_review.

use super::ConfigArgs;
use super::day_common as common;
use crate::day::state::Review;
use crate::day::{activity, overrides, rebuild, state, store};
use common::{EXIT_INPUT, fail};
use std::process::ExitCode;

const COMMAND: &str = "loupe day restore";

pub fn run(args: &ConfigArgs, date: &str, dry_run: bool) -> ExitCode {
    let (cfg, conn) = match common::open(args, dry_run, COMMAND) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let day = match common::date(Some(date), cfg.summary.tz, COMMAND) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let key = day.to_string();
    let ctx = rebuild::Ctx::from_config(&cfg);
    match activity::any(&activity::registered(), &conn, day, ctx.tz) {
        Ok(true) => {}
        Ok(false) => {
            return fail(
                EXIT_INPUT,
                format!(
                    "error: the day {key} has no activity, so it has no record to restore; the tombstone stays. Fix: pick a day with calls, for example from GET /v1/days. Reference: docs/specification/days.md. Verify: {COMMAND} --date {key} prints state: published."
                ),
            );
        }
        Err(e) => return common::db_fail(&cfg, COMMAND, e.to_string()),
    }
    let run = || -> rusqlite::Result<bool> {
        if dry_run {
            return Ok(false);
        }
        let now = common::now();
        let approved = store::read(&conn, &key)?.is_some_and(|r| r.published_at.is_some());
        overrides::clear_deleted(&conn, &key, &now)?;
        rebuild::rebuild(&conn, &ctx, day, &now, false, false)?;
        // A day that manual review never approved goes back to review.
        let to = if ctx.review == Review::Manual && !approved {
            state::PENDING_REVIEW
        } else {
            state::PUBLISHED
        };
        store::set_state(&conn, &key, to, &now)?;
        tracing::info!(day = %key, state = to, "day restored");
        Ok(true)
    };
    match run().and_then(|w| common::print_day(&conn, &key, w)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => common::db_fail(&cfg, COMMAND, e.to_string()),
    }
}
