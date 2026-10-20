//! @agents-index The capture hand-off: a bounded queue that never waits, a spill to disk when full, and the writer thread.

use super::record::ExchangeRecord;
use super::{insert::insert, spill};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};

/// How often the idle writer looks for spill files; it also looks after a record when this time passed.
const IMPORT_EVERY: Duration = Duration::from_secs(1);
/// The writer waits this long for a database lock that another process holds.
const BUSY_TIMEOUT: Duration = Duration::from_secs(60);

/// The handle that the forwarder holds. `submit` never waits and never drops a record.
#[derive(Clone)]
pub struct Capture {
    tx: SyncSender<ExchangeRecord>,
    spill_dir: PathBuf,
    key: [u8; 32],
    /// The count of records that went to a spill file because the queue was full.
    pub spilled: Arc<AtomicU64>,
}

impl Capture {
    /// Makes the handle and the receiving end for the writer.
    pub fn new(
        queue: usize,
        spill_dir: PathBuf,
        key: [u8; 32],
    ) -> (Capture, Receiver<ExchangeRecord>) {
        let (tx, rx) = sync_channel(queue);
        let spilled = Arc::new(AtomicU64::new(0));
        (
            Capture {
                tx,
                spill_dir,
                key,
                spilled,
            },
            rx,
        )
    }

    /// Queues the record, or spills it on a thread off the request path if the queue is full.
    pub fn submit(&self, r: ExchangeRecord) {
        let r = match self.tx.try_send(r) {
            Ok(()) => return,
            Err(TrySendError::Full(r) | TrySendError::Disconnected(r)) => r,
        };
        let total = self.spilled.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::warn!(spilled_total = total, id = %r.id, "capture queue full, the exchange record goes to a spill file; raise LOUPE_CAPTURE_QUEUE if this repeats");
        let (dir, key) = (self.spill_dir.clone(), self.key);
        std::thread::spawn(move || spill_or_log(&dir, &key, &r));
    }
}

/// Writes one spill file; a failure is logged with its fix and never reaches a client.
pub fn spill_or_log(dir: &std::path::Path, key: &[u8; 32], r: &ExchangeRecord) {
    if let Err(e) = spill::write(dir, key, r) {
        tracing::error!(id = %r.id, error = %e, "spill write failed, the exchange record is lost. Fix: make <LOUPE_DATA_DIR>/spill writable and free disk space. Reference: docs/specification/proxy.md, section Queue and spill. Verify: the next exchange makes a row in exchanges");
    }
}

fn import_or_log(conn: &Connection, key: &[u8; 32], dir: &std::path::Path) {
    if !spill::has_pending(dir) {
        return;
    }
    match spill::import(conn, key, dir) {
        Ok(n) => tracing::info!(imported = n, "spill files imported"),
        Err(e) => {
            tracing::error!(error = %e, "spill import failed, the files stay for the next try. Fix: check that the database is writable and LOUPE_DATA_KEY is the key that sealed the files. Reference: docs/specification/proxy.md, section Queue and spill. Verify: <LOUPE_DATA_DIR>/spill becomes empty")
        }
    }
}

/// Starts the writer thread on its own connection: imports the spill files first, then inserts each record.
/// A failed insert spills the record, so a database fault keeps the data and never stops serve.
pub fn start(
    conn: Connection,
    rx: Receiver<ExchangeRecord>,
    spill_dir: PathBuf,
    key: [u8; 32],
) -> std::thread::JoinHandle<()> {
    let _ = conn.busy_timeout(BUSY_TIMEOUT);
    std::thread::spawn(move || {
        import_or_log(&conn, &key, &spill_dir);
        let mut last_import = Instant::now();
        loop {
            match rx.recv_timeout(IMPORT_EVERY) {
                Ok(r) => {
                    if let Err(e) = insert(&conn, &key, &r) {
                        tracing::error!(id = %r.id, error = %e, "capture insert failed, the exchange record goes to a spill file. Fix: check that the database file is writable and the disk has space. Reference: docs/specification/proxy.md, section Queue and spill. Verify: <LOUPE_DATA_DIR>/spill becomes empty after the fix");
                        spill_or_log(&spill_dir, &key, &r);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if last_import.elapsed() >= IMPORT_EVERY {
                import_or_log(&conn, &key, &spill_dir);
                last_import = Instant::now();
            }
        }
        drop(conn.close());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn full_queue_spills_and_counts() {
        let buf = Buf::default();
        let sink = buf.clone();
        let sub = tracing_subscriber::fmt()
            .with_writer(move || sink.clone())
            .with_ansi(false)
            .finish();
        let data = tempfile::tempdir().unwrap();
        let dir = data.path().join("spill");
        // The writer is paused: nothing reads the receiving end.
        let (cap, _rx) = Capture::new(2, dir.clone(), [1u8; 32]);
        tracing::subscriber::with_default(sub, || {
            for i in 0..5 {
                cap.submit(crate::capture::spill::tests::sample(i));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let count = || {
            std::fs::read_dir(&dir)
                .map(|d| {
                    d.flatten()
                        .filter(|e| e.path().extension().is_some_and(|x| x == "spill"))
                        .count()
                })
                .unwrap_or(0)
        };
        while count() < 3 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(count(), 3);
        assert_eq!(cap.spilled.load(Ordering::Relaxed), 3);
        let log = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
        assert!(
            log.lines()
                .any(|l| l.contains("WARN") && l.contains("spilled_total=3")),
            "{log}"
        );
        for f in std::fs::read_dir(&dir).unwrap().flatten() {
            assert!(
                !String::from_utf8_lossy(&std::fs::read(f.path()).unwrap()).contains("anthropic")
            );
        }
    }
}
