//! @agents-index The scrubber: removes URLs, emails, paths, and tokens with a digit from a session line or a day paragraph.

// Self-contained on purpose: the corpus test includes this file by path to measure the leak rate.

/// Words that cannot end a line or a sentence, because a removed token left them dangling.
const DANGLING: &[&str] = &[
    "at", "in", "on", "for", "to", "from", "with", "of", "into", "via", "and", "the", "a",
];

/// The most words of a session line.
const MAX_WORDS: usize = 12;
/// The most sentences of a day paragraph.
const MAX_SENTENCES: usize = 8;

/// Splits `LOUPE_PUBLIC_PROJECTS`: comma-separated, trimmed, empty entries ignored.
pub fn public_projects(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn core(token: &str) -> &str {
    token.trim_matches(|c: char| !c.is_alphanumeric())
}

fn is_public(token: &str, public: &[String]) -> bool {
    let c = core(token);
    !c.is_empty() && public.iter().any(|p| core(p).eq_ignore_ascii_case(c))
}

/// A dot followed by 1 to 5 letters at the end of the token core, as in `main.rs` or `a.json`.
fn has_extension(token: &str) -> bool {
    let c = core(token);
    c.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && (1..=5).contains(&ext.len())
            && ext.chars().all(|ch| ch.is_ascii_alphabetic())
    })
}

/// True when the token is a URL, an email address, a path, or has a digit.
fn identifying(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.contains("://")
        || lower.starts_with("www.")
        || token.contains('@')
        || token.contains('/')
        || token.contains('\\')
        || has_extension(token)
        || token.chars().any(|c| c.is_ascii_digit())
}

/// Removes the identifying tokens, except a public project name, and collapses whitespace.
fn remove_tokens(text: &str, public: &[String]) -> Vec<String> {
    text.split_whitespace()
        .filter(|t| is_public(t, public) || !identifying(t))
        .map(|t| {
            t.replace(
                [
                    '"', '`', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}', '\'',
                ],
                "",
            )
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// Removes trailing punctuation and dangling words until the last word is neither.
fn trim_tail(words: &mut Vec<String>) {
    loop {
        let Some(last) = words.last_mut() else { return };
        let trimmed = last
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_string();
        if trimmed.is_empty() || DANGLING.contains(&trimmed.to_ascii_lowercase().as_str()) {
            words.pop();
            continue;
        }
        *last = trimmed;
        return;
    }
}

/// The scrubbed session line, or None when fewer than 2 words remain.
pub fn line(text: &str, public: &[String]) -> Option<String> {
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut words = remove_tokens(first, public);
    words.truncate(MAX_WORDS);
    trim_tail(&mut words);
    if words.len() < 2 {
        return None;
    }
    let joined = words.join(" ");
    let mut chars = joined.chars();
    let head = chars.next().map(|c| c.to_lowercase().collect::<String>());
    Some(head.unwrap_or_default() + chars.as_str())
}

/// Splits a paragraph into sentences at `.`, `!`, or `?` followed by whitespace or the end.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| n.is_whitespace()) {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// The scrubbed day paragraph: at most 8 sentences of 2 or more words, each with a final period.
pub fn paragraph(text: &str, public: &[String]) -> String {
    sentences(text)
        .iter()
        .filter_map(|s| {
            let mut words = remove_tokens(s, public);
            trim_tail(&mut words);
            (words.len() >= 2).then(|| format!("{}.", words.join(" ")))
        })
        .take(MAX_SENTENCES)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The count of sentences of a scrubbed paragraph.
pub fn sentence_count(paragraph: &str) -> usize {
    sentences(paragraph).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_identifying_tokens() {
        let out = line(
            "fixed https://x.io/a and src/main.rs for bob@corp.com in 42 places today",
            &[],
        )
        .unwrap();
        for bad in ["http", "x.io", "main.rs", "@", "42"] {
            assert!(!out.contains(bad), "{out}");
        }
        assert_eq!(out, "fixed and for in places today");
    }

    #[test]
    fn keeps_public_project_names() {
        let p = public_projects(" agentloupe , ,");
        let out = line("improved Agentloupe and acme-repo2 under acme/web", &p).unwrap();
        assert!(out.contains("Agentloupe") && !out.contains("acme"), "{out}");
        let out = line("fixed the agentloupe parser and acme/web", &p).unwrap();
        assert_eq!(out, "fixed the agentloupe parser");
    }

    #[test]
    fn day_paragraph_sentences() {
        let text: String = (1..=10)
            .map(|i| {
                if i == 2 {
                    "Read https://example.com docs today.".to_string()
                } else {
                    format!("Worked on part {} of the parser.", ["one", "two"][i % 2])
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let out = paragraph(&text, &[]);
        assert!(!out.contains("http"));
        assert!(sentence_count(&out) <= 8);
        assert!(sentences(&out).iter().all(|s| s.trim_end().ends_with('.')));
    }

    #[test]
    fn cuts_to_twelve_words_and_dangling() {
        let text =
            "updated one two three four five six seven eight nine for the more words here now";
        let out = line(text, &[]).unwrap();
        assert!(out.split(' ').count() <= 12);
        let out = line("updated a b c d e f g h i for the", &[]).unwrap();
        assert!(!out.ends_with(" for") && !out.ends_with(" the"), "{out}");
        assert_eq!(out, "updated a b c d e f g h i");
    }

    #[test]
    fn short_line_is_none() {
        assert_eq!(line("fixed 42", &[]), None);
    }
}
