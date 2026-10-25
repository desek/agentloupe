//! @agents-index Integration tests of the public mirror: contents, published days only, reconcile lag, rebuild, journal mode, write failure, write time with readers.

#[path = "common/days.rs"]
mod days;

use agent_loupe::live::hub::Hub;
use agent_loupe::mirror::{self, worker::Task};
use agent_loupe::publish::clock::{Clock, Fixed};
use rusqlite::Connection;
use std::sync::Arc;

struct Shared(Arc<Fixed>);
impl Clock for Shared {
    fn now_ms(&self) -> i64 {
        self.0.now_ms()
    }
}

const T0: i64 = 1_790_000_000_000;

fn put(c: &Connection, day: &str, hash: &str, state: &str) {
    c.execute(
        "INSERT OR REPLACE INTO days (day, record, content_hash, revision, built_at, state) VALUES (?1, '{\"day\":\"' || ?1 || '\"}', ?2, 1, '2026-10-01T00:00:00Z', ?3)",
        rusqlite::params![day, hash, state],
    )
    .unwrap();
}

fn task(dir: &std::path::Path) -> (Task, Arc<Fixed>) {
    let clock = Arc::new(Fixed::new(T0));
    let t = Task::new(
        agent_loupe::store::open::open(dir).unwrap(),
        mirror::schema::open(dir).unwrap(),
        chrono_tz::UTC,
        Hub::off(),
        Box::new(Shared(clock.clone())),
    );
    (t, clock)
}

fn mdays(dir: &std::path::Path) -> Vec<(String, String)> {
    let c = mirror::open::open(dir).unwrap();
    let mut s = c
        .prepare("SELECT day, content_hash FROM days ORDER BY day")
        .unwrap();
    s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn holds_only_published_days_and_two_tables() {
    let (dir, c) = days::db();
    put(&c, "2026-10-01", "a", "published");
    put(&c, "2026-10-02", "b", "pending_review");
    c.execute("INSERT INTO days (day, record, content_hash, revision, built_at, state) VALUES ('2026-10-03', NULL, NULL, 1, NULL, 'deleted')", []).unwrap();
    days::simple(&c, "2026-10-01T10:00:00Z", "claude-code");
    let (mut t, _) = task(dir.path());
    let p = t.pass();
    assert_eq!(p.days.written, 1);
    assert_eq!(mdays(dir.path()), vec![("2026-10-01".into(), "a".into())]);
    let m = mirror::open::open(dir.path()).unwrap();
    assert_eq!(
        mirror::schema::tables(&m).unwrap(),
        vec!["days", "live_state"]
    );
    let cols = |t: &str| -> Vec<String> {
        let mut s = m
            .prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))
            .unwrap();
        s.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        cols("days"),
        ["day", "record", "content_hash", "built_at", "state"]
    );
    assert_eq!(
        cols("live_state"),
        [
            "date",
            "calls",
            "input",
            "output",
            "cache_read",
            "cache_write",
            "reasoning",
            "cost_usd",
            "segments_open",
            "ended",
            "in_flight",
            "updated_at"
        ]
    );
    // The check constraint rejects every state other than published.
    let w = mirror::schema::open(dir.path()).unwrap();
    assert!(
        w.execute("INSERT INTO days (day, state) VALUES ('x', 'deleted')", [])
            .is_err()
    );
}

#[test]
fn reconcile_reaches_the_mirror_at_the_next_pass() {
    let (dir, c) = days::db();
    let (mut t, clock) = task(dir.path());
    t.pass();
    assert!(mdays(dir.path()).is_empty());
    put(&c, "2026-10-01", "a", "published");
    clock.set(T0 + 4_999);
    t.pass();
    assert!(mdays(dir.path()).is_empty(), "reconciled before 5 s");
    clock.set(T0 + 5_000);
    assert_eq!(t.pass().days.written, 1);
    put(&c, "2026-10-01", "a2", "published");
    clock.set(T0 + 10_000);
    t.pass();
    assert_eq!(mdays(dir.path()), vec![("2026-10-01".into(), "a2".into())]);
    c.execute(
        "UPDATE days SET state = 'deleted', record = NULL WHERE day = '2026-10-01'",
        [],
    )
    .unwrap();
    clock.set(T0 + 15_000);
    assert_eq!(t.pass().days.deleted, 1);
    assert!(mdays(dir.path()).is_empty());
}

#[test]
fn no_write_without_a_change() {
    let (dir, c) = days::db();
    put(&c, "2026-10-01", "a", "published");
    let (mut t, clock) = task(dir.path());
    assert_eq!(t.pass().days.written, 1);
    // A second reconcile with no change writes nothing: the days changes are zero.
    clock.set(T0 + 5_000);
    assert_eq!(t.pass().days, Default::default());
}

#[test]
fn another_version_or_table_makes_the_tables_again() {
    let (dir, c) = days::db();
    put(&c, "2026-10-01", "a", "published");
    let (mut t, _) = task(dir.path());
    t.pass();
    drop(t);
    {
        let w = mirror::schema::open(dir.path()).unwrap();
        w.execute_batch("CREATE TABLE sessions (x); PRAGMA user_version = 99")
            .unwrap();
    }
    assert!(matches!(
        mirror::open::open(dir.path()),
        Err(mirror::open::Error::Tables(_))
    ));
    let (mut t, _) = task(dir.path());
    let m = mirror::open::open(dir.path()).unwrap();
    assert_eq!(mirror::schema::version(&m).unwrap(), mirror::VERSION);
    assert!(mdays(dir.path()).is_empty());
    t.pass();
    assert_eq!(mdays(dir.path()).len(), 1);
    drop(t);
    mirror::schema::open(dir.path())
        .unwrap()
        .execute_batch("PRAGMA user_version = 7")
        .unwrap();
    let e = mirror::open::open(dir.path()).unwrap_err().to_string();
    assert!(
        e.contains("version 7") && e.contains("docker compose up -d loupe"),
        "{e}"
    );
}

#[test]
fn read_only_open_checks_and_refuses_writes() {
    let dir = tempfile::tempdir().unwrap();
    let e = mirror::open::open(dir.path()).unwrap_err().to_string();
    assert!(e.contains("public.db") && e.contains("loupe-public"), "{e}");
    assert!(!mirror::path(dir.path()).exists());
    mirror::schema::open(dir.path()).unwrap();
    let r = mirror::open::open(dir.path()).unwrap();
    assert!(r.execute("DELETE FROM days", []).is_err());
}

#[test]
fn journal_mode_is_delete() {
    let dir = tempfile::tempdir().unwrap();
    let w = mirror::schema::open(dir.path()).unwrap();
    let mode: String = w
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
    assert!(!dir.path().join("public").join("public.db-wal").exists());
}

#[derive(Clone, Default)]
struct Buf(Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failed_write_warns_once_and_the_next_pass_retries() {
    let (dir, c) = days::db();
    put(&c, "2026-10-01", "a", "published");
    let (mut t, clock) = task(dir.path());
    let other = mirror::schema::open(dir.path()).unwrap();
    other.execute_batch("DROP TABLE days").unwrap();
    let buf = Buf::default();
    let sink = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || sink.clone())
        .with_ansi(false)
        .finish();
    let p = tracing::subscriber::with_default(sub, || t.pass());
    assert_eq!(p.failures, 1);
    let log = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    assert_eq!(log.matches("WARN").count(), 1, "{log}");
    assert!(log.contains("Fix 1") && log.contains("Verify"), "{log}");
    other.execute_batch("CREATE TABLE days (day TEXT PRIMARY KEY, record TEXT, content_hash TEXT, built_at TEXT, state TEXT NOT NULL CHECK (state = 'published'))").unwrap();
    clock.set(T0 + 1);
    let p = t.pass();
    assert_eq!((p.failures, p.days.written), (0, 1));
}

#[test]
fn write_time_with_ten_readers() {
    let (dir, c) = days::db();
    days::simple(&c, "2026-10-01T10:00:00Z", "claude-code");
    let mut w = mirror::schema::open(dir.path()).unwrap();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let readers: Vec<_> = (0..10)
        .map(|_| {
            let (p, stop, errors) = (dir.path().to_path_buf(), stop.clone(), errors.clone());
            std::thread::spawn(move || {
                let r = mirror::open::open(&p).unwrap();
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    if r.query_row("SELECT count(*) FROM live_state", [], |x| {
                        x.get::<_, i64>(0)
                    })
                    .is_err()
                    {
                        errors.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            })
        })
        .collect();
    let row = mirror::live::read(&c, chrono_tz::UTC, T0, &Hub::off()).unwrap();
    let mut times = Vec::with_capacity(1000);
    for i in 0..1000 {
        let t0 = std::time::Instant::now();
        if mirror::live::write(&mut w, &row, T0 + i).is_err() {
            errors.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        times.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    for r in readers {
        r.join().unwrap();
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = times[949];
    eprintln!("mirror_write_p95_ms: {p95:.3}");
    assert!(p95 <= 50.0, "p95 {p95} ms");
    assert_eq!(errors.load(std::sync::atomic::Ordering::SeqCst), 0);
}
