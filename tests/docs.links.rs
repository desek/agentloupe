//! @agents-index Integration test: README.md and the user guides have only resolving relative links and anchors, and a short README.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The pages under check: README.md and each Markdown file under docs/guide.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md")];
    let mut guides: Vec<PathBuf> = std::fs::read_dir(root().join("docs/guide"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    guides.sort();
    out.extend(guides);
    out
}

/// The GitHub anchor of a heading: lowercase, spaces to hyphens, punctuation other than hyphens and underscores dropped.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

fn anchors(text: &str) -> HashSet<String> {
    let mut in_code = false;
    let mut out = HashSet::new();
    for line in text.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
        } else if !in_code && line.starts_with('#') {
            out.insert(anchor(line.trim_start_matches('#')));
        }
    }
    out
}

/// The targets of the inline links and images `](target)` outside code blocks.
fn links(text: &str) -> Vec<String> {
    let mut in_code = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            rest = &rest[i + 2..];
            if let Some(j) = rest.find(')') {
                out.push(rest[..j].to_string());
                rest = &rest[j..];
            }
        }
    }
    out
}

#[test]
fn every_relative_link_and_anchor_resolves() {
    let mut broken = Vec::new();
    for page in pages() {
        let text = std::fs::read_to_string(&page).unwrap();
        for link in links(&text) {
            if link.contains("://") || link.starts_with("mailto:") {
                continue;
            }
            let (path, frag) = link.split_once('#').unwrap_or((&link, ""));
            let target = if path.is_empty() {
                page.clone()
            } else {
                page.parent().unwrap().join(path)
            };
            if !target.exists() {
                broken.push(format!("{}: {link}: no such file", page.display()));
                continue;
            }
            if !frag.is_empty() {
                let body = std::fs::read_to_string(&target).unwrap_or_default();
                if !anchors(&body).contains(frag) {
                    broken.push(format!("{}: {link}: no such heading", page.display()));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "broken links: {broken:#?}. Fix: correct the path or the anchor (GitHub rules: lowercase, spaces to hyphens, punctuation dropped), or add the heading. Verify: cargo nextest run --test docs_links"
    );
}

#[test]
fn readme_is_short_and_shows_the_screenshot_first() {
    let text = std::fs::read_to_string(root().join("README.md")).unwrap();
    let count = text.lines().count();
    assert!(
        count <= 120,
        "README.md has {count} lines, more than 120. Fix: move the detail to a user guide under docs/guide/ and link it. Verify: wc -l README.md"
    );
    let image = text
        .lines()
        .find_map(|l| l.find("![").map(|i| &l[i..]))
        .and_then(|l| l.split_once("](").map(|(_, r)| r.split(')').next().unwrap().to_string()))
        .expect("README.md has no image. Fix: show docs/images/dashboard.png near the top.");
    assert!(
        image.starts_with("docs/images/") && root().join(&image).is_file(),
        "the first image of README.md is {image}, not an existing file under docs/images/. Fix: point it at docs/images/dashboard.png. Verify: ls docs/images"
    );
}
