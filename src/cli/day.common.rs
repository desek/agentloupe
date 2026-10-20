//! @agents-index Shared parts of loupe aggregate and loupe day: the --date rule, the open of the store, key: value output, exit codes.

use super::ConfigArgs;
use crate::config::{Config, loader};
use chrono::NaiveDate;
use rusqlite::Connection;
use std::process::ExitCode;

pub const EXIT_INPUT: u8 = 2;
pub const EXIT_INTERNAL: u8 = 1;

pub fn fail(code: u8, message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

/// The day of `--date`: YYYY-MM-DD, `today` in `tz`, or with no value the previous day in `tz`.
pub fn date(text: Option<&str>, tz: chrono_tz::Tz, command: &str) -> Result<NaiveDate, ExitCode> {
    let today = crate::day::boundary::day_of(chrono::Utc::now().timestamp_millis(), tz);
    match text {
        None => Ok(today.pred_opt().unwrap_or(today)),
        Some("today") => Ok(today),
        Some(t) => crate::api::query::date(t).ok_or_else(|| {
            fail(
                EXIT_INPUT,
                format!(
                    "error: --date {t} is not a valid YYYY-MM-DD date and not today. Fix 1: pass --date 2026-10-19. Fix 2: pass --date today. Reference: docs/specification/days.md. Verify: {command} --date 2026-10-19 --dry-run prints day: 2026-10-19."
                ),
            )
        }),
    }
}

/// Loads the configuration, starts the log, and opens and migrates the store (no migration on a
/// dry run). An invalid LOUPE_TZ or other value stops here with exit 2.
pub fn open(
    args: &ConfigArgs,
    dry_run: bool,
    command: &str,
) -> Result<(Config, Connection), ExitCode> {
    let cfg = loader::load_from_process(&args.as_flags())
        .and_then(|c| c.validate())
        .map_err(|e| fail(EXIT_INPUT, e.to_string()))?;
    crate::log::init(&cfg.data_dir).map_err(|e| {
        fail(
            EXIT_INTERNAL,
            format!(
                "error: cannot write the log file under {}: {e}. Fix: make LOUPE_DATA_DIR writable. Verify: {command} --dry-run exits 0.",
                cfg.data_dir.display()
            ),
        )
    })?;
    let mut conn = crate::store::open::open(&cfg.data_dir)
        .map_err(|e| db_fail(&cfg, command, e.to_string()))?;
    if !dry_run {
        crate::store::migrate::migrate(&mut conn)
            .map_err(|e| db_fail(&cfg, command, e.to_string()))?;
    }
    Ok((cfg, conn))
}

/// The internal error of a database step.
pub fn db_fail(cfg: &Config, command: &str, e: String) -> ExitCode {
    fail(
        EXIT_INTERNAL,
        format!(
            "error: {command} on the database under {} failed: {e}. Fix 1: start loupe serve one time so that the database exists and is migrated. Fix 2: point LOUPE_DATA_DIR or --data-dir at the data dir of loupe serve. Reference: docs/specification/days.md. Verify: {command} --dry-run exits 0.",
            cfg.data_dir.display()
        ),
    )
}

pub fn print(pairs: &[(&str, String)]) {
    for (k, v) in pairs {
        println!("{k}: {v}");
    }
}

/// `present` or `absent`; the summary text is never printed.
pub fn presence(summary: bool) -> String {
    if summary { "present" } else { "absent" }.into()
}

/// True when the stored record of the day has a summary.
pub fn stored_summary(row: Option<&crate::day::store::Row>) -> bool {
    row.and_then(|r| r.record.as_deref())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .is_some_and(|v| !v["summary"].is_null())
}

/// The lines of a loupe day command, read from the store after the command.
pub fn print_day(conn: &Connection, day: &str, written: bool) -> rusqlite::Result<()> {
    let row = crate::day::store::read(conn, day)?;
    print(&[
        ("day", day.to_string()),
        (
            "state",
            row.as_ref().map_or("none".into(), |r| r.state.clone()),
        ),
        ("summary", presence(stored_summary(row.as_ref()))),
        ("written", written.to_string()),
    ]);
    Ok(())
}

pub fn now() -> String {
    crate::session::time::text(chrono::Utc::now().timestamp_millis())
}
