//! @agents-index Integration test: full session and session day summaries per session and day, their requests, status chain, and sources.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/session_days.rs"]
mod sd;
#[path = "common/summary.rs"]
mod summary;

use sd::{DAY_MARK, FULL_MARK};

const FULL: &str = "session_full_summaries";
const DAY: &str = "session_day_summaries";

fn dirs() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn five_day_session_gets_five_of_each() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "s5", &sd::days("s5", 5));
    sd::backfill(data.path(), &root, fake, Some("rec_five"));
    for table in [FULL, DAY] {
        let rows = sd::statuses(data.path(), table);
        assert_eq!(rows.len(), 5, "{table}: {rows:?}");
        assert!(rows.iter().all(|(_, s)| s == "done"), "{rows:?}");
    }
    assert_eq!(sd::requests("rec_five", FULL_MARK).len(), 5);
    assert_eq!(sd::requests("rec_five", DAY_MARK).len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn day_summary_request_holds_full_summary() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "s2", &sd::days("s2", 2));
    sd::backfill(data.path(), &root, fake, Some("rec_ctx"));
    let texts: Vec<String> = bf::db(data.path())
        .prepare(&format!("SELECT text FROM {FULL} ORDER BY day"))
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let days = sd::requests("rec_ctx", DAY_MARK);
    assert_eq!(days.len(), 2);
    for (d, (_, user)) in days.iter().enumerate() {
        assert!(
            user.contains(&format!("<summary>\n{}\n</summary>", texts[d])),
            "{user}"
        );
        assert!(user.contains(&format!("DAYTEXT{}", d + 1)), "{user}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn full_request_holds_transcript_to_day() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "s3", &sd::days("s3", 3));
    sd::backfill(data.path(), &root, fake, Some("rec_to_day"));
    let full = sd::requests("rec_to_day", FULL_MARK);
    let day = sd::requests("rec_to_day", DAY_MARK);
    assert_eq!((full.len(), day.len()), (3, 3));
    // The requests run in day order; index 1 is day 2.
    let (system, user) = &full[1];
    assert!(system.contains("at most 8 sentences"));
    assert!(user.contains("DAYTEXT1") && user.contains("DAYTEXT2") && !user.contains("DAYTEXT3"));
    let (system, user) = &day[1];
    assert!(system.contains("at most 4 sentences"));
    let transcript = user.split("<transcript>").nth(1).unwrap();
    assert!(transcript.contains("DAYTEXT2"), "{transcript}");
    assert!(!transcript.contains("DAYTEXT1") && !transcript.contains("DAYTEXT3"));
}

#[tokio::test(flavor = "multi_thread")]
async fn skipped_full_summary_skips_day_summary() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "s1", &sd::days("s1", 1));
    sd::backfill(data.path(), &root, fake, Some("rec_word"));
    assert_eq!(sd::statuses(data.path(), FULL)[0].1, "skipped");
    assert_eq!(sd::statuses(data.path(), DAY)[0].1, "skipped");
    assert_eq!(sd::requests("rec_word", FULL_MARK).len(), 1);
    assert_eq!(sd::requests("rec_word", DAY_MARK).len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_and_disabled_full_summary_chain() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "sf", &sd::days("sf", 1));
    sd::backfill(data.path(), &root, fake, Some("rec_401"));
    assert_eq!(sd::statuses(data.path(), FULL)[0].1, "failed");
    assert_eq!(sd::statuses(data.path(), DAY)[0].1, "failed");
    assert_eq!(sd::requests("rec_401", DAY_MARK).len(), 0);

    let (logs, data) = dirs();
    let root = sd::tree(logs.path(), "sn", &sd::days("sn", 1));
    sd::backfill(data.path(), &root, fake, None);
    assert_eq!(sd::statuses(data.path(), FULL)[0].1, "disabled");
    assert_eq!(sd::statuses(data.path(), DAY)[0].1, "disabled");
    assert_eq!(fake::summary_hits("none"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn tool_only_segment_gets_day_text() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let lines = vec![
        sd::user("st", "please fix the scrubber rules now", "2026-01-01T08:00:00Z"),
        sd::answer("st", "m1", "fixed the scrubber rules", "2026-01-01T08:00:01Z"),
        sd::tool_call("st", "m2", "2026-01-01T20:00:00Z"),
    ];
    let root = sd::tree(logs.path(), "st", &lines);
    sd::backfill(data.path(), &root, fake, Some("rec_tool"));
    assert_eq!(
        bf::scalar(data.path(), "SELECT count(*) FROM sessions"),
        2,
        "two segments on day 1"
    );
    assert_eq!(
        sd::statuses(data.path(), DAY),
        vec![("2026-01-01".to_string(), "done".to_string())]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn open_segment_blocks_full_summary() {
    let fake = fake::spawn().await;
    let (logs, data) = dirs();
    let at = |secs: i64| {
        (chrono::Utc::now() - chrono::Duration::seconds(secs))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string()
    };
    let lines = vec![
        sd::user("so", "please keep working on it", &at(120)),
        sd::answer("so", "m1", "working", &at(60)),
    ];
    let root = sd::tree(logs.path(), "so", &lines);
    sd::backfill(data.path(), &root, fake, Some("rec_open"));
    assert_eq!(
        bf::scalar(data.path(), "SELECT count(*) FROM sessions WHERE state = 'open'"),
        1
    );
    assert_eq!(sd::requests("rec_open", FULL_MARK).len(), 0);
    for table in [FULL, DAY] {
        assert!(
            sd::statuses(data.path(), table)
                .iter()
                .all(|(_, s)| s == "pending"),
            "{table}"
        );
    }
}

/// Sends one proxied call of session `sid` with the user turns `turns` as its history.
async fn send(l: &loupe::Loupe, sid: &str, turns: &[&str]) {
    let mut messages = vec![];
    for t in turns {
        messages.push(serde_json::json!({"role":"user","content":t}));
        messages.push(serde_json::json!({"role":"assistant","content":"done"}));
    }
    messages.pop();
    let before = l.rows();
    let r = reqwest::Client::new()
        .post(l.url("/pi/fake/v1/messages"))
        .header("x-session-id", sid)
        .json(&serde_json::json!({"model":"claude-haiku-5-5","messages":messages}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.bytes().await.unwrap();
    l.wait_rows(before + 1);
}

/// A proxied session with a closed segment yesterday (`day1` histories) and one today (`day2`).
async fn two_day_proxied(fake: u16, day1: &[&[&str]], day2: &[&[&str]]) -> loupe::Loupe {
    let l = loupe::Loupe::start(fake, &[], summary::QUIET);
    for h in day1 {
        send(&l, "p1", h).await;
    }
    loupe::parse_and_close(l.data.path());
    l.db()
        .execute_batch(
            "UPDATE calls SET started_at = strftime('%Y-%m-%dT%H:%M:%fZ', started_at, '-1 day'); \
             UPDATE sessions SET started_at = strftime('%Y-%m-%dT%H:%M:%fZ', started_at, '-1 day'), \
             last_call_at = strftime('%Y-%m-%dT%H:%M:%fZ', last_call_at, '-1 day'), \
             closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', closed_at, '-1 day');",
        )
        .unwrap();
    for h in day2 {
        send(&l, "p1", h).await;
    }
    loupe::parse_and_close(l.data.path());
    assert_eq!(
        summary::scalar(&l, "SELECT count(*) FROM sessions WHERE state = 'closed'"),
        2
    );
    l
}

fn summarize(l: &loupe::Loupe, fake: u16, mode: &str) {
    let url = summary::base(fake, mode);
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[
            ("LOUPE_TZ", "UTC"),
            ("LOUPE_SUMMARY_API_KEY", "sk-test-Pr0x"),
            ("LOUPE_SUMMARY_BASE_URL", &url),
        ],
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[tokio::test(flavor = "multi_thread")]
async fn proxied_session_reads_all_segments() {
    let fake = fake::spawn().await;
    let l = two_day_proxied(
        fake,
        &[&["DAYONE first", "DAYONE second", "DAYONE third"]],
        &[&["DAYTWO only"]],
    )
    .await;
    summarize(&l, fake, "rec_proxied_all");
    let full = sd::requests("rec_proxied_all", FULL_MARK);
    assert_eq!(full.len(), 2);
    // The day 2 request holds the longest history of both segments: the one of day 1.
    assert!(full[1].1.contains("DAYONE third"), "{}", full[1].1);
    assert_eq!(
        summary::scalar(&l, "SELECT count(*) FROM session_day_summaries WHERE status = 'done'"),
        2
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn proxied_day_transcript_holds_new_messages() {
    let fake = fake::spawn().await;
    let l = two_day_proxied(
        fake,
        &[&["DAYONE ask"]],
        &[&["DAYONE ask", "DAYTWO ask"]],
    )
    .await;
    summarize(&l, fake, "rec_proxied_new");
    let day = sd::requests("rec_proxied_new", DAY_MARK);
    assert_eq!(day.len(), 2);
    let t1 = day[0].1.split("<transcript>").nth(1).unwrap();
    assert!(t1.contains("DAYONE ask"), "{t1}");
    let t2 = day[1].1.split("<transcript>").nth(1).unwrap();
    assert!(t2.contains("DAYTWO ask") && !t2.contains("DAYONE"), "{t2}");
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_bodies_use_remaining_and_log() {
    let fake = fake::spawn().await;
    let l = two_day_proxied(
        fake,
        &[&["DAYONE short"], &["DAYONE short", "DAYONE long"]],
        &[&["DAYTWO new"]],
    )
    .await;
    // The longest body of day 1 is gone before the build.
    l.db()
        .execute_batch(
            "UPDATE exchanges SET request_body = NULL, bodies_deleted_at = started_at \
             WHERE rowid = (SELECT max(e.rowid) FROM exchanges e JOIN calls c ON c.exchange_id = e.id \
             WHERE c.started_at < (SELECT min(started_at) FROM sessions WHERE segment = 2))",
        )
        .unwrap();
    summarize(&l, fake, "rec_missing");
    let full = sd::requests("rec_missing", FULL_MARK);
    assert_eq!(full.len(), 2);
    assert!(full[1].1.contains("DAYONE short") || full[1].1.contains("DAYTWO new"));
    assert!(!full[1].1.contains("DAYONE long"));
    let lines: Vec<serde_json::Value> = l
        .log()
        .lines()
        .filter(|x| x.contains("bodies_missing"))
        .map(|x| serde_json::from_str(x).unwrap())
        .collect();
    assert!(!lines.is_empty());
    for line in &lines {
        let mut keys: Vec<&String> = line["fields"].as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["day", "error_kind", "session_key"]);
        assert!(!line.to_string().contains("DAYONE"));
    }
}
