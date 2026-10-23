//! @agents-index The telemetry state of one start: built-out, disabled, or sent, as a pure decision of the built host and the opt-out value.

/// The state that the line `telemetry` names at each start of `loupe serve`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The binary holds no host, so the start path has no target.
    BuiltOut,
    /// The binary holds a host and the run-time opt-out is set.
    Disabled,
    /// The binary holds a host and the request starts.
    Sent,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::BuiltOut => "built-out",
            State::Disabled => "disabled",
            State::Sent => "sent",
        }
    }
}

/// The state for a built host and a run-time opt-out value. Only the exact value `1` disables the
/// request; no host gives built-out for any opt-out value.
pub fn decide(host: Option<&str>, opt_out: Option<&str>) -> State {
    match (host, opt_out) {
        (None, _) => State::BuiltOut,
        (Some(_), Some("1")) => State::Disabled,
        (Some(_), _) => State::Sent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUES: [Option<&str>; 8] = [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some("yes"),
        Some(" 1"),
        Some("1 "),
        Some("1"),
    ];

    #[test]
    fn telemetry_opt_out_only_value_1() {
        let h = Some("example.workers.dev");
        assert_eq!(decide(h, Some("1")), State::Disabled);
        for v in VALUES.iter().filter(|v| **v != Some("1")) {
            assert_eq!(decide(h, *v), State::Sent, "{v:?}");
        }
    }
}
