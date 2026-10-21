//! @agents-index Test helper: a scratch git repository with the real verdict and fingerprint scripts and stand-in stage scripts.

#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The stages of the verdict and the paths each stand-in declares with --inputs.
const STAGES: &[(&str, &str)] = &[
    ("format", "src"),
    ("lint", "src"),
    ("test", "src tests"),
    ("web", "web"),
    ("image", "web Dockerfile"),
];

/// A scratch repository; each stand-in stage appends its name to a marker file outside the tree, so a test
/// sees which stages ran without the marker changing a fingerprint.
pub struct Fixture {
    pub repo: tempfile::TempDir,
    pub marks: tempfile::TempDir,
}

impl Fixture {
    /// Makes the repository with one commit. `FAIL_STAGE` makes that stand-in fail, and the file
    /// `<marks>/toolchain.<stage>` replaces the --toolchain output of that stand-in.
    pub fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        let scripts = repo.path().join(".agents/scripts");
        std::fs::create_dir_all(&scripts).unwrap();
        for name in ["pipeline.verdict.sh", "pipeline.fingerprint.sh"] {
            std::fs::copy(root.join(".agents/scripts").join(name), scripts.join(name)).unwrap();
        }
        for (stage, inputs) in STAGES {
            let body = format!(
                "#!/usr/bin/env bash\ncase \"${{1:-}}\" in\n  --inputs) printf '%s\\n' {inputs}; exit 0 ;;\n  --toolchain) cat \"$MARKS/toolchain.{stage}\" 2>/dev/null || echo 'tool 1'; exit 0 ;;\nesac\necho {stage} >> \"$MARKS/ran\"\n[[ \"${{FAIL_STAGE:-}}\" == {stage} ]] && exit 1\nexit 0\n"
            );
            let path = scripts.join(format!("pipeline.{stage}.sh"));
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let f = Fixture { repo, marks };
        f.write("src/lib.rs", "pub fn a() {}\n");
        f.write("tests/a.rs", "#[test]\nfn a() {}\n");
        f.write("web/index.html", "<p>a</p>\n");
        f.write("Dockerfile", "FROM scratch\n");
        f.write("docs/specification/operations.md", "# Operations\n");
        f.write(".gitignore", ".agents/logs/\n");
        f.git(&["init", "-q"]);
        f.git(&["add", "-A"]);
        f.git(&["commit", "-qm", "init"]);
        f
    }

    pub fn path(&self) -> &Path {
        self.repo.path()
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.repo.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn git(&self, args: &[&str]) -> Output {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.repo.path())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    /// Runs one fixture script with machine output; `env` adds variables such as FAIL_STAGE.
    pub fn run(&self, script: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut c = Command::new("bash");
        c.arg(self.script(script))
            .args(args)
            .current_dir(self.repo.path())
            .env("MARKS", self.marks.path())
            .env_remove("FAIL_STAGE");
        for (k, v) in env {
            c.env(k, v);
        }
        c.output().unwrap()
    }

    pub fn script(&self, name: &str) -> PathBuf {
        self.repo.path().join(".agents/scripts").join(name)
    }

    /// The stages that ran since the last call, in order; the call clears the marker file.
    pub fn take_ran(&self) -> Vec<String> {
        let p = self.marks.path().join("ran");
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let _ = std::fs::remove_file(&p);
        text.lines().map(str::to_string).collect()
    }

    pub fn cache(&self) -> PathBuf {
        self.repo.path().join(".agents/logs/pipeline.verdict.cache")
    }
}

/// The status of one stage in the stdout of a verdict.
pub fn status(out: &Output, stage: &str) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let prefix = format!("stage: {stage} status: ");
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or("missing")
        .to_string()
}
