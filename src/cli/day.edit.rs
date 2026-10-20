//! @agents-index loupe day edit --summary TEXT: writes the override summary and builds the day again when it has a record.

use super::ConfigArgs;
use super::day_common as common;
use crate::day::{overrides, rebuild, state, store};
use common::{EXIT_INPUT, fail};
use std::process::ExitCode;

const COMMAND: &str = "loupe day edit";

pub fn run(args: &ConfigArgs, date: &str, summary: &str, dry_run: bool) -> ExitCode {
    if summary.trim().is_empty() {
        return fail(
            EXIT_INPUT,
            format!(
                "error: --summary is empty. Fix: pass --summary \"Worked on the parser.\" with the text of the day. Reference: docs/specification/days.md. Verify: {COMMAND} --date 2026-10-19 --summary TEXT prints summary: present."
            ),
        );
    }
    let (cfg, conn) = match common::open(args, dry_run, COMMAND) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let day = match common::date(Some(date), cfg.summary.tz, COMMAND) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let key = day.to_string();
    let run = || -> rusqlite::Result<bool> {
        if dry_run {
            return Ok(false);
        }
        let now = common::now();
        overrides::set_summary(&conn, &key, summary, &now)?;
        let row = store::read(&conn, &key)?;
        // With no record (or a tombstone) only the override is written; the next build uses it.
        if row.is_none_or(|r| r.state == state::DELETED) {
            return Ok(true);
        }
        let ctx = rebuild::Ctx::from_config(&cfg);
        rebuild::rebuild(&conn, &ctx, day, &now, false, false)?;
        tracing::info!(day = %key, "day summary edited");
        Ok(true)
    };
    match run().and_then(|w| common::print_day(&conn, &key, w)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => common::db_fail(&cfg, COMMAND, e.to_string()),
    }
}
