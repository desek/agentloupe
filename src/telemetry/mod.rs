//! @agents-index Start telemetry: the host, commit, and target fixed at build time, the state of a start, its info line, and the start of the one GET.

pub mod host;
pub mod request;
pub mod send;
pub mod state;

pub use state::State;

/// The values that build.rs fixes; each is empty in a build with no telemetry host.
const BUILT_HOST: &str = env!("LOUPE_TELEMETRY_BUILT_HOST");
const BUILT_COMMIT: &str = env!("LOUPE_TELEMETRY_BUILT_COMMIT");
const BUILT_TARGET: &str = env!("LOUPE_TELEMETRY_BUILT_TARGET");

/// The host of the start request, or None in a built-out binary. The run-time environment never
/// changes it.
pub fn built_host() -> Option<&'static str> {
    (!BUILT_HOST.is_empty()).then_some(BUILT_HOST)
}

/// The value of the line `telemetry` of `loupe config show`: the build state only.
pub fn build_state() -> String {
    built_host().map_or("built-out".into(), |h| format!("built-in host={h}"))
}

/// Writes the info line `telemetry` with the state and, when a host exists, its host name only.
pub fn log_state(state: State, host_value: Option<&str>) {
    match host_value {
        Some(h) if state != State::BuiltOut => {
            tracing::info!(state = state.as_str(), host = host::name(h), "telemetry")
        }
        _ => tracing::info!(state = state.as_str(), "telemetry"),
    }
}

/// Decides the state of this start, writes the line `telemetry`, and in the state `sent` spawns the one
/// start request on the current runtime. Call it after the line `listening`; it never waits for the
/// network, and a shutdown does not wait for the task.
pub fn start() -> State {
    let host_value = built_host();
    // A built-out binary does not read the opt-out.
    let opt_out = host_value.and_then(|_| std::env::var("DO_NOT_TRACK").ok());
    let state = state::decide(host_value, opt_out.as_deref());
    log_state(state, host_value);
    if let (State::Sent, Some(h)) = (state, host_value) {
        let url = request::url(h, BUILT_COMMIT);
        let agent = request::user_agent(env!("CARGO_PKG_VERSION"), BUILT_TARGET);
        tokio::spawn(async move { send::send(&url, &agent, send::TIMEOUT).await });
    }
    state
}
