//! @agents-index Integration test: every stage prints its declared inputs and toolchain, and the tests read only declared test inputs.

use std::path::Path;
use std::process::Command;

const STAGES: [&str; 5] = ["format", "lint", "test", "web", "image"];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The stdout lines of one stage entry point run with one flag; the flag must exit 0.
fn lines(stage: &str, flag: &str) -> Vec<String> {
    let out = Command::new("bash")
        .arg(root().join(format!(".agents/scripts/pipeline.{stage}.sh")))
        .arg(flag)
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stage} {flag}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

fn covered(path: &str, inputs: &[String]) -> bool {
    inputs
        .iter()
        .any(|i| path == i || path.starts_with(&format!("{i}/")))
}

#[test]
fn every_stage_prints_its_inputs() {
    for stage in STAGES {
        let inputs = lines(stage, "--inputs");
        let own = format!(".agents/scripts/pipeline.{stage}.sh");
        assert!(covered(&own, &inputs), "{stage}: {inputs:?}");
    }
    for stage in ["format", "lint", "test"] {
        let inputs = lines(stage, "--inputs");
        for p in [
            "src",
            "tests",
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "rust-toolchain.toml",
            "clippy.toml",
        ] {
            assert!(inputs.contains(&p.to_string()), "{stage} misses {p}");
        }
    }
    let test = lines("test", "--inputs");
    for p in [
        ".tool-versions",
        "upstreams.toml",
        "config",
        ".agents/scripts",
        "docs/specification/accounts.md",
        ".github/workflows",
    ] {
        assert!(test.contains(&p.to_string()), "test misses {p}");
    }
    assert_eq!(
        lines("web", "--inputs"),
        [
            "web",
            ".tool-versions",
            ".agents/scripts/pipeline.web.sh",
            ".agents/scripts/web.size.sh"
        ]
    );
}

/// The dependency check of the lint stage reads its policy files and its script, so they are lint inputs.
#[test]
fn lint_inputs_cover_deny_files() {
    let inputs = lines("lint", "--inputs");
    for p in ["deny.toml", "about.toml", ".agents/scripts/deps.deny.sh"] {
        assert!(inputs.contains(&p.to_string()), "lint misses {p}");
    }
}

#[test]
fn image_inputs_cover_dockerfile_copy_sources() {
    let inputs = lines("image", "--inputs");
    let docker = std::fs::read_to_string(root().join("Dockerfile")).unwrap();
    for line in docker.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first().map(|w| w.eq_ignore_ascii_case("COPY")) != Some(true)
            || words.iter().any(|w| w.starts_with("--from"))
        {
            continue;
        }
        for src in &words[1..words.len() - 1] {
            // A one-character bracket makes a source optional, for example THIRD-PARTY-NOTICE[S]; the
            // declared input is the plain path.
            let src = src.trim_start_matches("./").replace(['[', ']'], "");
            assert!(covered(&src, &inputs), "image misses {src}");
        }
    }
    for p in [
        "Dockerfile",
        ".dockerignore",
        "LICENSE",
        "THIRD-PARTY-NOTICES",
    ] {
        assert!(inputs.contains(&p.to_string()), "image misses {p}");
    }
}

/// Each FROM line names its base image by the SHA-256 digest of the image index, so a build is repeatable.
#[test]
fn dockerfile_base_images_pinned() {
    let docker = std::fs::read_to_string(root().join("Dockerfile")).unwrap();
    let froms: Vec<&str> = docker
        .lines()
        .filter(|l| {
            l.split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("FROM"))
        })
        .collect();
    assert!(!froms.is_empty(), "no FROM line");
    for line in froms {
        let image = line.split_whitespace().nth(1).unwrap_or("");
        let digest = image.split_once("@sha256:").map(|(_, d)| d).unwrap_or("");
        assert!(
            digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()),
            "FROM line without a sha256 digest of 64 hex digits: {line}"
        );
    }
}

/// Every string literal in a test file, and each include_str or include_bytes argument resolved against the file.
fn literal_paths(file: &Path, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (i, part) in text.split('"').enumerate() {
        if i % 2 == 0 || part.is_empty() || part.contains(char::is_whitespace) {
            continue;
        }
        found.push(part.trim_start_matches('/').to_string());
        let rel = file.parent().unwrap().join(part);
        if let Ok(abs) = rel.canonicalize()
            && let Ok(r) = abs.strip_prefix(root().canonicalize().unwrap())
        {
            found.push(r.to_string_lossy().to_string());
        }
    }
    found
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            if !p.ends_with("fixtures") {
                rust_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Literals that name a repository path but that the test does not read from the repository, with the reason.
const NOT_READ: &[(&str, &str)] = &[
    // An expected part of an error message.
    ("private.off.rs", ".env.example"),
    ("config.role.rs", ".env.example"),
    ("serve.role.rs", "compose.yaml"),
    // An expected entry of the image inputs.
    ("pipeline.inputs.rs", ".dockerignore"),
    ("pipeline.inputs.rs", "LICENSE"),
    // Expected entries of the lint inputs.
    ("pipeline.inputs.rs", "deny.toml"),
    ("pipeline.inputs.rs", "about.toml"),
    // Files of the scratch repository of the verdict tests, not of this repository.
    ("pipeline.rs", "Dockerfile"),
    ("pipeline.rs", "docs/specification/operations.md"),
    ("pipeline.rs", ".gitignore"),
    ("pipeline.cache.rs", "docs/specification/operations.md"),
    (
        "pipeline.fingerprint.rs",
        "docs/specification/operations.md",
    ),
    // The name of the config show line and of the start log line, not the directory telemetry/.
    ("telemetry.built_out.rs", "telemetry"),
    ("telemetry.send.rs", "telemetry"),
    ("private_config.rs", "telemetry"),
];

#[test]
fn tests_read_only_declared_inputs() {
    let inputs = lines("test", "--inputs");
    let tracked = Command::new("git")
        .args(["ls-files"])
        .current_dir(root())
        .output()
        .unwrap();
    let tracked = String::from_utf8_lossy(&tracked.stdout).to_string();
    // A literal counts as a repository path only if git tracks it as a file or as a directory prefix,
    // so names of temporary files and ignored paths do not count.
    let is_repo_path = |p: &str| {
        !p.is_empty()
            && p != "."
            && tracked
                .lines()
                .any(|t| t == p || t.starts_with(&format!("{}/", p.trim_end_matches('/'))))
    };
    let mut files = Vec::new();
    rust_files(&root().join("tests"), &mut files);
    let mut missing = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for p in literal_paths(&file, &text) {
            let p = p.trim_end_matches('/').to_string();
            let name = file.file_name().unwrap().to_string_lossy();
            let exempt = NOT_READ.iter().any(|(f, l)| name == *f && p == *l);
            if is_repo_path(&p) && !covered(&p, &inputs) && !exempt {
                missing.push(format!("{}: {p}", file.display()));
            }
        }
    }
    assert!(missing.is_empty(), "undeclared test inputs: {missing:#?}");
}

#[test]
fn every_stage_prints_its_toolchain() {
    for stage in STAGES.iter().filter(|s| **s != "image") {
        let out = lines(stage, "--toolchain");
        assert!(!out.is_empty(), "{stage}");
    }
    let docker = std::fs::read_to_string(root().join("Dockerfile")).unwrap();
    let bases: Vec<&str> = docker
        .lines()
        .filter_map(|l| l.strip_prefix("FROM "))
        .map(|f| f.split_whitespace().next().unwrap())
        .collect();
    // The image toolchain reads the local base images and pulls nothing. On a host that lacks one,
    // for example a runner of the test job, it must fail, so the verdict runs the image stage in full.
    let present = |img: &str| {
        Command::new("docker")
            .args(["image", "inspect", img])
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if !bases.iter().all(|b| present(b)) {
        let out = Command::new("bash")
            .arg(root().join(".agents/scripts/pipeline.image.sh"))
            .arg("--toolchain")
            .current_dir(root())
            .output()
            .unwrap();
        assert_ne!(out.status.code(), Some(0), "an absent base image must fail");
        return;
    }
    let image = lines("image", "--toolchain");
    for img in bases {
        assert!(
            image
                .iter()
                .any(|l| l.starts_with(&format!("base {img} sha256:"))),
            "{img}: {image:?}"
        );
    }
}
