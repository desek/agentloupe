//! @agents-index Test helper for the backfill: lays out the synthetic log fixtures as Claude Code and Pi trees, runs loupe backfill.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The raw session ID of the Claude Code fixture; also the name of its file and its session directory.
pub const CLAUDE_SESSION: &str = "c0ffee00-MARKERSESS";
/// The directory name of the fixture project: a cwd slug, which is a path in clear text.
pub const SLUG: &str = "-Users-marker-MARKERSLUG";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/backfill")
            .join(name),
    )
    .unwrap()
}

/// Writes `text` to `path`, with its parent directories.
pub fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A Claude Code root with the main file and, when `subagent` is set, its subagent file.
pub fn claude_tree(root: &Path, subagent: bool) -> PathBuf {
    let project = root.join("claude").join(SLUG);
    write(
        &project.join(format!("{CLAUDE_SESSION}.jsonl")),
        &fixture("claude.main.jsonl"),
    );
    if subagent {
        write(
            &project
                .join(CLAUDE_SESSION)
                .join("subagents/agent-MARKERAGENT.jsonl"),
            &fixture("claude.subagent.jsonl"),
        );
    }
    root.join("claude")
}

/// A Pi root with the session file.
pub fn pi_tree(root: &Path) -> PathBuf {
    write(
        &root
            .join("pi")
            .join(SLUG)
            .join("2026-03-03_MARKERPIFILE.jsonl"),
        &fixture("pi.session.jsonl"),
    );
    root.join("pi")
}

/// One Claude Code line with usage, made up for a test.
pub fn claude_line(session: &str, msg: &str, at: &str, model: &str) -> String {
    serde_json::json!({"type":"assistant","sessionId":session,"timestamp":at,
        "message":{"id":msg,"role":"assistant","model":model,"content":[{"type":"text","text":"x"}],
        "usage":{"input_tokens":2,"output_tokens":3}}})
    .to_string()
}

/// The `loupe backfill <args>` command against `data` with the test keys and the upstreams file of `data`, if any.
pub fn command(data: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.arg("backfill")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_DATA_KEY", super::loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", super::loupe::KEY);
    let up = data.join("upstreams.toml");
    if up.exists() {
        c.env("LOUPE_UPSTREAMS_FILE", up);
    }
    c
}

/// Runs `loupe backfill <args>` with the extra `env`.
pub fn run(data: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = command(data, args);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// Runs and asserts exit 0.
pub fn ok(data: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let out = run(data, args, env);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

pub fn db(data: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(data.join("loupe.db")).unwrap()
}

pub fn scalar(data: &Path, sql: &str) -> i64 {
    db(data).query_row(sql, [], |r| r.get(0)).unwrap()
}
