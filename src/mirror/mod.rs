//! @agents-index The public mirror: a derived SQLite file with only published days and the live row, written by the role full.
//!
//! This module never imports the capture, proxy, token, private, account, session, or parse
//! modules; it reads loupe.db only through a connection that the caller gives.

pub mod days;
pub mod live;
pub mod open;
pub mod schema;
pub mod worker;

use std::path::{Path, PathBuf};

/// The mirror version of the binary. Change it with each change of the mirror tables: the role
/// full then makes the tables again, and a role public of another version stops at start.
pub const VERSION: i64 = 1;

/// The two tables of the mirror, in the order of the table check.
pub const TABLES: [&str; 2] = ["days", "live_state"];

/// `<data dir>/public/public.db`.
pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("public").join("public.db")
}
