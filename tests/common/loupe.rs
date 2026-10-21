//! @agents-index Test helper: starts loupe serve on a free port with a temp data dir, reads rows and stderr, runs token and parse commands.
#![allow(dead_code)]

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// 32 bytes 0..31 in padded base64; a known value, so a test can search for it.
pub const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
pub const KEY_BYTES: [u8; 32] = {
    let mut k = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        k[i] = i as u8;
        i += 1;
    }
    k
};

pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A running loupe serve; killed on drop.
pub struct Loupe {
    pub port: u16,
    pub data: tempfile::TempDir,
    /// Holds stderr.log, outside the data dir.
    pub err_dir: tempfile::TempDir,
    child: Child,
}

/// Runs `loupe <args>` against `data` with a clean environment, as a second process would.
pub fn cmd(data: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_loupe"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data)
        .output()
        .unwrap()
}

/// Runs `loupe parse <args>` against `data` with the test keys, as a second process would.
pub fn parse(data: &std::path::Path, args: &[&str]) -> std::process::Output {
    parse_command(data, args).output().unwrap()
}

/// The `loupe parse <args>` command with the test keys, for a test that must spawn and kill it.
pub fn parse_command(data: &std::path::Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.arg("parse")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_DATA_KEY", KEY)
        .env("LOUPE_ACCOUNT_KEY", KEY);
    // The upstreams file of start_in maps the fake upstream to a provider, so its calls get a price.
    let up = data.join("upstreams.toml");
    if up.exists() {
        c.env("LOUPE_UPSTREAMS_FILE", up);
    }
    c
}

/// Runs `loupe summarize <args>` against `data` with the test keys and the extra `env`.
pub fn summarize(
    data: &std::path::Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> std::process::Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.arg("summarize")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data)
        .env("LOUPE_DATA_KEY", KEY)
        .env("LOUPE_ACCOUNT_KEY", KEY);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// Runs `loupe parse --once` with `LOUPE_IDLE_SECS=1`, waits past the idle time, and parses again,
/// so every segment of the sent calls is closed.
pub fn parse_and_close(data: &std::path::Path) {
    for _ in 0..2 {
        let out = parse_command(data, &["--once"])
            .env("LOUPE_IDLE_SECS", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        std::thread::sleep(Duration::from_millis(1200));
    }
}

/// The value of `key: value` lines of stdout.
pub fn count(out: &std::process::Output, key: &str) -> i64 {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| {
            l.strip_prefix(&format!("{key}: "))
                .map(|v| v.parse().unwrap())
        })
        .unwrap_or_else(|| panic!("no {key} in {}", String::from_utf8_lossy(&out.stdout)))
}

/// Creates a token named `name` in `data` and returns the raw value.
pub fn create_token(data: &std::path::Path, name: &str) -> String {
    let out = cmd(data, &["token", "create", "--name", name]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["token"].as_str().unwrap().to_string()
}

/// Revokes the token `name` in `data`.
pub fn revoke_token(data: &std::path::Path, name: &str) {
    let out = cmd(data, &["token", "revoke", name]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

impl Drop for Loupe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Loupe {
    /// Starts loupe with `fake` pointing at the fake upstream, plus `more` upstream entries and env values.
    pub fn start(fake_port: u16, more: &[(&str, String)], env: &[(&str, &str)]) -> Loupe {
        Loupe::start_in(tempfile::tempdir().unwrap(), fake_port, more, env)
    }

    /// Like start, with a data dir that a test prepared, for example with tokens.
    pub fn start_in(
        data: tempfile::TempDir,
        fake_port: u16,
        more: &[(&str, String)],
        env: &[(&str, &str)],
    ) -> Loupe {
        let mut file = format!(
            "[anthropic]\nbase_url = \"https://api.anthropic.com\"\nlitellm_provider = \"anthropic\"\n\
             [fake]\nbase_url = \"http://127.0.0.1:{fake_port}\"\nlitellm_provider = \"anthropic\"\n"
        );
        for (k, v) in more {
            file.push_str(&format!(
                "[{k}]\nbase_url = \"{v}\"\nlitellm_provider = \"anthropic\"\n"
            ));
        }
        let up = data.path().join("upstreams.toml");
        std::fs::write(&up, file).unwrap();
        let err_dir = tempfile::tempdir().unwrap();
        // The server binds port 0 and logs the port it got, so two parallel tests never share a port.
        let (child, port) = Self::spawn(data.path(), &up, err_dir.path(), env);
        Loupe {
            port,
            data,
            err_dir,
            child,
        }
    }

    /// Spawns loupe serve on a port that the kernel picks, and reads that port from the log line.
    fn spawn(
        data: &std::path::Path,
        up: &std::path::Path,
        err: &std::path::Path,
        env: &[(&str, &str)],
    ) -> (Child, u16) {
        let log = err.join("stderr.log");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_loupe"));
        cmd.arg("serve")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env_clear()
            .env("LOUPE_LISTEN", "127.0.0.1:0")
            .env("LOUPE_DATA_DIR", data)
            .env("LOUPE_DATA_KEY", KEY)
            .env("LOUPE_ACCOUNT_KEY", KEY)
            .env("LOUPE_UPSTREAMS_FILE", up)
            .stderr(Stdio::from(std::fs::File::create(&log).unwrap()));
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            if let Some(status) = child.try_wait().unwrap() {
                panic!("loupe exited before it listened ({status}): {text}");
            }
            if let Some(port) = listening_port(&text) {
                return (child, port);
            }
            assert!(Instant::now() < deadline, "loupe did not start: {text}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The process ID of the server, for a read of its resident memory.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.data.path().join("loupe.db")).unwrap()
    }

    pub fn rows(&self) -> i64 {
        self.db()
            .query_row("SELECT count(*) FROM exchanges", [], |r| r.get(0))
            .unwrap_or(0)
    }

    /// Waits until the table has `n` rows; panics after 15 seconds.
    pub fn wait_rows(&self, n: i64) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while self.rows() < n {
            assert!(
                Instant::now() < deadline,
                "only {} of {n} rows",
                self.rows()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(self.err_dir.path().join("stderr.log")).unwrap_or_default()
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.data.path().join("logs/loupe.log")).unwrap_or_default()
    }

    /// Every byte of the database files, the WAL included.
    pub fn db_bytes(&self) -> Vec<u8> {
        ["loupe.db", "loupe.db-wal", "loupe.db-shm"]
            .iter()
            .flat_map(|f| std::fs::read(self.data.path().join(f)).unwrap_or_default())
            .collect()
    }

    pub fn spill_files(&self) -> usize {
        std::fs::read_dir(self.data.path().join("spill"))
            .map(|d| d.count())
            .unwrap_or(0)
    }
}

pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The port of the `listening` log line, whose field is `"listen":"127.0.0.1:PORT"`.
fn listening_port(log: &str) -> Option<u16> {
    let line = log.lines().find(|l| l.contains("\"listening\""))?;
    let tag = "\"listen\":\"127.0.0.1:";
    let start = line.find(tag)? + tag.len();
    line[start..].split('"').next()?.parse().ok()
}
