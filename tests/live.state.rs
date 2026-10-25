//! @agents-index Integration tests of the live_state row of the public mirror: the write rules with a set clock and in_flight 0 at a clean shutdown.

#[path = "common/days.rs"]
mod days;

use agent_loupe::live::hub::Hub;
use agent_loupe::mirror::{self, worker::Task};
use agent_loupe::publish::clock::{Clock, Fixed};
use std::sync::Arc;

struct Shared(Arc<Fixed>);
impl Clock for Shared {
    fn now_ms(&self) -> i64 {
        self.0.now_ms()
    }
}

fn row(dir: &std::path::Path) -> (i64, i64, i64, i64) {
    mirror::open::open(dir)
        .unwrap()
        .query_row(
            "SELECT calls, in_flight, ended, updated_at FROM live_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
}

fn setup(
    hub: Hub,
) -> (
    tempfile::TempDir,
    rusqlite::Connection,
    Task,
    Arc<Fixed>,
    i64,
) {
    let (dir, c) = days::db();
    let t0 = days::ms("2026-10-01T12:00:00Z");
    let clock = Arc::new(Fixed::new(t0));
    let t = Task::new(
        agent_loupe::store::open::open(dir.path()).unwrap(),
        mirror::schema::open(dir.path()).unwrap(),
        chrono_tz::UTC,
        hub,
        Box::new(Shared(clock.clone())),
    );
    (dir, c, t, clock, t0)
}

#[test]
fn writes_on_change_at_most_each_second_and_at_least_each_five() {
    let hub = Hub::new(16);
    let (dir, c, mut t, clock, t0) = setup(hub.clone());
    assert!(t.pass().live_written);
    assert_eq!(row(dir.path()), (0, 0, 0, t0));
    days::simple(&c, "2026-10-01T11:00:00Z", "claude-code");
    clock.set(t0 + 999);
    assert!(!t.pass().live_written, "written inside one second");
    clock.set(t0 + 1_000);
    assert!(t.pass().live_written);
    assert_eq!(row(dir.path()).0, 1);
    clock.set(t0 + 2_000);
    assert!(!t.pass().live_written, "written with no change");
    clock.set(t0 + 6_000);
    assert!(t.pass().live_written, "no write after five seconds");
    hub.exchange_start();
    hub.exchange_start();
    hub.exchange_end(200);
    clock.set(t0 + 7_000);
    assert!(t.pass().live_written);
    assert_eq!(row(dir.path()), (1, 1, 1, t0 + 7_000));
}

#[test]
fn hub_off_gives_zero_and_clean_shutdown_writes_in_flight_zero() {
    let (dir, _c, mut t, _, _) = setup(Hub::off());
    t.pass();
    assert_eq!((row(dir.path()).1, row(dir.path()).2), (0, 0));
    let hub = Hub::new(16);
    let (dir, _c, t, _, _) = setup(hub.clone());
    hub.exchange_start();
    let h = mirror::worker::start(t);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while mirror::open::open(dir.path())
        .unwrap()
        .query_row("SELECT count(*) FROM live_state", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
        == 0
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(row(dir.path()).1, 1);
    h.stop();
    assert_eq!(row(dir.path()).1, 0);
}
