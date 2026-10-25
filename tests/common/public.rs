//! @agents-index Test helpers for the role public: a seeded mirror, and loupe serve with LOUPE_ROLE=public and no secret.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const DAY: &str = "2026-01-02";

/// Makes the mirror with the writer open of the role full, and one published day.
pub fn seed_mirror(data: &Path) {
    let c = agent_loupe::mirror::schema::open(data).unwrap();
    c.execute(
        "INSERT INTO days (day, record, content_hash, built_at, state) VALUES (?1, ?2, 'h', '2026-01-03T00:00:00Z', 'published')",
        [DAY, &format!(r#"{{"day":"{DAY}","calls":1}}"#)],
    )
    .unwrap();
}

/// The command of loupe serve in the role public, with a clean environment plus `env`.
pub fn command(data: &Path, env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_loupe"));
    cmd.arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_ROLE", "public")
        .env("LOUPE_LISTEN", "127.0.0.1:0")
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_UPSTREAMS_FILE", data.join("absent-upstreams.toml"));
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

/// A running role public; the drop kills it.
pub struct Public {
    pub port: u16,
    pub child: Child,
    pub stderr: std::path::PathBuf,
}

impl Drop for Public {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Public {
    pub fn start(data: &Path, err: &Path, env: &[(&str, &str)]) -> Public {
        let stderr = err.join("stderr.log");
        let mut child = command(data, env)
            .stderr(Stdio::from(std::fs::File::create(&stderr).unwrap()))
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = std::fs::read_to_string(&stderr).unwrap_or_default();
            if let Some(s) = child.try_wait().unwrap() {
                panic!("loupe exited before it listened ({s}): {text}");
            }
            if let Some(port) = port(&text) {
                return Public {
                    port,
                    child,
                    stderr,
                };
            }
            assert!(Instant::now() < deadline, "loupe did not start: {text}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(&self.stderr).unwrap_or_default()
    }
}

fn port(log: &str) -> Option<u16> {
    let line = log.lines().find(|l| l.contains("\"listening\""))?;
    let tag = "\"listen\":\"127.0.0.1:";
    let start = line.find(tag)? + tag.len();
    line[start..].split('"').next()?.parse().ok()
}
