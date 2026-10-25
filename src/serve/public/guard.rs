//! @agents-index The two start guards of the role public: no private database in the data directory, and no secret variable set.

use crate::config::vars::{GITHUB_TOKEN_PREFIX, VARS};
use std::path::Path;

/// Stops when `<data dir>/loupe.db` exists, so the public process never reaches the private database.
pub fn no_private_db(data_dir: &Path) -> Result<(), String> {
    let db = data_dir.join("loupe.db");
    if !db.exists() {
        return Ok(());
    }
    Err(format!(
        "error: the private database {} is reachable from the public process (LOUPE_ROLE=public). Fix: give the public process its own data directory with only the mirror volume loupe-public mounted, as the service loupe-public in compose.yaml shows. See docs/specification/configuration.md, section Role. Verify: loupe config show prints LOUPE_DATA_DIR, and the mounts of the service hold no loupe.db",
        db.display()
    ))
}

/// The names of the secret variables in `env` that hold a value that is not empty; never a value.
pub fn set_secrets(env: impl IntoIterator<Item = (String, String)>) -> Vec<String> {
    let mut names: Vec<String> = env
        .into_iter()
        .filter(|(k, v)| {
            !v.is_empty()
                && (k.starts_with(GITHUB_TOKEN_PREFIX)
                    || VARS.iter().any(|x| x.secret && x.name == k))
        })
        .map(|(k, _)| k)
        .collect();
    names.sort();
    names
}

/// Stops when a secret variable is set in the environment of the process; the error names only the variables.
pub fn no_secrets() -> Result<(), String> {
    let env = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.to_string_lossy().into_owned())));
    let names = set_secrets(env);
    if names.is_empty() {
        return Ok(());
    }
    Err(format!(
        "error: the public process (LOUPE_ROLE=public) has secret variables set: {}. Fix: remove these variables from the environment of the public process, and use .env.public with no secret, as .env.public.example shows. See docs/specification/configuration.md, section Role. Verify: loupe config show prints each secret as unset",
        names.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::set_secrets;

    #[test]
    fn names_each_set_secret_and_skips_empty_and_public() {
        let env = [
            ("LOUPE_DATA_KEY", "x"),
            ("LOUPE_GITHUB_TOKEN_X", "y"),
            ("ANTHROPIC_API_KEY", ""),
            ("LOUPE_TZ", "UTC"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()));
        assert_eq!(set_secrets(env), ["LOUPE_DATA_KEY", "LOUPE_GITHUB_TOKEN_X"]);
    }
}
