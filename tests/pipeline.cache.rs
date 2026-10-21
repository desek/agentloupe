//! @agents-index Integration test: the verdict cache skips only stages whose fingerprint passed before, and fails closed on a bad cache.

#[path = "common/pipeline.rs"]
mod pipeline;

use pipeline::{Fixture, status};

const ALL: [&str; 5] = ["format", "lint", "test", "web", "image"];

fn verdict(f: &Fixture, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    f.run("pipeline.verdict.sh", args, env)
}

fn report(out: &std::process::Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn unchanged_tree_prints_cached() {
    let f = Fixture::new();
    let first = verdict(&f, &[], &[]);
    assert_eq!(first.status.code(), Some(0), "{}", report(&first));
    assert_eq!(f.take_ran(), ALL);
    let second = verdict(&f, &[], &[]);
    assert_eq!(second.status.code(), Some(0), "{}", report(&second));
    for s in ALL {
        assert_eq!(status(&second, s), "cached", "{}", report(&second));
    }
    assert!(f.take_ran().is_empty());
}

#[test]
fn changed_input_runs_its_stage_only() {
    let f = Fixture::new();
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    f.take_ran();
    f.write("web/index.html", "<p>b</p>\n");
    let out = verdict(&f, &[], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", report(&out));
    // The image stand-in declares web as an input, as the Dockerfile copies web.
    assert_eq!(f.take_ran(), ["web", "image"]);
    for s in ["format", "lint", "test"] {
        assert_eq!(status(&out, s), "cached", "{}", report(&out));
    }
}

#[test]
fn failed_stage_is_never_cached() {
    let f = Fixture::new();
    for _ in 0..2 {
        let out = verdict(&f, &[], &[("FAIL_STAGE", "lint")]);
        assert_eq!(out.status.code(), Some(1), "{}", report(&out));
        assert_eq!(status(&out, "lint"), "failed", "{}", report(&out));
        assert!(f.take_ran().contains(&"lint".to_string()));
    }
}

#[test]
fn corrupt_cache_runs_everything() {
    let f = Fixture::new();
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    f.take_ran();
    std::fs::write(f.cache(), "garbage\x00\nstage lint zz\n").unwrap();
    let out = verdict(&f, &[], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", report(&out));
    assert_eq!(f.take_ran(), ALL);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("diagnostic: the cache is garbled").count(),
        1,
        "{stderr}"
    );
}

#[test]
fn no_cache_flag_runs_everything() {
    let f = Fixture::new();
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    f.take_ran();
    let out = verdict(&f, &["--no-cache"], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", report(&out));
    assert_eq!(f.take_ran(), ALL);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("status: cached"));
}

#[test]
fn toolchain_change_runs_stage() {
    let f = Fixture::new();
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    f.take_ran();
    std::fs::write(f.marks.path().join("toolchain.test"), "tool 2\n").unwrap();
    let out = verdict(&f, &[], &[]);
    assert_eq!(out.status.code(), Some(0), "{}", report(&out));
    assert_eq!(f.take_ran(), ["test"]);
}

#[test]
fn cache_is_bounded() {
    let f = Fixture::new();
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    // Seed 200 older tree entries; the next passed tree drops the oldest.
    let mut text = std::fs::read_to_string(f.cache()).unwrap();
    let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with("tree ")).collect();
    let mut seeded = kept.join("\n") + "\n";
    for i in 0..200 {
        seeded.push_str(&format!("tree {i:040x}\n"));
    }
    std::fs::write(f.cache(), &seeded).unwrap();
    f.write(
        "docs/specification/operations.md",
        "# Operations, changed\n",
    );
    assert_eq!(verdict(&f, &[], &[]).status.code(), Some(0));
    text = std::fs::read_to_string(f.cache()).unwrap();
    let trees: Vec<&str> = text.lines().filter(|l| l.starts_with("tree ")).collect();
    assert_eq!(trees.len(), 200, "{text}");
    assert!(!trees.contains(&format!("tree {:040x}", 0).as_str()));
}
