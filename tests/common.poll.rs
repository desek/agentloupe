//! @agents-index Integration test of the poll helper: it returns on the end state and fails at its cap with the expected and the last state.

#[path = "common/loupe.rs"]
mod loupe;

use std::time::Duration;

#[test]
fn poll_returns_on_end_state() {
    let mut n = 0;
    let v = loupe::poll(Duration::from_secs(2), "3", || {
        n += 1;
        (n == 3).then_some(n).ok_or(format!("{n}"))
    });
    assert_eq!(v, 3);
}

#[test]
fn poll_fails_at_cap_with_last_state() {
    let r = std::panic::catch_unwind(|| {
        loupe::poll::<()>(
            Duration::from_millis(250),
            "ready",
            || Err("waiting".into()),
        )
    });
    let msg = *r.unwrap_err().downcast::<String>().unwrap();
    assert!(
        msg.contains("expected ready") && msg.contains("last observed waiting"),
        "{msg}"
    );
}
