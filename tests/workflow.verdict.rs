//! @agents-index Workflow test: pins the head checkout, the reuse job rules, the always-run lint, the judge call, read-only permissions, and the full verdict of the release.

use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

/// The text of each job under `jobs:` (a key with two spaces of indent), by job id, in file order.
fn jobs(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut in_jobs = false;
    for line in text.lines() {
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let is_key = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if is_key {
            out.push((line.trim().trim_end_matches(':').to_string(), String::new()));
        } else if let Some((_, body)) = out.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out
}

fn job<'a>(all: &'a [(String, String)], id: &str) -> &'a str {
    &all.iter()
        .find(|(n, _)| n == id)
        .unwrap_or_else(|| panic!("no job {id}"))
        .1
}

/// The value of a key of the job itself (four spaces of indent), for example `if` or `needs`.
fn job_key<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("    {key}:");
    body.lines()
        .find(|l| l.starts_with(&prefix) && !l.starts_with("     "))
        .map(|l| l[prefix.len()..].trim())
}

const STAGES: [&str; 5] = ["format", "lint", "test", "web", "image"];
const SKIPPABLE: [&str; 4] = ["format", "test", "web", "image"];
const DECISION_IF: &str = "${{ !cancelled() && needs.reuse.outputs.decision != 'hit' }}";

/// Every checkout of the verdict workflow uses the head commit of the event and never a merge ref.
#[test]
fn checkout_uses_head_commit() {
    let text = read(".github/workflows/verdict.yml");
    assert!(
        text.contains(
            "HEAD_REF: ${{ inputs.ref || github.event.pull_request.head.sha || github.sha }}"
        ),
        "the head commit expression changed"
    );
    assert!(!text.contains("refs/pull/"), "a merge ref is named");
    assert!(
        !text.contains("github.ref }}"),
        "the event ref is checked out"
    );
    let lines: Vec<&str> = text.lines().collect();
    let mut checkouts = 0;
    for (i, l) in lines.iter().enumerate() {
        if l.contains("uses: actions/checkout@") {
            checkouts += 1;
            let step: Vec<&str> = lines[i + 1..]
                .iter()
                .take_while(|n| !n.trim_start().starts_with("- ") && n.starts_with("        "))
                .copied()
                .collect();
            assert!(
                step.iter().any(|n| n.trim() == "ref: ${{ env.HEAD_REF }}"),
                "checkout at line {} does not use the head commit",
                i + 1
            );
        }
    }
    // reuse, the five stages, and the judge.
    assert_eq!(checkouts, 7, "checkout count");
}

/// The job reuse cannot run on a call from another workflow: a call always has the ref input.
#[test]
fn reuse_never_on_workflow_call() {
    let all = jobs(&read(".github/workflows/verdict.yml"));
    let reuse = job(&all, "reuse");
    let cond = job_key(reuse, "if").expect("the job reuse has no condition");
    assert!(cond.starts_with("${{ !inputs.ref &&"), "{cond}");
    assert_eq!(job_key(reuse, "timeout-minutes"), Some("5"));
    assert_eq!(reuse.matches("verdict.reuse.sh lookup").count(), 1);
    assert_eq!(
        reuse.matches("run:").count(),
        1,
        "one call in the job reuse"
    );
    assert!(
        reuse.contains("fetch-depth: 2"),
        "the base needs the first parent"
    );
    assert!(reuse.contains("      contents: read\n      actions: read\n"));
    assert_eq!(all[0].0, "reuse", "reuse is the first job");
}

/// The lint job runs on each event: no condition and no dependency on the decision.
#[test]
fn lint_always_runs() {
    let all = jobs(&read(".github/workflows/verdict.yml"));
    let lint = job(&all, "lint");
    assert_eq!(job_key(lint, "if"), None, "lint has a condition");
    assert_eq!(job_key(lint, "needs"), None, "lint waits for another job");
    assert!(!lint.contains("decision"));
    for s in SKIPPABLE {
        let body = job(&all, s);
        assert_eq!(job_key(body, "needs"), Some("reuse"), "{s}");
        assert_eq!(job_key(body, "if"), Some(DECISION_IF), "{s}");
    }
}

/// The job verdict makes exactly one call of the judge and needs reuse and the five stages.
#[test]
fn verdict_job_calls_judge() {
    let all = jobs(&read(".github/workflows/verdict.yml"));
    let verdict = job(&all, "verdict");
    assert_eq!(
        job_key(verdict, "needs"),
        Some("[reuse, format, lint, test, web, image]")
    );
    assert_eq!(job_key(verdict, "if"), Some("${{ always() }}"));
    assert_eq!(verdict.matches("verdict.reuse.sh judge").count(), 1);
    assert_eq!(
        verdict.matches("run:").count(),
        1,
        "one call in the job verdict"
    );
    for s in STAGES {
        assert!(
            verdict.contains(&format!("${{{{ needs.{s}.result }}}}")),
            "{s} result"
        );
    }
}

/// No job of the verdict workflow has a write permission.
#[test]
fn verdict_has_no_write_permission() {
    let text = read(".github/workflows/verdict.yml");
    for l in text.lines() {
        let t = l.trim();
        assert!(!t.ends_with(": write") && t != "write-all", "{l}");
    }
}

/// The release workflow calls the verdict workflow on the tag before its builds and image builds.
#[test]
fn release_calls_full_verdict() {
    let text = read(".github/workflows/release.yml");
    let all = jobs(&text);
    let verdict = job(&all, "verdict");
    assert!(verdict.contains("uses: ./.github/workflows/verdict.yml"));
    assert!(
        verdict.contains("ref: refs/tags/"),
        "the call passes the tag as ref"
    );
    assert_eq!(job_key(job(&all, "build"), "needs"), Some("verdict"));
    assert_eq!(job_key(job(&all, "image"), "needs"), Some("verdict"));
}
