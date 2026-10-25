//! @agents-index Integration test of the start request against a local fake endpoint: one body-less GET, the user agent, the timeout, no retry, no redirect, and the log lines.

#[path = "common/fake_telemetry.rs"]
mod fake_telemetry;

use agent_loupe::telemetry::{State, log_state, request, send};
use fake_telemetry::Mode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TARGET: &str = "aarch64-apple-darwin";

fn agent() -> String {
    request::user_agent(env!("CARGO_PKG_VERSION"), TARGET)
}

/// A writer that keeps every formatted log line of the crate, at debug level, for this thread only.
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Buf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).to_string()
    }
}

fn capture() -> (Buf, tracing::subscriber::DefaultGuard) {
    use tracing_subscriber::layer::SubscriberExt;
    let buf = Buf::default();
    let w = buf.clone();
    let sub = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(move || w.clone()),
        )
        .with(
            tracing_subscriber::filter::Targets::new()
                .with_target("agent_loupe", tracing::Level::DEBUG),
        );
    (buf, tracing::subscriber::set_default(sub))
}

#[tokio::test]
async fn telemetry_sends_one_get() {
    let f = fake_telemetry::start(Mode::Status(204)).await;
    let url = request::url(&f.host(), COMMIT);
    assert_eq!(
        send::send(&url, &agent(), send::TIMEOUT).await,
        send::Outcome::Status(204)
    );
    let seen = f.requests();
    assert_eq!(seen.len(), 1, "{seen:?}");
    let r = &seen[0];
    assert_eq!(r.method, "GET");
    assert_eq!(r.target, format!("/{COMMIT}"));
    assert!(!r.target.contains('?'));
    assert!(r.body.is_empty());
    assert!(
        r.headers
            .get("content-length")
            .is_none_or(|v| v.as_str() == "0")
    );
    assert_eq!(
        r.headers.get("user-agent").map(String::as_str),
        Some(agent().as_str())
    );
    for h in ["authorization", "cookie", "content-type"] {
        assert!(!r.headers.contains_key(h), "{h} must be absent");
    }
}

#[tokio::test]
async fn telemetry_gives_up_after_timeout() {
    let f = fake_telemetry::start(Mode::Hold).await;
    let url = request::url(&f.host(), COMMIT);
    let t0 = Instant::now();
    // The cap of the poll: a send that never ends fails the test at 6 seconds, not at the test timeout.
    let out = tokio::time::timeout(
        Duration::from_secs(6),
        send::send(&url, &agent(), send::TIMEOUT),
    )
    .await
    .expect("the send ended");
    let took = t0.elapsed();
    assert_eq!(out, send::Outcome::Error("timeout"));
    assert!(took <= Duration::from_millis(5500), "took {took:?}");
    assert!(took >= Duration::from_millis(4900), "took {took:?}");
    assert_eq!(f.connections(), 1);
    assert_eq!(f.requests().len(), 1);
}

#[tokio::test]
async fn telemetry_no_retry() {
    let (buf, _g) = capture();
    let f = fake_telemetry::start(Mode::Status(500)).await;
    let out = send::send(&request::url(&f.host(), COMMIT), &agent(), send::TIMEOUT).await;
    assert_eq!(out, send::Outcome::Status(500));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(f.requests().len(), 1);
    assert_eq!(f.connections(), 1);

    // A refused connection: a port that nobody listens on.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let before = buf.text().matches("telemetry result").count();
    let out = send::send(
        &request::url(&format!("127.0.0.1:{port}"), COMMIT),
        &agent(),
        send::TIMEOUT,
    )
    .await;
    assert_eq!(out, send::Outcome::Error("connect"));
    assert_eq!(buf.text().matches("telemetry result").count(), before + 1);

    // A redirect: the target of the redirect gets no request.
    let target = fake_telemetry::start(Mode::Status(204)).await;
    let r =
        fake_telemetry::start(Mode::Redirect(format!("http://{}/{COMMIT}", target.host()))).await;
    let out = send::send(&request::url(&r.host(), COMMIT), &agent(), send::TIMEOUT).await;
    assert_eq!(out, send::Outcome::Status(302));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(r.requests().len(), 1);
    assert_eq!(target.requests().len(), 0);
    assert_eq!(target.connections(), 0);
}

#[tokio::test]
async fn telemetry_log_line_has_host_only() {
    let (buf, _g) = capture();
    let f = fake_telemetry::start(Mode::Status(500)).await;
    let host = f.host();
    log_state(State::Sent, Some(&host));
    log_state(State::Disabled, Some(&host));
    log_state(State::BuiltOut, None);
    send::send(&request::url(&host, COMMIT), &agent(), send::TIMEOUT).await;
    let text = buf.text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert!(lines[0].contains("telemetry") && lines[0].contains("state=\"sent\""));
    assert!(lines[1].contains("state=\"disabled\""));
    assert!(lines[2].contains("state=\"built-out\"") && !lines[2].contains("host"));
    assert!(lines[3].contains("DEBUG") && lines[3].contains("status=500"));
    for l in &lines[..2] {
        assert!(
            l.contains(" INFO ") && l.contains("host=\"127.0.0.1\""),
            "{l}"
        );
    }
    assert!(lines[3].contains("host=127.0.0.1"), "{}", lines[3]);
    let port = format!(":{}", f.port);
    for bad in [
        "http",
        port.as_str(),
        COMMIT,
        &COMMIT[..12],
        "?",
        "agentloupe/",
        TARGET,
    ] {
        assert!(!text.contains(bad), "a log line holds {bad}: {text}");
    }
}

#[path = "common/public.rs"]
mod public;

/// The role public writes one line telemetry after the line listening, as the role full does;
/// the decision of that line is the only start request of the process.
#[test]
fn telemetry_one_start_in_role_public() {
    let data = tempfile::tempdir().unwrap();
    let err = tempfile::tempdir().unwrap();
    public::seed_mirror(data.path());
    let p = public::Public::start(data.path(), err.path(), &[]);
    assert_eq!(
        reqwest::blocking::get(p.url("/healthz")).unwrap().status(),
        200
    );
    let text = p.stderr();
    let lines: Vec<&str> = text.lines().collect();
    let listen = lines
        .iter()
        .position(|l| l.contains("\"message\":\"listening\""))
        .unwrap();
    let tel: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].contains("\"message\":\"telemetry\""))
        .collect();
    assert_eq!(tel.len(), 1, "{text}");
    assert!(tel[0] > listen, "{text}");
}
