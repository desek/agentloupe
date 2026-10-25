//! @agents-index Reads compose.yaml, the Dockerfile, and the env examples so the public service never reaches the private data or secrets.
//!
//! The project has no YAML parser, so a small line-based reader splits compose.yaml into the
//! block of each service. The checks are on the text of each block, which is enough for the
//! fixed shape of this file.

use std::path::Path;

fn read(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
        .unwrap_or_else(|e| panic!("cannot read {name}: {e}"))
}

/// The lines of one service block: the lines under `  <name>:` up to the next line with an indent of 2 or less.
fn service(compose: &str, name: &str) -> Vec<String> {
    let head = format!("  {name}:");
    let mut lines = compose.lines().skip_while(|l| *l != head);
    assert!(lines.next().is_some(), "compose.yaml has no service {name}");
    lines
        .take_while(|l| {
            l.trim().is_empty() || l.starts_with("   ") || l.trim_start().starts_with('#')
        })
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

fn has(block: &[String], line: &str) -> bool {
    block.iter().any(|l| l == line)
}

/// Every quoted port mapping in the file, from any service.
fn ports(compose: &str) -> Vec<String> {
    compose
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("- \""))
        .filter_map(|l| l.strip_suffix('"'))
        .filter(|l| {
            l.matches(':').count() >= 1 && l.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn loupe_mounts_the_mirror_volume_read_write() {
    let s = service(&read("compose.yaml"), "loupe");
    assert!(has(&s, "- loupe-data:/data"));
    assert!(has(&s, "- loupe-public:/data/public"));
    assert!(has(&s, "image: agent-loupe:local"));
}

#[test]
fn loupe_public_has_the_required_shape() {
    let c = read("compose.yaml");
    let s = service(&c, "loupe-public");
    for line in [
        "image: agent-loupe:local",
        "LOUPE_ENV: container",
        "LOUPE_ROLE: public",
        "- loupe-public:/data/public:ro",
        "- loupe-public-logs:/data",
        "- path: .env.public",
        "required: false",
        "- \"127.0.0.1:8788:8787\"",
        "condition: service_healthy",
        "restart: unless-stopped",
        "interval: 10s",
        "timeout: 3s",
        "start_period: 5s",
        "start_interval: 1s",
        "retries: 3",
    ] {
        assert!(has(&s, line), "loupe-public lacks the line {line}");
    }
    assert!(
        !s.iter().any(|l| l.starts_with("build:")),
        "loupe-public must not build a second image"
    );
    assert!(
        s.iter()
            .any(|l| l.starts_with("test:") && l.contains("GET /healthz"))
    );
    for v in ["loupe-data:", "loupe-public-logs:", "loupe-public:"] {
        assert!(
            c.lines().any(|l| l == format!("  {v}")),
            "volume {v} is not declared"
        );
    }
}

#[test]
fn loupe_public_never_reaches_private_data_or_secrets() {
    let s = service(&read("compose.yaml"), "loupe-public");
    for l in &s {
        assert!(
            !l.contains("loupe-data"),
            "loupe-public mounts loupe-data: {l}"
        );
        assert!(
            !l.split(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .any(|w| w == ".env"),
            "loupe-public names .env: {l}"
        );
        assert!(
            !l.starts_with("env_file: .env"),
            "loupe-public reads .env: {l}"
        );
    }
}

#[test]
fn every_port_binds_loopback_only() {
    let p = ports(&read("compose.yaml"));
    assert!(p.len() >= 2, "expected two port mappings, found {p:?}");
    for m in p {
        assert!(
            m.starts_with("127.0.0.1:"),
            "a port binds an address other than 127.0.0.1: {m}"
        );
    }
}

#[test]
fn the_image_makes_the_mirror_directory_for_loupe() {
    let d = read("Dockerfile");
    assert!(d.contains("mkdir -p /data/public && chown loupe:loupe /data /data/public"));
}

#[test]
fn the_env_examples_and_gitignore() {
    assert!(read(".env.example").lines().any(|l| l == "LOUPE_ROLE="));
    let public = read(".env.public.example");
    let vars = read("src/config/vars.rs");
    // Secret names: each var(...) entry marked true, plus the GitHub token prefix.
    let mut secrets: Vec<String> = vec!["LOUPE_GITHUB_TOKEN_".into()];
    let flat = vars.split_whitespace().collect::<String>();
    for entry in flat.split("var(\"").skip(1) {
        let name = entry.split('"').next().unwrap();
        if entry.split(')').next().unwrap().ends_with(",true") {
            secrets.push(name.to_string());
        }
    }
    assert!(
        secrets.contains(&"LOUPE_DATA_KEY".to_string()),
        "secret list parse failed: {secrets:?}"
    );
    for s in &secrets {
        assert!(
            !public.contains(s.as_str()),
            ".env.public.example names the secret {s}"
        );
    }
    for v in [
        "LOUPE_TZ",
        "LOUPE_SITE_TITLE",
        "LOUPE_SUBSCRIPTION_",
        "LOUPE_LIVE_ENABLED",
        "LOUPE_LIVE_ALLOWED_ORIGIN",
        "LOUPE_LIVE_MAX_CONNECTIONS",
        "DO_NOT_TRACK",
    ] {
        assert!(public.contains(v), ".env.public.example lacks {v}");
    }
    let gi = read(".gitignore");
    assert!(gi.lines().any(|l| l == ".env"));
    assert!(gi.lines().any(|l| l == ".env.*" || l == ".env.public"));
    assert!(!gi.lines().any(|l| l == "!.env.public"));
}
