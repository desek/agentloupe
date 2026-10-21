//! @agents-index Integration test: the backfill stores and logs no prompt, path, cwd slug, file name, or raw ID of the logs.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/session_days.rs"]
mod sd;

const MARKERS: &[&str] = &[
    "MARKERPROMPT",
    "MARKERREMINDER",
    "MARKERANSWER",
    "MARKERFILE",
    "MARKERTOOLRESULT",
    "MARKERCWD",
    "MARKERSLUG",
    "MARKERSESS",
    "MARKERMSG",
    "MARKERUUID",
    "MARKERREQ",
    "MARKERBRANCH",
    "MARKERVER",
    "MARKERAGENT",
    "MARKERSUBPROMPT",
    "MARKERPISESS",
    "MARKERPIFILE",
    "MARKERRESP",
    "MARKERSUMMARY",
    "MARKERDAYONE",
    "sk-test-Sx9Pw4",
];

#[tokio::test(flavor = "multi_thread")]
async fn no_prompt_path_or_raw_id_stored() {
    let fake = fake::spawn().await;
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let url = format!("http://127.0.0.1:{fake}/summary/line");
    let env = [
        ("ANTHROPIC_API_KEY", "sk-test-Sx9Pw4"),
        ("ANTHROPIC_BASE_URL", url.as_str()),
    ];
    let claude = bf::claude_tree(logs.path(), true);
    let pi = bf::pi_tree(logs.path());
    bf::write(
        &claude.join(bf::SLUG).join("broken-MARKERFILE.jsonl"),
        "{not json MARKERPROMPT\n",
    );
    // A two-day session with a marker on day 1: the session texts of day 2 read day 1 too.
    bf::write(
        &claude.join(bf::SLUG).join("twoday-MARKERFILE.jsonl"),
        &[
            sd::user(
                "twoday",
                "please fix MARKERDAYONE now",
                "2026-03-01T10:00:00Z",
            ),
            sd::answer("twoday", "d1", "fixed it", "2026-03-01T10:00:01Z"),
            sd::user("twoday", "please continue", "2026-03-02T10:00:00Z"),
            sd::answer("twoday", "d2", "continued", "2026-03-02T10:00:01Z"),
        ]
        .join("\n"),
    );
    let mut stderr = String::new();
    for (source, root) in [("claude-code", &claude), ("pi", &pi)] {
        let o = bf::ok(
            data.path(),
            &["--source", source, "--path", root.to_str().unwrap()],
            &env,
        );
        assert!(loupe::count(&o, "calls_added") > 0);
        assert!(loupe::count(&o, "summaries_done") > 0);
        stderr.push_str(&String::from_utf8_lossy(&o.stderr));
        stderr.push_str(&String::from_utf8_lossy(&o.stdout));
    }
    for table in ["session_full_summaries", "session_day_summaries"] {
        let n = bf::scalar(
            data.path(),
            &format!(
                "SELECT count(*) FROM {table} WHERE status = 'done' AND day IN ('2026-03-01', '2026-03-02')"
            ),
        );
        assert!(n >= 2, "{table} has {n} done rows of the two-day session");
    }
    let bad = bf::run(data.path(), &["--source", "codex"], &env);
    stderr.push_str(&String::from_utf8_lossy(&bad.stderr));
    let mut bytes: Vec<u8> = ["loupe.db", "loupe.db-wal"]
        .iter()
        .flat_map(|f| std::fs::read(data.path().join(f)).unwrap_or_default())
        .collect();
    let log = std::fs::read_to_string(data.path().join("logs/loupe.log")).unwrap();
    assert!(log.contains("malformed log line"));
    bytes.extend(log.as_bytes());
    bytes.extend(stderr.as_bytes());
    for m in MARKERS {
        assert!(!loupe::contains(&bytes, m.as_bytes()), "{m} leaked");
    }
}
