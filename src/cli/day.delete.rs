//! @agents-index loupe day delete: sets the deletion mark and makes the day a tombstone, so the publisher never creates it again.

use super::ConfigArgs;
use super::day_common as common;
use crate::day::{overrides, store};
use std::process::ExitCode;

const COMMAND: &str = "loupe day delete";

pub fn run(args: &ConfigArgs, date: &str, dry_run: bool) -> ExitCode {
    let (cfg, conn) = match common::open(args, dry_run, COMMAND) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let day = match common::date(Some(date), cfg.summary.tz, COMMAND) {
        Ok(d) => d.to_string(),
        Err(code) => return code,
    };
    let run = || -> rusqlite::Result<bool> {
        if dry_run {
            return Ok(false);
        }
        let now = common::now();
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let r = overrides::set_deleted(&conn, &day, &now)
            .and_then(|_| store::tombstone(&conn, &day))
            .and_then(|_| crate::day::queue::delete(&conn, &day));
        conn.execute_batch(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        r?;
        tracing::info!(day = %day, state = "deleted", "day deleted");
        Ok(true)
    };
    match run().and_then(|w| common::print_day(&conn, &day, w)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => common::db_fail(&cfg, COMMAND, e.to_string()),
    }
}
