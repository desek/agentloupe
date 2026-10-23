//! @agents-index Resolves each known value by precedence flag, env, file, default, and records the source of each value.

use super::error::ConfigError;
use super::secret::Secret;
use super::vars::{GITHUB_TOKEN_PREFIX, SUBSCRIPTION_PREFIX, VARS};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Where an effective value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Flag,
    Env,
    File,
    Default,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Flag => "flag",
            Source::Env => "env",
            Source::File => "file",
            Source::Default => "default",
        })
    }
}

/// One effective value. Every value is held as a Secret so that a stray Debug never prints it;
/// `secret` decides whether show may print the value.
#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub value: Option<Secret>,
    pub source: Source,
    pub secret: bool,
}

/// The resolved but not yet validated configuration.
#[derive(Debug)]
pub struct Loaded {
    pub entries: Vec<Entry>,
}

impl Loaded {
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

/// Resolves every known variable from the three inputs; pure, so tests need no process state.
pub fn resolve(
    flags: &[(String, String)],
    env: &BTreeMap<String, String>,
    file: &BTreeMap<String, String>,
) -> Loaded {
    let flag = |n: &str| {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.clone())
    };
    let pick = |name: &str, default: Option<&str>, file_allowed: bool| {
        if let Some(v) = flag(name) {
            (Some(v), Source::Flag)
        } else if let Some(v) = env.get(name) {
            (Some(v.clone()), Source::Env)
        } else if let Some(v) = file.get(name).filter(|_| file_allowed) {
            (Some(v.clone()), Source::File)
        } else {
            (default.map(str::to_string), Source::Default)
        }
    };
    let mut entries: Vec<Entry> = VARS
        .iter()
        .map(|v| {
            let (value, source) = pick(v.name, v.default, !v.secret && v.name != "LOUPE_ENV");
            Entry {
                name: v.name.to_string(),
                value: value.map(Secret::new),
                source,
                secret: v.secret,
            }
        })
        .collect();
    let mut dynamic: Vec<(String, bool)> = env
        .keys()
        .filter(|k| k.starts_with(GITHUB_TOKEN_PREFIX))
        .map(|k| (k.clone(), true))
        .collect();
    dynamic.extend(
        env.keys()
            .chain(file.keys())
            .filter(|k| k.starts_with(SUBSCRIPTION_PREFIX))
            .map(|k| (k.clone(), false)),
    );
    dynamic.sort();
    dynamic.dedup();
    for (name, secret) in dynamic {
        let (value, source) = pick(&name, None, !secret);
        entries.push(Entry {
            name,
            value: value.map(Secret::new),
            source,
            secret,
        });
    }
    Loaded { entries }
}

/// Reads config/<env>.toml under `dir` as variable name to text; secrets in the file are refused.
pub fn read_file(dir: &Path, env_name: &str) -> Result<BTreeMap<String, String>, ConfigError> {
    let path = dir.join(format!("{env_name}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|e| {
        ConfigError::new(
            "LOUPE_ENV",
            format!("names the environment {env_name:?}, but {} cannot be read: {e}", path.display()),
            vec![
                "set LOUPE_ENV or --env to an environment that has a file in config/ (local or container)".into(),
                "run loupe from the directory that holds config/".into(),
            ],
        )
    })?;
    let table: toml::Table = text.parse().map_err(|e| {
        ConfigError::new(
            "LOUPE_ENV",
            format!("selects {}, which is not valid TOML: {e}", path.display()),
            vec![format!("correct the syntax of {}", path.display())],
        )
    })?;
    let mut out = BTreeMap::new();
    for (k, v) in table {
        let secret =
            k.starts_with(GITHUB_TOKEN_PREFIX) || VARS.iter().any(|x| x.name == k && x.secret);
        if secret {
            return Err(ConfigError::new(
                &k,
                format!("is a secret but is in {}", path.display()),
                vec![format!(
                    "remove {k} from {} and put it in .env",
                    path.display()
                )],
            ));
        }
        let text = match v {
            toml::Value::String(s) => s,
            other => other.to_string(),
        };
        out.insert(k, text);
    }
    Ok(out)
}

/// Loads from the real process: flags as given, the environment, and config/<env>.toml in the working directory.
pub fn load_from_process(flags: &[(String, String)]) -> Result<Loaded, ConfigError> {
    let env: BTreeMap<String, String> = std::env::vars()
        .filter(|(k, _)| super::vars::known(k))
        .collect();
    let probe = resolve(flags, &env, &BTreeMap::new());
    let env_name = probe
        .get("LOUPE_ENV")
        .and_then(|e| e.value.clone())
        .map(|s| s.expose().to_string());
    let file = read_file(Path::new("config"), env_name.as_deref().unwrap_or("local"))?;
    Ok(resolve(flags, &env, &file))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn value(l: &Loaded, n: &str) -> (String, Source) {
        let e = l.get(n).unwrap();
        (e.value.as_ref().unwrap().expose().to_string(), e.source)
    }

    #[test]
    fn flag_overrides_env_file_and_default() {
        let flags = vec![("LOUPE_LISTEN".to_string(), "1.1.1.1:1".to_string())];
        let l = resolve(
            &flags,
            &map(&[("LOUPE_LISTEN", "2.2.2.2:2")]),
            &map(&[("LOUPE_LISTEN", "3.3.3.3:3")]),
        );
        assert_eq!(
            value(&l, "LOUPE_LISTEN"),
            ("1.1.1.1:1".into(), Source::Flag)
        );
    }

    #[test]
    fn env_overrides_file() {
        let l = resolve(
            &[],
            &map(&[("LOUPE_TZ", "UTC")]),
            &map(&[("LOUPE_TZ", "Europe/Paris")]),
        );
        assert_eq!(value(&l, "LOUPE_TZ"), ("UTC".into(), Source::Env));
    }

    #[test]
    fn file_overrides_default() {
        let l = resolve(&[], &BTreeMap::new(), &map(&[("LOUPE_IDLE_SECS", "60")]));
        assert_eq!(value(&l, "LOUPE_IDLE_SECS"), ("60".into(), Source::File));
    }

    #[test]
    fn default_when_unset() {
        let l = resolve(&[], &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(value(&l, "LOUPE_TZ"), ("UTC".into(), Source::Default));
        assert_eq!(
            value(&l, "LOUPE_IDLE_SECS"),
            ("3600".into(), Source::Default)
        );
    }

    #[test]
    fn missing_required_key_is_actionable() {
        let key = super::super::key::KEY_LEN;
        let good =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, vec![0u8; key]);
        let l = resolve(&[], &map(&[("LOUPE_ACCOUNT_KEY", &good)]), &BTreeMap::new());
        let text = l.validate().err().unwrap().to_string();
        for needle in [
            "LOUPE_DATA_KEY",
            "loupe gen-key",
            ".env.example",
            "loupe config show",
        ] {
            assert!(text.contains(needle), "missing {needle} in {text}");
        }
    }

    #[test]
    fn collects_github_token_labels() {
        let env = map(&[
            ("LOUPE_GITHUB_TOKEN_PERSONAL", "ghp_one"),
            ("LOUPE_GITHUB_TOKEN_ACME", "ghp_two"),
        ]);
        let l = resolve(&[], &env, &BTreeMap::new());
        let tokens: Vec<&Entry> = l
            .entries
            .iter()
            .filter(|e| e.name.starts_with(GITHUB_TOKEN_PREFIX))
            .collect();
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|e| e.secret));
        let shown = super::super::show::render(&l, false);
        assert!(!shown.contains("ghp_one") && !shown.contains("ghp_two"));
    }
}
