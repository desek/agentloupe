//! @agents-index Reads the source of the role public modules and fails on an import of a module that holds non-public data.

const FILES: &[&str] = &[
    "src/serve/public/mod.rs",
    "src/serve/public/guard.rs",
    "src/serve/public/router.rs",
    "src/serve/not_found.rs",
];

const FORBIDDEN: &[&str] = &[
    "capture", "proxy", "token", "private", "account", "session", "parse", "store",
];

#[test]
fn public_role_modules_import_no_private_module() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for f in FILES {
        let text = std::fs::read_to_string(root.join(f)).unwrap();
        for m in FORBIDDEN {
            for pat in [format!("crate::{m}"), format!("agent_loupe::{m}")] {
                assert!(!text.contains(&pat), "{f} imports {pat}");
            }
        }
    }
}
