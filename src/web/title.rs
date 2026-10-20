//! @agents-index Puts the escaped site title into index.html at start, in place of the title placeholder.

/// The marker in web/index.html that the site title replaces, in the title element and the header.
pub const PLACEHOLDER: &str = "{{site_title}}";

/// Escapes the five HTML characters, so a title can never add markup to the page.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn apply(html: &str, title: &str) -> String {
    html.replace(PLACEHOLDER, &escape(title))
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_all_five() {
        assert_eq!(
            super::escape(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }
}
