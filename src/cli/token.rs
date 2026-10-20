//! @agents-index The token subcommands: generate, create, list, revoke, with --dry-run, machine output by default, --human tables.

use super::ConfigArgs;
use crate::config::loader;
use crate::token::{format, store};
use rusqlite::Connection;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

/// The data dir from the same precedence that serve uses; the keys are not needed here.
fn data_dir(args: &ConfigArgs) -> Result<PathBuf, store::StoreError> {
    let cfg = loader::load_from_process(&args.as_flags()).map_err(|e| store::StoreError {
        code: 2,
        message: e.to_string(),
    })?;
    Ok(cfg
        .get("LOUPE_DATA_DIR")
        .and_then(|e| e.value.as_ref())
        .map(|v| PathBuf::from(v.expose()))
        .unwrap_or_else(|| PathBuf::from("data")))
}

/// Opens and migrates the store, making the data dir if needed.
fn open(args: &ConfigArgs) -> Result<Connection, store::StoreError> {
    let dir = data_dir(args)?;
    let fail = |e: String| store::StoreError {
        code: 1,
        message: format!(
            "error: the database under {} cannot be opened: {e}. Fix: point LOUPE_DATA_DIR or --data-dir at a writable directory. Reference: docs/specification/operations.md. Verify: loupe token list exits 0.",
            dir.display()
        ),
    };
    std::fs::create_dir_all(&dir).map_err(|e| fail(e.to_string()))?;
    let mut conn = crate::store::open::open(&dir).map_err(|e| fail(e.to_string()))?;
    crate::store::migrate::migrate(&mut conn).map_err(|e| fail(e.to_string()))?;
    Ok(conn)
}

/// Opens the store only if it exists, read-only, so a dry run writes nothing.
fn open_read_only(args: &ConfigArgs) -> Result<Option<Connection>, store::StoreError> {
    let db = data_dir(args)?.join("loupe.db");
    if !db.exists() {
        return Ok(None);
    }
    Ok(Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok())
}

fn finish(r: Result<(), store::StoreError>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e.message);
            ExitCode::from(e.code)
        }
    }
}

fn table(human: bool) -> bool {
    human || std::io::stdout().is_terminal()
}

/// Prints one raw token; opens no database.
pub fn generate() -> ExitCode {
    println!("{}", format::generate());
    ExitCode::SUCCESS
}

pub fn create(
    args: &ConfigArgs,
    name: &str,
    tags: &[String],
    dry_run: bool,
    human: bool,
) -> ExitCode {
    finish((|| {
        store::check_name(name)?;
        let tags = store::parse_tags(tags)?;
        if dry_run {
            if let Some(conn) = open_read_only(args)?
                && store::exists(&conn, name).unwrap_or(false)
            {
                return Err(store::StoreError {
                    code: 1,
                    message: format!(
                        "error: a token with the name {name} exists, active or revoked, and a name is never reused. Fix: select another name. Verify: loupe token list."
                    ),
                });
            }
            let out = serde_json::json!({"dry_run": true, "action": "create", "name": name, "tags": tags});
            if table(human) {
                println!(
                    "Dry run: would create the token {name} with {} tags. Nothing was written.",
                    tags.len()
                );
            } else {
                println!("{out}");
            }
            return Ok(());
        }
        let conn = open(args)?;
        let raw = format::generate();
        let created_at = store::create(&conn, name, &tags, &format::hash(&raw))?;
        if table(human) {
            println!(
                "Token {name} was created at {created_at}. It shows only this one time:\n{raw}"
            );
        } else {
            println!(
                "{}",
                serde_json::json!({"name": name, "token": raw, "tags": tags, "created_at": created_at})
            );
        }
        Ok(())
    })())
}

pub fn list(args: &ConfigArgs, human: bool) -> ExitCode {
    finish((|| {
        let rows = store::list(&open(args)?)?;
        if !table(human) {
            for r in &rows {
                println!("{}", serde_json::to_string(r).expect("plain fields"));
            }
            return Ok(());
        }
        let w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
        println!(
            "{:w$}  {:7}  {:24}  {:24}  {:24}  TAGS",
            "NAME", "STATUS", "CREATED_AT", "LAST_USED_AT", "REVOKED_AT"
        );
        for r in &rows {
            let dash = || "-".to_string();
            println!(
                "{:w$}  {:7}  {:24}  {:24}  {:24}  {}",
                r.name,
                r.status,
                r.created_at,
                r.last_used_at.clone().unwrap_or_else(dash),
                r.revoked_at.clone().unwrap_or_else(dash),
                r.tags
            );
        }
        Ok(())
    })())
}

pub fn revoke(args: &ConfigArgs, name: &str, dry_run: bool, human: bool) -> ExitCode {
    finish((|| {
        let (at, changed) = if dry_run {
            let state = match open_read_only(args)? {
                Some(conn) => store::revoked_at(&conn, name).unwrap_or(None),
                None => None,
            };
            match state {
                None => return Err(store::unknown(name)),
                Some(at) => (at.unwrap_or_default(), false),
            }
        } else {
            store::revoke(&open(args)?, name)?
        };
        let out = serde_json::json!({"dry_run": dry_run, "action": "revoke", "name": name, "revoked_at": if at.is_empty() { None } else { Some(&at) }, "changed": changed});
        if table(human) {
            match (dry_run, at.is_empty()) {
                (true, true) => {
                    println!("Dry run: would revoke the token {name}. Nothing was written.")
                }
                (true, false) => println!(
                    "Dry run: the token {name} is already revoked at {at}. Nothing would change."
                ),
                _ => println!("The token {name} is revoked at {at}."),
            }
        } else {
            println!("{out}");
        }
        Ok(())
    })())
}
