//! @agents-index The parse subcommand: one parse pass and one idle step, the four counts as key: value lines, --dry-run.

use super::ConfigArgs;
use crate::config::loader;
use crate::parse::worker::{self, Settings};
use std::io::IsTerminal;
use std::process::ExitCode;

const EXIT_CONFIG: u8 = 2;
const EXIT_RUNTIME: u8 = 1;

fn fail(code: u8, message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

/// `once` must be set: the continuous worker runs only inside `loupe serve`.
pub fn run(args: &ConfigArgs, once: bool, dry_run: bool, human: bool) -> ExitCode {
    if !once {
        return fail(EXIT_CONFIG, "error: loupe parse needs --once. Fix: run loupe parse --once, or run loupe serve for the continuous worker. Verify: the command prints calls_added.".into());
    }
    let cfg = match loader::load_from_process(&args.as_flags()).and_then(|c| c.validate()) {
        Ok(c) => c,
        Err(e) => return fail(EXIT_CONFIG, e.to_string()),
    };
    if let Err(e) = crate::log::init(&cfg.data_dir) {
        return fail(
            EXIT_RUNTIME,
            format!(
                "error: cannot write the log file under {}: {e}. Fix: make LOUPE_DATA_DIR writable, or point it at a writable directory. Verify: loupe parse --once exits 0.",
                cfg.data_dir.display()
            ),
        );
    }
    let db_fail = |e: String| {
        fail(
            EXIT_RUNTIME,
            format!(
                "error: the parse of the database under {} failed: {e}. Fix 1: start loupe serve one time so that the database exists and is migrated. Fix 2: point LOUPE_DATA_DIR or --data-dir at the data dir of loupe serve. Reference: docs/specification/parser.md. Verify: loupe parse --once exits 0.",
                cfg.data_dir.display()
            ),
        )
    };
    let mut conn = match crate::store::open::open(&cfg.data_dir) {
        Ok(c) => c,
        Err(e) => return db_fail(e.to_string()),
    };
    // A dry run writes nothing, so it does not migrate; a database without the parser tables fails.
    if !dry_run && let Err(e) = crate::store::migrate::migrate(&mut conn) {
        return db_fail(e.to_string());
    }
    let s = Settings {
        data_key: cfg.data_key,
        account_key: cfg.account_key,
        idle_secs: cfg.idle_secs,
    };
    let c = match worker::pass(&conn, &s, chrono::Utc::now().timestamp_millis(), dry_run) {
        Ok(c) => c,
        Err(e) => return db_fail(e.to_string()),
    };
    if human || std::io::stdout().is_terminal() {
        let mode = if dry_run {
            " (dry run, nothing written)"
        } else {
            ""
        };
        println!(
            "The parse read {} exchanges, added {} calls, opened {} sessions, and closed {} sessions{mode}.",
            c.exchanges_read, c.calls_added, c.sessions_opened, c.sessions_closed
        );
    } else {
        println!("exchanges_read: {}", c.exchanges_read);
        println!("calls_added: {}", c.calls_added);
        println!("sessions_opened: {}", c.sessions_opened);
        println!("sessions_closed: {}", c.sessions_closed);
    }
    ExitCode::SUCCESS
}
