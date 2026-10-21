//! @agents-index Integration test: the public API publishes no full session summary and no session day summary.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

const MARKER: &str = "MARKERSESSIONTEXT";

#[tokio::test(flavor = "multi_thread")]
async fn public_api_hides_session_texts() {
    let fake = fake::spawn().await;
    let l = loupe::Loupe::start(fake, &[], &[("LOUPE_TZ", "UTC")]);
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for table in ["session_full_summaries", "session_day_summaries"] {
        l.db()
            .execute(
                &format!("INSERT INTO {table} (session_key, day, status, text, attempts, created_at, updated_at) VALUES ('k', ?1, 'done', ?2, 1, 'x', 'x')"),
                rusqlite::params![day, format!("Worked on {MARKER} today.")],
            )
            .unwrap();
    }
    for path in [
        "/v1/days".to_string(),
        format!("/v1/days/{day}"),
        "/v1/sessions".into(),
        "/v1/calls".into(),
        "/v1/projects".into(),
        "/v1/summary".into(),
        "/healthz".into(),
    ] {
        let body = reqwest::get(l.url(&path))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(!body.contains(MARKER), "{path}: {body}");
    }
}
