//! @agents-index Actionable configuration error: what failed, the labeled fixes, the pointer to .env.example, and what to verify.

use std::fmt;

#[derive(Debug)]
pub struct ConfigError {
    pub variable: String,
    pub problem: String,
    pub fixes: Vec<String>,
    /// The check after the fix, when the default check (the value in `loupe config show`) does not fit.
    pub verify: Option<String>,
}

impl ConfigError {
    pub fn new(variable: &str, problem: impl Into<String>, fixes: Vec<String>) -> Self {
        Self {
            variable: variable.to_string(),
            problem: problem.into(),
            fixes,
            verify: None,
        }
    }

    /// The same error with a check of its own in place of the default check.
    pub fn verify(mut self, check: impl Into<String>) -> Self {
        self.verify = Some(check.into());
        self
    }

    /// The error for a required key that is missing or not valid; the fix is always a new key.
    pub fn key(variable: &str, problem: impl Into<String>) -> Self {
        Self::new(
            variable,
            problem,
            vec![
                format!("run `loupe gen-key` and put the value in .env as {variable}=<value>"),
                format!("export {variable} with a key from `loupe gen-key` before you start loupe"),
            ],
        )
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "error: configuration value {} {}.",
            self.variable, self.problem
        )?;
        for (i, fix) in self.fixes.iter().enumerate() {
            writeln!(f, "fix {}: {fix}", i + 1)?;
        }
        writeln!(
            f,
            "reference: .env.example lists every variable; docs/specification/configuration.md describes each value."
        )?;
        match &self.verify {
            Some(check) => write!(f, "verify: {check}."),
            None => write!(
                f,
                "verify: run `loupe config show` and see `set` or the expected value for {}.",
                self.variable
            ),
        }
    }
}

impl std::error::Error for ConfigError {}
