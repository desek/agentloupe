//! @agents-index Integration test: after a GitHub fetch, no table and no log line holds the owner, repository, number, title, URL, login, or token.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_github.rs"]
mod fake_github;

const D: &str = "2026-10-18";
const TOKEN: &str = "ghp_privacyTokenValue9182736455";
const LOGIN: &str = "secretlogin-zq";

/// Every value of every table as text.
fn dump(c: &rusqlite::Connection) -> String {
    let tables: Vec<String> = c
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut out = String::new();
    for t in tables {
        let mut st = c.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
        let n = st.column_count();
        let mut rows = st.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            for i in 0..n {
                let v: rusqlite::types::Value = r.get(i).unwrap();
                out.push_str(&format!("{v:?}\n"));
            }
        }
    }
    out
}

#[test]
fn tables_hold_no_repo_number_url_or_token() {
    let mut w = fake_github::world(TOKEN, LOGIN);
    let mut p = fake_github::pr(LOGIN, 4_242_421, D, Some(D));
    p.owner = "PrivOwnerXq".into();
    p.repo = "priv-repo-yk".into();
    p.title = "Hidden title of the change".into();
    w.prs = vec![p];
    let f = fake_github::start(w);
    let (dir, c) = days::db();
    let out = days::loupe(
        dir.path(),
        &["aggregate", "--date", D],
        &[
            ("LOUPE_GITHUB_TOKEN_PRIV", TOKEN),
            ("LOUPE_GITHUB_API_URL", &f.url),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM github_prs"), 1);
    assert_eq!(days::record(&c, D).unwrap()["prs_merged"], 1);
    let text = format!(
        "{}\n{}\n{}",
        dump(&c),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_lowercase();
    for needle in [
        "privownerxq",
        "priv-repo-yk",
        "4242421",
        "hidden title",
        "github.com/",
        LOGIN,
        &TOKEN.to_lowercase(),
        &TOKEN[4..14].to_lowercase(),
    ] {
        assert!(!text.contains(needle), "found {needle}");
    }
}
