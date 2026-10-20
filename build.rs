//! @agents-index Build script: reads the price commit from vendor/litellm/SOURCE.md and writes the embed list of the web/ files.

use std::path::Path;

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

fn main() {
    embed_web();
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
