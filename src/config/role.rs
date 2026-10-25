//! @agents-index The role of loupe serve from LOUPE_ROLE, full or public, and its actionable error for a bad value.

use super::error::ConfigError;
use super::loader::Loaded;

/// What `loupe serve` runs: everything, or only the public routes from the public mirror.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Full,
    Public,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Full => "full",
            Role::Public => "public",
        }
    }

    /// The name of the log file under `<data dir>/logs/`, so two roles on one host never share a file.
    pub fn log_file(self) -> &'static str {
        match self {
            Role::Full => "loupe.log",
            Role::Public => "loupe-public.log",
        }
    }
}

impl Loaded {
    /// The resolved role. Only `serve` and `config show` call this; the other subcommands ignore the role.
    pub fn role(&self) -> Result<Role, ConfigError> {
        let v = self
            .get("LOUPE_ROLE")
            .and_then(|e| e.value.as_ref())
            .map(|v| v.expose().to_string())
            .unwrap_or_else(|| "full".into());
        match v.as_str() {
            "full" => Ok(Role::Full),
            "public" => Ok(Role::Public),
            _ => Err(ConfigError::new(
                "LOUPE_ROLE",
                "is not full or public",
                vec![
                    "set LOUPE_ROLE=full for the instance with the proxy and the private page".into(),
                    "set LOUPE_ROLE=public for the instance that serves only the public routes from the public mirror".into(),
                    "unset LOUPE_ROLE to use the default full".into(),
                ],
            )
            .verify("run `loupe config show` and see the line role with the value full or public")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Role;

    #[test]
    fn each_role_has_its_own_log_file() {
        assert_eq!(Role::Full.log_file(), "loupe.log");
        assert_eq!(Role::Public.log_file(), "loupe-public.log");
    }
}
