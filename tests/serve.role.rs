//! @agents-index Integration tests of loupe serve in the role public: the public routes, the 404 list, the start guards, and the files it writes.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/public.rs"]
mod public;

use public::{DAY, Public};

const TOKEN: &str = "lpt_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn started() -> (tempfile::TempDir, tempfile::TempDir, Public) {
    let data = tempfile::tempdir().unwrap();
    let err = tempfile::tempdir().unwrap();
    public::seed_mirror(data.path());
    let p = Public::start(data.path(), err.path(), &[]);
    (data, err, p)
}

#[tokio::test(flavor = "multi_thread")]
async fn public_routes_answer_and_the_rest_is_404() {
    let _up = fake::spawn().await;
    let hits = fake::HITS.load(std::sync::atomic::Ordering::SeqCst);
    let (_d, _e, p) = started();
    let c = reqwest::Client::new();
    let day = format!("/v1/days/{DAY}");
    for path in [
        "/",
        "/favicon.ico",
        "/assets/js/app.js",
        "/v1/days",
        &day,
        "/healthz",
    ] {
        assert_eq!(
            c.get(p.url(path)).send().await.unwrap().status(),
            200,
            "{path}"
        );
    }
    let tok_proxy = format!("/{TOKEN}/claude-code/anthropic/v1/messages");
    let tok_days = format!("/{TOKEN}/v1/days");
    let list: Vec<(&str, &str)> = vec![
        ("GET", "/private/"),
        ("GET", "/private/login"),
        ("POST", "/private/login"),
        ("POST", "/private/logout"),
        ("GET", "/private/api/v1/report"),
        ("GET", "/private/api/v1/sessions"),
        ("GET", "/private/api/providers"),
        ("GET", "/private/api/config"),
        ("POST", "/claude-code/anthropic/v1/messages"),
        ("GET", "/claude-code/anthropic/v1/messages"),
        ("POST", "/pi/openai/v1/responses"),
        ("GET", "/pi/openai/v1/responses"),
        ("POST", &tok_proxy),
        ("GET", &tok_proxy),
        ("GET", &tok_days),
        ("GET", "/live/x"),
        ("GET", "/v1/other"),
        ("GET", "/healthz/x"),
        ("GET", "/unknown"),
        ("GET", "/a/b/c"),
        ("POST", "/pi/fake/json"),
    ];
    assert!(list.len() >= 20);
    for (m, path) in list {
        let res = c
            .request(m.parse().unwrap(), p.url(path))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 404, "{m} {path}");
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["error"], "not_found", "{m} {path}");
        assert!(!body.to_string().contains("lpt_"), "{m} {path}: {body}");
    }
    assert_eq!(fake::HITS.load(std::sync::atomic::Ordering::SeqCst), hits);
}

#[test]
fn start_line_log_file_and_no_spill() {
    let (d, _e, p) = started();
    let first = p.stderr().lines().next().unwrap().to_string();
    assert!(first.contains("\"role\":\"public\""), "{first}");
    let logs = d.path().join("logs");
    assert!(logs.join("loupe-public.log").is_file());
    assert!(!logs.join("loupe.log").exists());
    assert!(!d.path().join("spill").exists());
    assert!(!d.path().join("loupe.db").exists());
    let names: Vec<_> = std::fs::read_dir(d.path().join("public"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["public.db"]);
}

#[test]
fn private_db_in_data_dir_stops_the_start() {
    let data = tempfile::tempdir().unwrap();
    public::seed_mirror(data.path());
    std::fs::write(data.path().join("loupe.db"), b"").unwrap();
    let out = public::command(data.path(), &[]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("private database") && err.contains("compose.yaml"),
        "{err}"
    );
    assert!(!data.path().join("logs").exists());
}

#[test]
fn each_secret_stops_the_start_without_its_value() {
    let names: Vec<&str> = agent_loupe::config::vars::VARS
        .iter()
        .filter(|v| v.secret)
        .map(|v| v.name)
        .chain(["LOUPE_GITHUB_TOKEN_X"])
        .collect();
    for name in names {
        let data = tempfile::tempdir().unwrap();
        public::seed_mirror(data.path());
        let value = "secretvalue12345";
        let out = public::command(data.path(), &[(name, value)])
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{name}: {err}");
        assert!(
            err.contains(name) && err.contains(".env.public.example"),
            "{name}: {err}"
        );
        assert!(!err.contains(value), "{name}");
        assert!(!data.path().join("logs").exists(), "{name}");
    }
}

/// The role public starts no worker and never reads the upstreams file: a file that is not valid
/// TOML does not stop it, and no start line of a worker of the role full appears.
#[test]
fn starts_no_worker_and_reads_no_upstreams_file() {
    let data = tempfile::tempdir().unwrap();
    let err = tempfile::tempdir().unwrap();
    public::seed_mirror(data.path());
    let bad = data.path().join("bad-upstreams.toml");
    std::fs::write(&bad, "this is [not toml").unwrap();
    let p = Public::start(
        data.path(),
        err.path(),
        &[("LOUPE_UPSTREAMS_FILE", bad.to_str().unwrap())],
    );
    // Each worker of the role full logs its start line before the line listening, which
    // Public::start waits for, so no further wait is needed.
    let log = p.stderr();
    for line in [
        "\"database open and migrated\"",
        "\"bootstrap token step done\"",
        "\"price index loaded\"",
        "\"providers detected\"",
        "\"github: disabled, no token\"",
        "\"github: enabled\"",
        "\"public mirror open\"",
        "\"private page state\"",
    ] {
        assert!(!log.contains(line), "the role public logged {line}: {log}");
    }
    assert!(!data.path().join("spill").exists());
    assert!(!data.path().join("loupe.db").exists());
}
