//! @agents-index The backfill subcommand: reads Claude Code or Pi session logs into calls of origin log, prints the counters.

use super::ConfigArgs;
use crate::backfill::{Source, run, summary};
use crate::config::loader;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_RUNTIME: u8 = 1;
const EXIT_INPUT: u8 = 2;
const EXIT_DIR: u8 = 3;

/// The flags of `loupe backfill`, apart from the configuration flags.
pub struct Flags {
    pub source: Option<String>,
    pub path: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub pi_label: String,
    pub dry_run: bool,
    pub human: bool,
}

fn fail(code: u8, message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

/// The default log root of a source under the home directory.
fn default_root(source: Source) -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    match source {
        Source::ClaudeCode => home.join(".claude/projects"),
        Source::Pi => home.join(".pi/agent/sessions"),
    }
}

/// The start of a date in the time zone, in Unix milliseconds; `next` gives the start of the day after.
fn day_ms(flag: &str, text: &str, tz: chrono_tz::Tz, next: bool) -> Result<i64, String> {
    let d = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| {
        format!("error: {flag} {text} is not a date in YYYY-MM-DD. Fix: pass {flag} 2026-10-20. Reference: docs/specification/backfill.md. Verify: the command prints calls_added.")
    })?;
    let d = if next { d.succ_opt().unwrap_or(d) } else { d };
    Ok(crate::day::boundary::start_ms(d, tz))
}

fn print(c: &run::Counts, human: bool) {
    if human || std::io::stdout().is_terminal() {
        println!("{:<28} value", "counter");
        for (k, v) in c.pairs() {
            println!("{k:<28} {v}");
        }
    } else {
        for (k, v) in c.pairs() {
            println!("{k}: {v}");
        }
    }
}

pub fn run(args: &ConfigArgs, f: &Flags) -> ExitCode {
    let Some(source) = f.source.as_deref().and_then(Source::parse) else {
        return fail(
            EXIT_INPUT,
            format!(
                "error: loupe backfill needs --source with a known value, got {}. Fix 1: pass --source claude-code. Fix 2: pass --source pi. Reference: docs/specification/backfill.md. Verify: the command prints files_read.",
                f.source.as_deref().unwrap_or("nothing")
            ),
        );
    };
    let cfg = match loader::load_from_process(&args.as_flags()).and_then(|c| c.validate()) {
        Ok(c) => c,
        Err(e) => return fail(EXIT_INPUT, e.to_string()),
    };
    let tz = cfg.summary.tz;
    let range = (
        f.from
            .as_deref()
            .map(|t| day_ms("--from", t, tz, false))
            .transpose(),
        f.to.as_deref()
            .map(|t| day_ms("--to", t, tz, true))
            .transpose(),
    );
    let (from_ms, to_ms) = match range {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return fail(EXIT_INPUT, e),
    };
    let root = f
        .path
        .as_ref()
        .map_or_else(|| default_root(source), PathBuf::from);
    if let Err(e) = std::fs::read_dir(&root) {
        return fail(
            EXIT_DIR,
            format!(
                "error: cannot read the log directory {}: {}. Fix 1: pass --path with the directory that holds the {} logs. Fix 2: check the permissions of the directory. Reference: docs/specification/backfill.md. Verify: ls {} lists the log directories.",
                root.display(),
                e.kind(),
                source.name(),
                root.display()
            ),
        );
    }
    if let Err(e) = crate::log::init(&cfg.data_dir) {
        return fail(
            EXIT_RUNTIME,
            format!(
                "error: cannot write the log file under {}: {e}. Fix: make LOUPE_DATA_DIR writable. Verify: loupe backfill --dry-run exits 0.",
                cfg.data_dir.display()
            ),
        );
    }
    let db_fail = |e: String| {
        fail(
            EXIT_RUNTIME,
            format!(
                "error: the backfill on the database under {} failed: {e}. The files that completed are stored; a new run adds the rest. Fix 1: point LOUPE_DATA_DIR or --data-dir at the data dir of loupe serve. Fix 2: stop a process that holds the database lock, then run again. Reference: docs/specification/backfill.md. Verify: a second run prints calls_added: 0.",
                cfg.data_dir.display()
            ),
        )
    };
    let upstreams = match crate::proxy::upstreams::load(&cfg.upstreams_file) {
        Ok(u) => u,
        Err(e) => return fail(EXIT_INPUT, e),
    };
    let mut conn = match crate::store::open::open(&cfg.data_dir) {
        Ok(c) => c,
        Err(e) => return db_fail(e.to_string()),
    };
    // A dry run does all the work in one outer transaction, the migration included, then rolls it back.
    if f.dry_run
        && let Err(e) = conn.execute_batch("BEGIN IMMEDIATE")
    {
        return db_fail(e.to_string());
    }
    if let Err(e) = crate::store::migrate::migrate(&mut conn) {
        return db_fail(e.to_string());
    }
    let s = run::Settings {
        account_key: cfg.account_key,
        idle_secs: cfg.idle_secs,
        tz,
        providers: crate::proxy::upstreams::providers(&upstreams),
    };
    let o = run::Options {
        source,
        root: root.clone(),
        from_ms,
        to_ms,
        pi_label: f.pi_label.clone(),
        nested: f.dry_run,
    };
    let result = run::run(&conn, &s, &o);
    if f.dry_run {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let mut out = match result {
        Ok(o) => o,
        Err(run::Error::Dir(e)) => {
            return fail(
                EXIT_DIR,
                format!(
                    "error: cannot read the log directory {}: {}. Fix 1: pass --path with the directory that holds the logs. Fix 2: check the permissions. Verify: ls {} lists the log directories.",
                    root.display(),
                    e.kind(),
                    root.display()
                ),
            );
        }
        Err(run::Error::Db(e)) => return db_fail(e.to_string()),
    };
    if let Some(w) = &out.warning {
        eprintln!("{w}");
    }
    if !f.dry_run {
        let ws = crate::summary::settings(&cfg);
        let mut c = out.counts.clone();
        if let Err(e) = summary::pass(&conn, &ws, source, &cfg.account_key, &out, &mut c) {
            return db_fail(e.to_string());
        }
        out.counts = c;
    }
    print(&out.counts, f.human);
    ExitCode::SUCCESS
}
