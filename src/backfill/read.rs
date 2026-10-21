//! @agents-index Reads one log file as a stream of JSON lines, counting malformed lines with a file index and a line number only.

use serde_json::Value;
use std::io::BufRead;

/// The entries of one file and the line counts.
#[derive(Default)]
pub struct Lines {
    /// The line number (from 1) and the parsed object of each good line.
    pub entries: Vec<(usize, Value)>,
    pub read: u64,
    pub skipped: u64,
}

/// Reads the JSON object of each non-empty line. A malformed line is counted and, when `log` is set,
/// logged with the file index and the line number, never with the line or the path.
pub fn file(
    path: &std::path::Path,
    source: &str,
    file_index: usize,
    log: bool,
) -> std::io::Result<Lines> {
    let mut r = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut out = Lines::default();
    let mut buf = Vec::new();
    let mut n = 0;
    loop {
        buf.clear();
        if r.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        n += 1;
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        out.read += 1;
        match serde_json::from_slice::<Value>(&buf) {
            Ok(v) if v.is_object() => out.entries.push((n, v)),
            _ => {
                out.skipped += 1;
                if log {
                    tracing::warn!(source, file_index, line = n, "malformed log line skipped");
                }
            }
        }
    }
    Ok(out)
}
