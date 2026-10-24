//! @agents-index Build script: reads the price commit from vendor/litellm/SOURCE.md, writes the embed list of the web/ files, and fixes the telemetry host, commit, and target.

use std::path::Path;

/// The host rule, shared with the crate and its tests.
#[allow(dead_code)]
#[path = "src/telemetry/host.rs"]
mod host;

const SOURCE: &str = "vendor/litellm/SOURCE.md";

/// The 40-hex value of the `Commit` row of SOURCE.md, or None if the row is missing or not valid.
fn commit(text: &str) -> Option<String> {
    let row = text
        .lines()
        .find(|l| l.trim_start().starts_with("| Commit |"))?;
    let value = row.split('|').nth(2)?.trim().trim_matches('`');
    let ok = value.len() == 40
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    ok.then(|| value.to_string())
}

/// The extensions that the static handler serves; other files of web/ are not embedded.
const WEB_EXT: &[&str] = &["html", "js", "css", "woff2", "svg", "ico"];

/// Collects every servable file under `dir`, except the unit tests in web/test, and prints a rerun line
/// for each file and each directory, so an added, changed, or removed file rebuilds the embed.
fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("web/ must be readable")
        .map(|e| e.expect("readable web entry").path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p != Path::new("web/test") {
                walk(&p, out);
            }
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| WEB_EXT.contains(&e))
        {
            println!("cargo:rerun-if-changed={}", p.display());
            out.push(p);
        }
    }
}

/// Writes `web_files.rs`: a slice of (path relative to web/, bytes) with include_bytes, so the
/// binary holds every file and the handler never reads the file system.
fn embed_web() {
    let mut files = Vec::new();
    walk(Path::new("web"), &mut files);
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir");
    let mut code = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for f in files {
        let rel = f
            .strip_prefix("web")
            .expect("under web")
            .to_string_lossy()
            .replace('\\', "/");
        let rel = rel.trim_start_matches('/');
        code.push_str(&format!(
            "    ({rel:?}, include_bytes!({:?})),\n",
            Path::new(&root).join(&f).display().to_string()
        ));
    }
    code.push_str("];\n");
    let out =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("web_files.rs");
    std::fs::write(out, code).expect("OUT_DIR is writable");
}

/// True for exactly 40 lowercase hex characters, the form of a full commit.
fn is_commit(v: &str) -> bool {
    v.len() == 40
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The output of one git command in the manifest directory, trimmed; None when git or the repository is absent.
fn git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(std::env::var("CARGO_MANIFEST_DIR").ok()?)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The commit of HEAD from git, with a rerun line for each git file that names it: HEAD, the ref that
/// HEAD points to, and packed-refs, so a new commit builds the binary again with the new hash.
fn git_commit() -> Option<String> {
    let head = git(&["rev-parse", "--verify", "HEAD^{commit}"])?;
    let mut watch = vec![git(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "HEAD",
    ])];
    if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch.push(git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            &r,
        ]));
    }
    watch.push(git(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "packed-refs",
    ]));
    // A missing file would make cargo run the script at each build, so only the files that exist count.
    for p in watch.into_iter().flatten() {
        if Path::new(&p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    is_commit(&head).then_some(head)
}

/// Stops the build with an actionable error; the text never holds the value of a variable.
fn stop(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}

/// Fixes the telemetry values of the binary: the host, the commit, and the target triple, each empty when
/// the host is absent. Each value is always set, so a variable of the same name in the environment of the
/// compiler cannot reach the binary.
fn telemetry() {
    println!("cargo:rerun-if-env-changed=LOUPE_TELEMETRY_HOST");
    println!("cargo:rerun-if-env-changed=LOUPE_BUILD_COMMIT");
    let host = std::env::var("LOUPE_TELEMETRY_HOST").unwrap_or_default();
    let host = host.trim();
    let (commit, target) = if host.is_empty() {
        (String::new(), String::new())
    } else {
        if let Err(why) = host::check(host) {
            stop(&format!(
                "LOUPE_TELEMETRY_HOST {why}, so the build cannot fix the target of the start request. The rule: a host name with an optional port, with no scheme, no path, no query, no fragment, no user name, no password, and no empty label. Fix 1: set it to the host only, for example telemetry.agentloupe.dev or 127.0.0.1:8899. Fix 2: unset it for a build with no start request. Reference: docs/specification/configuration.md, section Build variables. Verify: cargo build succeeds and loupe config show prints the line telemetry."
            ));
        }
        let given = std::env::var("LOUPE_BUILD_COMMIT").unwrap_or_default();
        let commit = if is_commit(&given) {
            given
        } else {
            git_commit().unwrap_or_else(|| stop("LOUPE_TELEMETRY_HOST is set, but the build found no commit: LOUPE_BUILD_COMMIT does not hold exactly 40 lowercase hex characters, and git gives no commit of HEAD in the source tree. The rule: the commit of the build is 40 lowercase hex characters. Fix 1: set LOUPE_BUILD_COMMIT to the full commit, for example $(git rev-parse HEAD). Fix 2: build in a git clone. Fix 3: unset LOUPE_TELEMETRY_HOST for a build with no start request. Reference: docs/specification/configuration.md, section Build variables. Verify: cargo build succeeds."))
        };
        (commit, std::env::var("TARGET").unwrap_or_default())
    };
    println!("cargo:rustc-env=LOUPE_TELEMETRY_BUILT_HOST={host}");
    println!("cargo:rustc-env=LOUPE_TELEMETRY_BUILT_COMMIT={commit}");
    println!("cargo:rustc-env=LOUPE_TELEMETRY_BUILT_TARGET={target}");
}

fn main() {
    embed_web();
    telemetry();
    for entry in std::fs::read_dir("vendor/litellm").expect("vendor/litellm must exist") {
        let path = entry.expect("readable vendor entry").path();
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed=vendor/litellm");
    let text = std::fs::read_to_string(Path::new(SOURCE)).unwrap_or_default();
    match commit(&text) {
        Some(c) => println!("cargo:rustc-env=LOUPE_PRICE_COMMIT={c}"),
        None => {
            eprintln!(
                "error: {SOURCE} has no Commit row of 40 lowercase hex characters, so the build cannot embed the price commit. Fix: run .agents/scripts/prices.vendor.sh to vendor the price file and write SOURCE.md again; do not edit vendor/litellm/ by hand. Reference: docs/specification/cost.md, section Embedding and the price commit. Verify: cargo build succeeds and loupe config show prints price_commit."
            );
            std::process::exit(1);
        }
    }
}
