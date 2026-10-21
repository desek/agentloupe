//! @agents-index Integration test: build_day_line reads the session day summaries, waits on pending or open, and orders the fallback.

#[path = "common/db.rs"]
mod db;
#[path = "common/fake_upstream.rs"]
mod fake;

use agent_loupe::summary::day::{Settings, build_day_line};

const DAY: &str = "session_day_summaries";

fn date() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()
}

fn build(conn: &rusqlite::Connection, url: Option<&str>) -> agent_loupe::summary::day::DayResult {
    let c = url.map(db::client);
    let s = Settings {
        client: c.as_ref(),
        public: &[],
        max_attempts: 1,
        tz: chrono_tz::UTC,
    };
    build_day_line(conn, &s, date(), false).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn day_line_reads_session_day_summaries() {
    let fake = fake::spawn().await;
    let (_d, conn) = db::open();
    for (k, text) in [
        ("ka", "Worked on TEXTALPHA today."),
        ("kb", "Worked on TEXTBETA today."),
    ] {
        db::segment(&conn, k, "closed", "2026-01-02T10:00:00.000Z", 1);
        db::text_row(&conn, DAY, k, "2026-01-02", "done", Some(text));
    }
    // A session line of the same day is not read.
    conn.execute_batch("INSERT INTO session_summaries (session_id, status, line, attempts, created_at, updated_at) VALUES (1, 'done', 'TEXTLINE', 1, 'x', 'x')").unwrap();
    let url = format!("http://127.0.0.1:{fake}/summary/rec_dayline");
    let r = tokio::task::spawn_blocking(move || {
        let r = build(&conn, Some(&url));
        let n: i64 = conn
            .query_row(
                "SELECT session_lines FROM day_summaries WHERE day = '2026-01-02'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (r.session_lines, n)
    })
    .await
    .unwrap();
    assert_eq!(r, (2, 2));
    let bodies = fake::summary_bodies("rec_dayline");
    assert_eq!(bodies.len(), 1);
    let user = bodies[0]["messages"][0]["content"].as_str().unwrap();
    assert!(user.contains("TEXTALPHA") && user.contains("TEXTBETA") && !user.contains("TEXTLINE"));
}

#[test]
fn day_line_not_ready_while_pending_or_open() {
    let (_d, conn) = db::open();
    db::segment(&conn, "ka", "closed", "2026-01-02T10:00:00.000Z", 1);
    db::text_row(&conn, DAY, "ka", "2026-01-02", "pending", None);
    assert_eq!(build(&conn, None).status, "not_ready");

    let (_d, conn) = db::open();
    db::segment(&conn, "kb", "open", "2026-01-02T10:00:00.000Z", 1);
    assert_eq!(build(&conn, None).status, "not_ready");
    let n: i64 = conn
        .query_row("SELECT count(*) FROM day_summaries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn fallback_orders_by_calls_at_most_eight() {
    let (_d, conn) = db::open();
    for i in 1..=10i64 {
        let k = format!("k{i:02}");
        db::segment(&conn, &k, "closed", "2026-01-02T10:00:00.000Z", i);
        let text = format!(
            "Worked on part {} of it.",
            [
                "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
                "ten"
            ][i as usize]
        );
        db::text_row(&conn, DAY, &k, "2026-01-02", "done", Some(&text));
    }
    let r = build(&conn, None);
    assert_eq!(r.status, "fallback");
    assert_eq!(r.session_lines, 10);
    let line = r.line.unwrap();
    let sentences: Vec<&str> = line.split(". ").collect();
    assert_eq!(sentences.len(), 8, "{line}");
    assert!(
        sentences[0].contains("ten") && sentences[7].contains("three"),
        "{line}"
    );
}
