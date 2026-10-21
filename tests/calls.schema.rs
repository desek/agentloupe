//! @agents-index Integration test: the calls table requires exactly one of exchange_id and log_key, and log_key is unique.

fn db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = agent_loupe::store::open::open(dir.path()).unwrap();
    agent_loupe::store::migrate::migrate(&mut c).unwrap();
    c.execute_batch(
        "INSERT INTO sessions (id, agent_label, key_hash, key_source, segment, started_at, last_call_at, state, call_count) VALUES (1, 'pi', 'k', 'log', 1, 'x', 'x', 'open', 1);
         INSERT INTO exchanges (id, started_at, ended_at, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_truncated, response_truncated, outcome) VALUES ('e1', 'x', 'x', 'pi', 'u', 'POST', '/', '', 200, 0, 0, '{}', '{}', 0, 0, 'ok');",
    )
    .unwrap();
    (dir, c)
}

fn insert(
    c: &rusqlite::Connection,
    exchange: Option<&str>,
    log_key: Option<&str>,
    origin: &str,
) -> rusqlite::Result<usize> {
    c.execute(
        "INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, session_id, status, input, cache_read, cache_write_5m, cache_write_1h, output, origin, log_key) \
         VALUES (?1, 0, 'x', 'pi', 'u', 1, 200, 0, 0, 0, 0, 0, ?3, ?2)",
        rusqlite::params![exchange, log_key, origin],
    )
}

#[test]
fn exchange_or_log_key_required() {
    let (_d, c) = db();
    assert!(insert(&c, Some("e1"), Some("lk"), "log").is_err());
    assert!(insert(&c, None, None, "log").is_err());
    assert!(insert(&c, None, Some("lk"), "other").is_err());
    insert(&c, Some("e1"), None, "proxy").unwrap();
    insert(&c, None, Some("lk"), "log").unwrap();
    assert!(insert(&c, None, Some("lk"), "log").is_err());
    let origin: String = c
        .query_row(
            "SELECT origin FROM calls WHERE exchange_id = 'e1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(origin, "proxy");
}
