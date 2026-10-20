//! @agents-index Spill files: seal one exchange record to <data dir>/spill/, import them in start-time order, delete after commit.

use super::insert::insert;
use super::record::ExchangeRecord;
use super::seal::{open, seal};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const PART: &str = "spill";
const EXT: &str = "spill";

/// Writes the sealed record under a temporary name, then renames it, so an import never reads half a file.
/// The name starts with the zero-padded start time, so a sort by name is a sort by start time.
pub fn write(dir: &Path, key: &[u8; 32], r: &ExchangeRecord) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let json = serde_json::to_vec(r).map_err(|e| format!("cannot encode the record: {e}"))?;
    let blob = seal(key, &r.id, PART, &json)?;
    let name = format!("{:020}-{}", r.started_ms.max(0), r.id);
    let tmp = dir.join(format!("{name}.tmp"));
    let path = dir.join(format!("{name}.{EXT}"));
    std::fs::write(&tmp, blob).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("cannot rename {}: {e}", tmp.display()))?;
    Ok(path)
}

fn pending(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == EXT))
        .collect();
    files.sort();
    files
}

fn import_one(conn: &Connection, key: &[u8; 32], path: &Path) -> Result<(), String> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let id = stem.rsplit('-').next().unwrap_or("");
    let blob = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let json = open(key, id, PART, &blob)?;
    let r: ExchangeRecord = serde_json::from_slice(&json)
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
    insert(conn, key, &r)?;
    std::fs::remove_file(path).map_err(|e| format!("cannot delete {}: {e}", path.display()))
}

/// Imports every spill file in start-time order; stops at the first failure and returns the count imported.
pub fn import(conn: &Connection, key: &[u8; 32], dir: &Path) -> Result<usize, String> {
    let mut n = 0;
    for path in pending(dir) {
        import_one(conn, key, &path)?;
        n += 1;
    }
    Ok(n)
}

/// True if a spill file waits for import.
pub fn has_pending(dir: &Path) -> bool {
    !pending(dir).is_empty()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::capture::record::{ExchangeRecord, new_id};

    pub fn sample(started_ms: i64) -> ExchangeRecord {
        ExchangeRecord {
            id: new_id(),
            started_ms,
            ended_ms: started_ms + 5,
            ttfb_ms: Some(1),
            agent: "pi".into(),
            upstream: "anthropic".into(),
            method: "POST".into(),
            path: "/v1/messages".into(),
            query: String::new(),
            status: 200,
            request_size: 2,
            response_size: 2,
            request_headers: serde_json::json!([]),
            response_headers: serde_json::json!([]),
            request_body: b"{}".to_vec(),
            response_body: b"{}".to_vec(),
            request_truncated: false,
            response_truncated: false,
            credential_hash: None,
            credential_source: None,
            account_id_hash: None,
            account_id_source: None,
            outcome: "complete".into(),
            error: None,
            proxy_token_id: None,
        }
    }

    #[test]
    fn spill_import_is_idempotent_and_deletes() {
        let data = tempfile::tempdir().unwrap();
        let mut conn = crate::store::open::open(data.path()).unwrap();
        crate::store::migrate::migrate(&mut conn).unwrap();
        let dir = data.path().join("spill");
        let key = [5u8; 32];
        let paths: Vec<PathBuf> = (0..3)
            .map(|i| write(&dir, &key, &sample(1000 + i)).unwrap())
            .collect();
        let copy = std::fs::read(&paths[0]).unwrap();
        assert_eq!(import(&conn, &key, &dir).unwrap(), 3);
        std::fs::write(&paths[0], copy).unwrap();
        assert_eq!(import(&conn, &key, &dir).unwrap(), 1);
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM exchanges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 3);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }
}
