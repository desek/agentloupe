//! @agents-index Integration test: build_day_line gives empty, a scrubbed paragraph, or a fallback, and not_ready with an open segment.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/summary.rs"]
mod summary;

use loupe::Loupe;
use summary::{QUIET, base, scalar, send_session, text};

fn day_run(l: &Loupe, day: &str, url: &str, attempts: &str) -> std::process::Output {
    let out = loupe::summarize(
        l.data.path(),
        &["--day", day, "--show-line"],
        &[
            ("LOUPE_SUMMARY_API_KEY", "k"),
            ("LOUPE_SUMMARY_BASE_URL", url),
            ("LOUPE_SUMMARY_MAX_ATTEMPTS", attempts),
            ("LOUPE_TZ", "UTC"),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn field(out: &std::process::Output, key: &str) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}: ")).map(str::to_string))
        .unwrap_or_default()
}

/// A loupe with `n` closed and summarized segments; returns it with the day of the segments.
async fn with_lines(fake: u16, n: usize) -> (Loupe, String) {
    let l = Loupe::start(fake, &[], QUIET);
    for i in 0..n {
        send_session(&l, &format!("d{i}"), 1 + i).await;
    }
    loupe::parse_and_close(l.data.path());
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[
            ("LOUPE_SUMMARY_API_KEY", "k"),
            ("LOUPE_SUMMARY_BASE_URL", &base(fake, "line")),
        ],
    );
    assert_eq!(loupe::count(&out, "done"), n as i64);
    let day = text(&l, "SELECT substr(min(started_at), 1, 10) FROM sessions").unwrap();
    (l, day)
}

#[tokio::test(flavor = "multi_thread")]
async fn day_line_rules() {
    let fake = fake::spawn().await;
    let (l, day) = with_lines(fake, 1).await;
    let out = day_run(&l, "2001-01-01", &base(fake, "day"), "1");
    assert_eq!(field(&out, "status"), "empty");
    assert_eq!(
        text(
            &l,
            "SELECT status FROM day_summaries WHERE day = '2001-01-01'"
        )
        .unwrap(),
        "empty"
    );

    let out = day_run(&l, &day, &base(fake, "day"), "1");
    assert_eq!(field(&out, "status"), "done");
    assert_eq!(field(&out, "session_lines"), "1");

    let (l, day) = with_lines(fake, 3).await;
    let out = day_run(&l, &day, &base(fake, "day"), "1");
    assert_eq!(field(&out, "status"), "done");
    assert_eq!(field(&out, "session_lines"), "3");
    let line = field(&out, "line");
    let sentences = line.matches(". ").count() + 1;
    assert!((3..=8).contains(&sentences), "{line}");
    assert!(!line.contains("http") && line.ends_with('.'), "{line}");
    assert_eq!(
        text(
            &l,
            &format!("SELECT line FROM day_summaries WHERE day = '{day}'")
        )
        .unwrap(),
        line
    );

    let out = day_run(&l, &day, &base(fake, "500"), "1");
    assert_eq!(field(&out, "status"), "fallback");
    assert_eq!(
        field(&out, "line"),
        ["Fixed the parser in per for in places."; 3].join(" ")
    );
    let out = day_run(&l, &day, &base(fake, "day_short"), "1");
    assert_eq!(field(&out, "status"), "fallback");
    assert_eq!(scalar(&l, "SELECT count(*) FROM day_summaries"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn day_not_ready_with_open_segment() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    send_session(&l, "open1", 1).await;
    let out = loupe::parse(l.data.path(), &["--once"]);
    assert!(out.status.success());
    assert_eq!(
        scalar(&l, "SELECT count(*) FROM sessions WHERE state = 'open'"),
        1
    );
    let day = text(&l, "SELECT substr(started_at, 1, 10) FROM sessions").unwrap();
    let out = day_run(&l, &day, &base(fake, "day"), "1");
    assert_eq!(field(&out, "status"), "not_ready");
    assert_eq!(scalar(&l, "SELECT count(*) FROM day_summaries"), 0);
}
