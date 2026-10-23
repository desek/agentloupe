//! @agents-index Integration test of a build with no telemetry host: the pure decision, the config show line, and a serve start that sends nothing.

#[path = "common/fake_telemetry.rs"]
mod fake_telemetry;
#[path = "common/loupe.rs"]
mod loupe;

use agent_loupe::telemetry::{self, State, state::decide};
use std::time::Duration;

fn config_show(env: &[(&str, &str)]) -> String {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(["config", "show"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear();
    for (k, v) in env {
        c.env(k, v);
    }
    String::from_utf8_lossy(&c.output().unwrap().stdout).to_string()
}

#[test]
fn telemetry_built_out_state() {
    for v in [
        None,
        Some(""),
        Some("0"),
        Some("1"),
        Some("true"),
        Some(" 1"),
    ] {
        assert_eq!(decide(None, v), State::BuiltOut, "{v:?}");
    }
    // The verdict builds with no host: the pipeline scripts unset the build variables.
    assert_eq!(telemetry::built_host(), None);
    let opt_out = "DO_NOT_TRACK";
    for env in [vec![], vec![(opt_out, "1")]] {
        let out = config_show(&env);
        let line = out
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["name"] == "telemetry")
            .expect("a telemetry line");
        assert_eq!(line["value"], "built-out");
        assert_eq!(line["source"], "build");
        assert!(!out.contains(opt_out));
    }

    // A host in the run-time environment has no effect: the start sends nothing.
    let f = fake_telemetry::start_blocking(fake_telemetry::Mode::Status(204));
    let host = f.host();
    let l = loupe::Loupe::start(1, &[], &[("LOUPE_TELEMETRY_HOST", host.as_str())]);
    let ok = reqwest::blocking::get(l.url("/healthz")).unwrap().status();
    assert_eq!(ok, 200);
    let line = loupe::poll(Duration::from_secs(10), "a telemetry line", || {
        l.stderr()
            .lines()
            .find(|x| x.contains("\"message\":\"telemetry\""))
            .map(str::to_string)
            .ok_or_else(|| "no telemetry line yet".to_string())
    });
    assert!(line.contains("\"state\":\"built-out\""), "{line}");
    assert!(line.contains("\"level\":\"INFO\""), "{line}");
    assert!(!line.contains("host"), "{line}");
    let log = l.stderr();
    let listening = log.find("\"listening\"").unwrap();
    assert!(log.find("\"message\":\"telemetry\"").unwrap() > listening);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(f.requests().len(), 0);
    assert_eq!(f.connections(), 0);
}
