//! @agents-index Measurement test: runs the scrubber over the corpus file and prints leaks, which must be 0.

#[allow(dead_code, unused_imports)]
#[path = "../src/summary/scrub.rs"]
mod scrub;

/// The public projects of the measurement, as LOUPE_PUBLIC_PROJECTS would give them.
const PUBLIC: &str = "agentloupe, loupe2";

/// True when a token of the output is one that the rules remove and is not a public name.
fn identifying_left(out: &str, public: &[String]) -> bool {
    out.split_whitespace().any(|t| {
        let core = t.trim_matches(|c: char| !c.is_alphanumeric());
        let public_name = public.iter().any(|p| p.eq_ignore_ascii_case(core));
        !public_name
            && (t.contains("://")
                || t.contains('@')
                || t.contains('/')
                || t.contains('\\')
                || t.chars().any(|c| c.is_ascii_digit()))
    })
}

fn scrub_corpus_has_no_leaks() {
    let corpus = include_str!("fixtures/summary/scrub_corpus.txt");
    let public = scrub::public_projects(PUBLIC);
    let (mut lines, mut leaks, mut lost) = (0, 0, 0);
    for row in corpus
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let cols: Vec<&str> = row.split('\t').collect();
        let out = match cols[0] {
            "line" => scrub::line(cols[1], &public).unwrap_or_default(),
            _ => scrub::paragraph(cols[1], &public),
        };
        lines += 1;
        let forbidden = cols
            .get(2)
            .map_or(vec![], |c| c.split('|').filter(|s| !s.is_empty()).collect());
        if forbidden.iter().any(|f| out.contains(f)) || identifying_left(&out, &public) {
            leaks += 1;
            eprintln!("leak: {out}");
        }
        let keep = cols
            .get(3)
            .map_or(vec![], |c| c.split('|').filter(|s| !s.is_empty()).collect());
        if keep
            .iter()
            .any(|k| !out.to_lowercase().contains(&k.to_lowercase()))
        {
            lost += 1;
            eprintln!("lost public name: {out}");
        }
    }
    println!("lines: {lines}");
    println!("leaks: {leaks}");
    println!("lost_public_names: {lost}");
    assert!(lines >= 50, "the corpus needs at least 50 lines");
    assert_eq!(leaks, 0);
    assert_eq!(lost, 0);
}

/// Runs without the test harness, so `cargo test` prints the measured counts on success too.
fn main() {
    scrub_corpus_has_no_leaks();
    println!("test scrub_corpus_has_no_leaks ... ok");
}
