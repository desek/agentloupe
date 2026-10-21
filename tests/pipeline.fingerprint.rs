//! @agents-index Integration test: the tree and stage fingerprints depend only on content and leave the index and the stash unchanged.

#[path = "common/pipeline.rs"]
mod pipeline;

use pipeline::Fixture;

fn print(f: &Fixture, args: &[&str]) -> String {
    let out = f.run("pipeline.fingerprint.sh", args, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn tree_fingerprint_counts_dirty_and_untracked_files() {
    let f = Fixture::new();
    let clean = print(&f, &["tree"]);
    let index = std::fs::read(f.path().join(".git/index")).unwrap();
    f.write("src/lib.rs", "pub fn b() {}\n");
    let dirty = print(&f, &["tree"]);
    assert_ne!(clean, dirty);
    f.write("src/new.rs", "pub fn c() {}\n");
    let untracked = print(&f, &["tree"]);
    assert_ne!(dirty, untracked);
    assert_eq!(std::fs::read(f.path().join(".git/index")).unwrap(), index);
    assert!(f.git(&["stash", "list"]).stdout.is_empty());
    let st = String::from_utf8_lossy(&f.git(&["status", "--porcelain"]).stdout).to_string();
    assert!(
        st.contains(" M src/lib.rs") && st.contains("?? src/new.rs"),
        "{st}"
    );
}

#[test]
fn tree_fingerprint_is_independent_of_commit() {
    let f = Fixture::new();
    let first = print(&f, &["tree"]);
    f.git(&["commit", "-q", "--allow-empty", "-m", "same content"]);
    assert_eq!(print(&f, &["tree"]), first);
}

#[test]
fn stage_fingerprint_ignores_other_paths() {
    let f = Fixture::new();
    let before = print(&f, &["stage", "test"]);
    f.write(
        "docs/specification/operations.md",
        "# Operations, changed\n",
    );
    assert_eq!(print(&f, &["stage", "test"]), before);
    f.write("src/lib.rs", "pub fn changed() {}\n");
    assert_ne!(print(&f, &["stage", "test"]), before);
}
