//! @agents-index Stops the start when a removed summary variable is set, with an error that names its new name and never its value.

use crate::config::error::ConfigError;

/// Each removed name with the name that replaces it.
pub const RENAMED: [(&str, &str); 3] = [
    ("LOUPE_SUMMARY_API_KEY", "ANTHROPIC_API_KEY"),
    ("LOUPE_SUMMARY_OAUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"),
    ("LOUPE_SUMMARY_BASE_URL", "ANTHROPIC_BASE_URL"),
];

/// The first removed name that `get` gives a value that is not empty, as an actionable error.
pub fn check(get: impl Fn(&str) -> Option<String>) -> Result<(), ConfigError> {
    for (old, new) in RENAMED {
        if get(old).is_some_and(|v| !v.trim().is_empty()) {
            return Err(ConfigError::new(
                old,
                format!("is set, but loupe no longer reads it; the new name is {new}"),
                vec![
                    format!("rename {old} to {new} in .env, and keep the value"),
                    format!("remove {old} from the environment of the process"),
                ],
            )
            .verify("run `loupe config show` and see the provider.anthropic line"));
        }
    }
    Ok(())
}

/// The check over the environment of this process.
pub fn check_process() -> Result<(), ConfigError> {
    check(|n| std::env::var(n).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_new_variable_and_hides_the_value() {
        for (old, new) in RENAMED {
            let e = check(|n| (n == old).then(|| "Zq8Lk3Vw".to_string())).unwrap_err();
            let text = e.to_string();
            assert!(text.contains(old) && text.contains(new) && text.contains(".env"));
            assert!(!text.contains("Zq8Lk3Vw"));
        }
        assert!(check(|_| Some(String::new())).is_ok());
        assert!(check(|_| None).is_ok());
    }
}
