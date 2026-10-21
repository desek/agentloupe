//! @agents-index The summarize subcommand: --once, --day (build_day_line), --retry, --rebuild-days, each with --dry-run.

use super::ConfigArgs;
use crate::config::loader;
use crate::summary::{day, store, worker};
use std::io::IsTerminal;
use std::process::ExitCode;

const EXIT_CONFIG: u8 = 2;
const EXIT_RUNTIME: u8 = 1;

/// The selected mode; exactly one of the four flags.
pub enum Mode {
    Once,
    Day(String),
    Retry,
    /// The FROM..TO text of --rebuild-days.
    Rebuild(String),
}

fn fail(code: u8, message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

fn print(pairs: &[(&str, String)], human: bool) {
    if human || std::io::stdout().is_terminal() {
        let text: Vec<String> = pairs.iter().map(|(k, v)| format!("{k} {v}")).collect();
        println!("The summarize run finished: {}.", text.join(", "));
    } else {
        for (k, v) in pairs {
            println!("{k}: {v}");
        }
    }
}

pub fn run(
    args: &ConfigArgs,
    mode: Option<Mode>,
    dry_run: bool,
    show_line: bool,
    human: bool,
) -> ExitCode {
    let Some(mode) = mode else {
        return fail(EXIT_CONFIG, "error: loupe summarize needs one mode. Fix 1: run loupe summarize --once. Fix 2: run loupe summarize --day YYYY-MM-DD. Fix 3: run loupe summarize --retry. Fix 4: run loupe summarize --rebuild-days YYYY-MM-DD..YYYY-MM-DD. Verify: the command prints key: value lines.".into());
    };
    if let Mode::Rebuild(text) = &mode
        && super::summarize_rebuild::parse(text).is_none()
    {
        return fail(EXIT_CONFIG, super::summarize_rebuild::bad_range(text));
    }
    let cfg = match loader::load_from_process(&args.as_flags()).and_then(|c| c.validate()) {
        Ok(c) => c,
        Err(e) => return fail(EXIT_CONFIG, e.to_string()),
    };
    if let Err(e) = crate::log::init(&cfg.data_dir) {
        return fail(
            EXIT_RUNTIME,
            format!(
                "error: cannot write the log file under {}: {e}. Fix: make LOUPE_DATA_DIR writable. Verify: loupe summarize --once exits 0.",
                cfg.data_dir.display()
            ),
        );
    }
    let db_fail = |e: String| {
        fail(
            EXIT_RUNTIME,
            format!(
                "error: the summarize run on the database under {} failed: {e}. Fix 1: start loupe serve one time so that the database exists and is migrated. Fix 2: point LOUPE_DATA_DIR or --data-dir at the data dir of loupe serve. Reference: docs/specification/summaries.md. Verify: loupe summarize --once exits 0.",
                cfg.data_dir.display()
            ),
        )
    };
    let mut conn = match crate::store::open::open(&cfg.data_dir) {
        Ok(c) => c,
        Err(e) => return db_fail(e.to_string()),
    };
    if !dry_run && let Err(e) = crate::store::migrate::migrate(&mut conn) {
        return db_fail(e.to_string());
    }
    let now = chrono::Utc::now().timestamp_millis();
    let s = crate::summary::settings(&cfg);
    match mode {
        Mode::Once => {
            let c = match worker::pass(&conn, &s, dry_run) {
                Ok(c) => c,
                Err(e) => return db_fail(e.to_string()),
            };
            let r = match crate::retention::worker::pass(
                &conn,
                cfg.summary.raw_retention_hours,
                now,
                dry_run,
            ) {
                Ok(r) => r,
                Err(e) => return db_fail(e.to_string()),
            };
            print(
                &[
                    ("segments_read", c.segments_read.to_string()),
                    ("done", c.done.to_string()),
                    ("skipped", c.skipped.to_string()),
                    ("failed", c.failed.to_string()),
                    ("disabled", c.disabled.to_string()),
                    ("retry_later", c.retry_later.to_string()),
                    ("bodies_deleted", r.bodies_deleted.to_string()),
                    (
                        "bodies_kept_decrypt_failed",
                        r.bodies_kept_decrypt_failed.to_string(),
                    ),
                ],
                human,
            );
        }
        Mode::Retry => {
            let t = crate::session::time::text(now);
            match store::retry(&conn, &t, dry_run).and_then(|a| {
                crate::summary::store_session_day::retry(&conn, &t, dry_run).map(|b| a + b)
            }) {
                Ok(n) => print(&[("rows", n.to_string())], human),
                Err(e) => return db_fail(e.to_string()),
            }
        }
        Mode::Rebuild(text) => {
            let range = super::summarize_rebuild::parse(&text).expect("checked above");
            match super::summarize_rebuild::run(&conn, &s, range, dry_run) {
                Ok(n) => print(&[("summaries_reset", n.to_string())], human),
                Err(e) => return db_fail(e.to_string()),
            }
        }
        Mode::Day(text) => {
            let Ok(date) = chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d") else {
                return fail(
                    EXIT_CONFIG,
                    format!(
                        "error: --day {text} is not a date in YYYY-MM-DD. Fix: pass --day 2026-10-20. Verify: the command prints status."
                    ),
                );
            };
            let ds = day::Settings {
                client: s.client.as_ref(),
                public: &s.public,
                max_attempts: s.max_attempts,
                tz: cfg.summary.tz,
            };
            let r = match day::build_day_line(&conn, &ds, date, dry_run) {
                Ok(r) => r,
                Err(e) => return db_fail(e.to_string()),
            };
            let mut pairs = vec![
                ("day", text),
                ("status", r.status.to_string()),
                ("session_lines", r.session_lines.to_string()),
            ];
            if show_line {
                pairs.push(("line", r.line.unwrap_or_default()));
            }
            print(&pairs, human);
        }
    }
    ExitCode::SUCCESS
}
