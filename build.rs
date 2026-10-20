//! @agents-index Build script: reads the price commit from vendor/litellm/SOURCE.md and reruns when a vendored file changes.

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

fn main() {
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
