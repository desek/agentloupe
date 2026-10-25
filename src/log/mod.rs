//! @agents-index Starts the global logger that writes JSON lines to stderr and to <data dir>/logs/, one file per role.

mod format;

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;

/// A cloneable handle on the log file, so each event appends one whole line.
#[derive(Clone)]
struct FileWriter(Arc<Mutex<File>>);

impl std::io::Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).flush()
    }
}

impl<'a> MakeWriter<'a> for FileWriter {
    type Writer = FileWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Starts the logger on `<data dir>/logs/loupe.log`, the file of every subcommand other than a public serve.
pub fn init(data_dir: &Path) -> std::io::Result<()> {
    init_file(data_dir, "loupe.log")
}

/// Opens `<data dir>/logs/<file>` and installs the global subscriber. Call once, after the configuration passed.
pub fn init_file(data_dir: &Path, file: &str) -> std::io::Result<()> {
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(file))?;
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(format::JsonLine)
                .with_writer(std::io::stderr),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(format::JsonLine)
                .with_writer(FileWriter(Arc::new(Mutex::new(file)))),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO);
    tracing::subscriber::set_global_default(subscriber).map_err(std::io::Error::other)
}
