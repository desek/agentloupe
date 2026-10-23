//! @agents-index Integration tests of GET /private/api/config: authorization, the entries of loupe config show, masking, and cached reads.

#[path = "common/fake_provider.rs"]
mod fake_provider;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;
#[path = "common/providers.rs"]
mod providers;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// The env of the server beside the env that `Loupe::spawn` sets itself.
fn extra(a: &str) -> Vec<(&str, &str)> {
    vec![
        ("LOUPE_ADMIN_TOKEN", private::ADMIN),
        ("ANTHROPIC_API_KEY", providers::ANTHROPIC_KEY),
        ("CLAUDE_CODE_OAUTH_TOKEN", providers::OAUTH_TOKEN),
        ("OPENAI_API_KEY", providers::OPENAI_KEY),
        ("ANTHROPIC_BASE_URL", a),
        ("LOUPE_TZ", "UTC"),
    ]
}

fn entries(body: &serde_json::Value) -> BTreeMap<String, (String, String)> {
    body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let s = |k: &str| e[k].as_str().unwrap().to_string();
            (s("name"), (s("value"), s("source")))
        })
        .collect()
}

#[test]
fn private_config_route_shape_and_auth() {
    let f = fake_provider::start(fake_provider::Mode::Ok);
    let a = f.url();
    let env = extra(&a);
    let l = loupe::Loupe::start(1, &[], &env);
    let url = l.url("/private/api/config");
    assert_eq!(private::client().get(&url).send().unwrap().status(), 401);
    let res = private::client()
        .get(&url)
        .bearer_auth(private::ADMIN)
        .send()
        .unwrap();
    assert_eq!(res.status(), 200);
    assert!(res.headers().get("access-control-allow-origin").is_none());
    let got = entries(&res.json().unwrap());
    // The same env as the server: Loupe::spawn sets these four, plus the extra env.
    let up = l.data.path().join("upstreams.toml");
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(["config", "show"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_LISTEN", "127.0.0.1:0")
        .env("LOUPE_DATA_DIR", l.data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_UPSTREAMS_FILE", &up);
    for (k, v) in &env {
        c.env(k, v);
    }
    let out = c.output().unwrap();
    let want: BTreeMap<String, (String, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .map(|v| {
            let s = |k: &str| v[k].as_str().unwrap().to_string();
            (s("name"), (s("value"), s("source")))
        })
        .collect();
    assert_eq!(got, want);
    assert_eq!(got["summary_provider"].0, "anthropic");
    assert!(got["provider.openai"].0.starts_with("credential=api_key "));
    assert_eq!(got["LOUPE_ADMIN_TOKEN"].0, "set");
    // The verdict build holds no telemetry host.
    assert_eq!(
        got["telemetry"],
        ("built-out".to_string(), "build".to_string())
    );
}

#[test]
fn private_config_never_holds_a_credential() {
    let made = loupe::cmd(tempfile::tempdir().unwrap().path(), &["token", "generate"]);
    let proxy = String::from_utf8_lossy(&made.stdout).trim().to_string();
    assert!(proxy.starts_with("lpt_"), "{proxy}");
    let f = fake_provider::start(fake_provider::Mode::Ok);
    let a = f.url();
    let mut env = extra(&a);
    env.push(("LOUPE_BOOTSTRAP_TOKEN", &proxy));
    let l = loupe::Loupe::start(1, &[], &env);
    let (code, body) = private::get_json(&l, "/private/api/config");
    assert_eq!(code, 200);
    let text = body.to_string();
    let mut secrets: Vec<&str> = providers::SECRETS.to_vec();
    secrets.push(private::ADMIN);
    secrets.push(&proxy);
    let hits = secrets
        .iter()
        .flat_map(|s| (0..=s.len() - 8).map(move |i| &s[i..i + 8]))
        .filter(|part| text.contains(part))
        .count();
    println!("private_config_credential_hits: {hits}");
    assert_eq!(hits, 0);
}

#[test]
fn private_config_read_is_cached() {
    let f = fake_provider::start(fake_provider::Mode::Ok);
    let a = f.url();
    let l = loupe::Loupe::start(1, &[], &extra(&a));
    providers::wait_status(&l, "anthropic", "ok");
    let before = f.paths().len();
    let c = private::client();
    let url = l.url("/private/api/config");
    let first = c
        .get(&url)
        .bearer_auth(private::ADMIN)
        .send()
        .unwrap()
        .text()
        .unwrap();
    let mut times = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let t = Instant::now();
        let body = c
            .get(&url)
            .bearer_auth(private::ADMIN)
            .send()
            .unwrap()
            .text()
            .unwrap();
        times.push(t.elapsed());
        assert_eq!(body, first, "the configuration changed between reads");
    }
    times.sort();
    let p95 = times[949];
    println!(
        "private_config_read_p95_ms: {:.3}",
        p95.as_secs_f64() * 1000.0
    );
    assert!(p95 < Duration::from_millis(5), "{p95:?}");
    let mut ptimes = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let t = Instant::now();
        let r = c
            .get(l.url("/private/api/providers"))
            .bearer_auth(private::ADMIN)
            .send()
            .unwrap();
        assert_eq!(r.status(), 200);
        ptimes.push(t.elapsed());
    }
    ptimes.sort();
    println!(
        "provider_status_route_read_p95_ms: {:.3}",
        ptimes[949].as_secs_f64() * 1000.0
    );
    assert!(ptimes[949] < Duration::from_millis(5));
    assert_eq!(f.paths().len(), before, "a read started a ping");
}
