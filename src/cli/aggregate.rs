//! @agents-index The aggregate subcommand: builds one day now with build_day and writes it with the upsert rule, or a dry run.

use super::ConfigArgs;
use super::day_common as common;
use crate::day::{activity, rebuild, state, store};
use common::{db_fail, print};
use std::process::ExitCode;

const COMMAND: &str = "loupe aggregate";

pub fn run(args: &ConfigArgs, date: Option<&str>, dry_run: bool) -> ExitCode {
    let (cfg, conn) = match common::open(args, dry_run, COMMAND) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let day = match common::date(date, cfg.summary.tz, COMMAND) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let ctx = rebuild::Ctx::from_config(&cfg);
    let key = day.to_string();
    let run = || -> rusqlite::Result<Vec<(&'static str, String)>> {
        let active = activity::any(&activity::registered(), &conn, day, ctx.tz)?;
        let row = store::read(&conn, &key)?;
        let tombstone = row.as_ref().is_some_and(|r| r.state == state::DELETED);
        // A day with no activity and a tombstone get no write.
        let write = active && !tombstone && !dry_run;
        let b = rebuild::rebuild(&conn, &ctx, day, &common::now(), false, !write)?;
        if b.written {
            tracing::info!(day = %key, revision = b.revision, reason = "aggregate", calls = b.record.calls, sessions = b.record.sessions, unpriced_calls = b.record.unpriced_calls, "day aggregated");
        }
        Ok(vec![
            ("day", key.clone()),
            ("calls", b.record.calls.to_string()),
            ("sessions", b.record.sessions.to_string()),
            ("cost_usd", b.record.cost_usd.to_string()),
            ("unpriced_calls", b.record.unpriced_calls.to_string()),
            ("summary", common::presence(b.record.summary.is_some())),
            ("revision", b.revision.to_string()),
            ("written", b.written.to_string()),
        ])
    };
    match run() {
        Ok(lines) => {
            print(&lines);
            ExitCode::SUCCESS
        }
        Err(e) => db_fail(&cfg, COMMAND, e.to_string()),
    }
}
