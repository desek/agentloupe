//! @agents-index Integration test: prices.vendor.sh rejects a price file without the required model with exit 4 and writes nothing.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

#[test]
fn vendor_rejects_commit_without_required_model() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("vendor/litellm/SOURCE.md");
    let before = std::fs::read(&source).unwrap();
    // A stand-in for uvx that passes the schema step, so the test needs no network and no Python package.
    let bin = tempfile::tempdir().unwrap();
    let uvx = bin.path().join("uvx");
    std::fs::write(&uvx, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&uvx, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = Command::new("bash")
        .arg(root.join(".agents/scripts/prices.vendor.sh"))
        .arg("--dry-run")
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin",
                bin.path().display()
            ),
        )
        .env(
            "PRICES_VENDOR_SOURCE_DIR",
            root.join("tests/fixtures/price/vendor-without-required"),
        )
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(4), "{stderr}");
    assert!(stderr.contains("claude-haiku-5-5"), "{stderr}");
    assert!(stderr.contains("Fix:"), "{stderr}");
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
}
