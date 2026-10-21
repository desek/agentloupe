//! @agents-index Integration tests of the start of loupe serve: the provider log line, no credential in the log, a ping that hangs, legacy names.

#[path = "common/fake_provider.rs"]
mod fake_provider;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;
#[path = "common/providers.rs"]
mod providers;

use fake_provider::Mode;
use providers::{ANTHROPIC_KEY, OPENAI_KEY, clean};
use std::time::{Duration, Instant};

fn wait_paths(f: &fake_provider::Fake, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while f.paths().len() < n {
        assert!(Instant::now() < deadline, "only {:?}", f.paths());
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn start_log_lists_providers_without_credential() {
    let f = fake_provider::start(Mode::Ok);
    let (a, o) = providers::bases(&f);
    let l = loupe::Loupe::start(
        1,
        &[],
        &[
            ("ANTHROPIC_API_KEY", ANTHROPIC_KEY),
            ("OPENAI_API_KEY", OPENAI_KEY),
            ("ANTHROPIC_BASE_URL", &a),
            ("OPENAI_BASE_URL", &o),
        ],
    );
    wait_paths(&f, 2);
    let text = l.stderr();
    let line = text
        .lines()
        .find(|x| x.contains("providers detected"))
        .unwrap_or_else(|| panic!("no provider line: {text}"));
    assert!(line.contains("anthropic,openai"), "{line}");
    assert!(
        line.contains("\"summary_provider\":\"anthropic\""),
        "{line}"
    );
    assert!(clean(&text) && clean(&l.log()));
}

#[test]
fn provider_log_never_holds_a_credential() {
    let f = fake_provider::start(Mode::Unauthorized);
    let (a, o) = providers::bases(&f);
    let l = loupe::Loupe::start(
        1,
        &[],
        &[
            ("CLAUDE_CODE_OAUTH_TOKEN", providers::OAUTH_TOKEN),
            ("OPENAI_API_KEY", OPENAI_KEY),
            ("ANTHROPIC_BASE_URL", &a),
            ("OPENAI_BASE_URL", &o),
        ],
    );
    wait_paths(&f, 2);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !l.stderr().contains("provider ping did not get ok") {
        assert!(Instant::now() < deadline, "no ping warning: {}", l.stderr());
        std::thread::sleep(Duration::from_millis(50));
    }
    let all = format!("{}{}", l.stderr(), l.log());
    let hits = providers::SECRETS
        .iter()
        .filter(|s| all.contains(**s))
        .count();
    println!("provider_log_credential_hits: {hits}");
    assert_eq!(hits, 0);
    assert!(clean(&all));
}

#[test]
fn listener_starts_while_ping_hangs() {
    let f = fake_provider::start(Mode::Hang);
    let t = Instant::now();
    let l = loupe::Loupe::start(
        1,
        &[],
        &[
            ("ANTHROPIC_API_KEY", ANTHROPIC_KEY),
            ("ANTHROPIC_BASE_URL", &f.url()),
        ],
    );
    let res = reqwest::blocking::get(l.url("/healthz")).unwrap();
    let took = t.elapsed();
    println!("healthz_while_ping_hangs_secs: {:.2}", took.as_secs_f64());
    assert_eq!(res.status(), 200);
    assert!(took < Duration::from_secs(5), "{took:?}");
}

#[test]
fn legacy_name_stops_serve() {
    let data = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_SUMMARY_OAUTH_TOKEN", "legacy-Vq3Zr8")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        err.contains("CLAUDE_CODE_OAUTH_TOKEN") && err.contains(".env"),
        "{err}"
    );
    assert!(!err.contains("legacy-Vq3Zr8"));
}
