//! @agents-index The read-only open of the mirror for the role public: no create, query_only, and the table and version checks.

use super::{TABLES, VERSION, path, schema};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Why the role public cannot read the mirror. Each message names the fix and the check.
#[derive(Debug)]
pub enum Error {
    Absent(PathBuf),
    Tables(Vec<String>),
    Version { found: i64, want: i64 },
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Absent(p) => write!(
                f,
                "the public mirror {} does not exist; the role public never makes it. Fix: start the role full first, with the volume loupe-public mounted (docker compose up -d loupe). See docs/specification/days.md, section Public mirror. Verify: the file exists, then start the role public again",
                p.display()
            ),
            Error::Tables(extra) => write!(
                f,
                "the file is not a public mirror: it holds the tables {} besides days and live_state. Fix 1: mount the volume loupe-public, not the private data volume. Fix 2: start the role full, which makes the mirror again. See docs/specification/days.md, section Public mirror. Verify: the start of the role public passes and GET /healthz returns 200",
                extra.join(",")
            ),
            Error::Version { found, want } => write!(
                f,
                "the public mirror has version {found} and this binary reads version {want}. Fix: run the same image for the two services, and start the role full first, for example docker compose up -d loupe. See docs/specification/days.md, section Public mirror. Verify: GET /healthz of the role full returns 200, then start the role public again"
            ),
            Error::Sqlite(e) => write!(
                f,
                "the public mirror could not be read: {e}. Fix: check the mount of the volume loupe-public and its permissions. Verify: the start of the role public passes"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

/// Opens the mirror read-only with no create flag. It does not set the journal mode and does not
/// open the file as immutable, so each commit of the role full is visible.
pub fn open(data_dir: &Path) -> Result<Connection, Error> {
    let p = path(data_dir);
    if !p.is_file() {
        return Err(Error::Absent(p));
    }
    let conn = Connection::open_with_flags(
        &p,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA query_only = 1")?;
    check(&conn)?;
    Ok(conn)
}

/// The table check, then the version check.
pub fn check(conn: &Connection) -> Result<(), Error> {
    let extra: Vec<String> = schema::tables(conn)?
        .into_iter()
        .filter(|t| !TABLES.contains(&t.as_str()))
        .collect();
    if !extra.is_empty() {
        return Err(Error::Tables(extra));
    }
    let found = schema::version(conn)?;
    if found != VERSION {
        return Err(Error::Version {
            found,
            want: VERSION,
        });
    }
    Ok(())
}
