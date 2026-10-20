//! @agents-index loupe day approve: sets a pending_review day to published; refuses a tombstone and a day with no row.

use super::ConfigArgs;
use super::day_common as common;
use crate::day::{state, store};
use common::{EXIT_INPUT, fail};
use std::process::ExitCode;

const COMMAND: &str = "loupe day approve";

pub fn run(args: &ConfigArgs, date: &str, dry_run: bool) -> ExitCode {
    let (cfg, conn) = match common::open(args, dry_run, COMMAND) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let day = match common::date(Some(date), cfg.summary.tz, COMMAND) {
        Ok(d) => d.to_string(),
        Err(code) => return code,
    };
    let row = match store::read(&conn, &day) {
        Ok(r) => r,
        Err(e) => return common::db_fail(&cfg, COMMAND, e.to_string()),
    };
    let written = match row.as_ref().map(|r| r.state.as_str()) {
        None => {
            return fail(
                EXIT_INPUT,
                format!(
                    "error: the day {day} has no record, so it cannot be approved. Fix: run loupe aggregate --date {day} to build it, or wait until it is due. Reference: docs/specification/days.md. Verify: {COMMAND} --date {day} prints state: published."
                ),
            );
        }
        Some(state::DELETED) => {
            return fail(
                EXIT_INPUT,
                format!(
                    "error: the day {day} is deleted, so it cannot be approved. Fix: run loupe day restore --date {day}. Reference: docs/specification/days.md. Verify: {COMMAND} --date {day} prints state: published."
                ),
            );
        }
        Some(state::PENDING_REVIEW) if !dry_run => {
            if let Err(e) = store::set_state(&conn, &day, state::PUBLISHED, &common::now()) {
                return common::db_fail(&cfg, COMMAND, e.to_string());
            }
            tracing::info!(day = %day, state = state::PUBLISHED, "day approved");
            true
        }
        _ => false,
    };
    match common::print_day(&conn, &day, written) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => common::db_fail(&cfg, COMMAND, e.to_string()),
    }
}
