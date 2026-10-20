//! @agents-index The web/ files built into the binary by build.rs, and the content type of each extension.

// FILES: every servable file of web/ as (path relative to web/, bytes); build.rs writes the list.
include!(concat!(env!("OUT_DIR"), "/web_files.rs"));

/// The content type for the extension of `path`, or None for a file that the handler does not serve.
pub fn content_type(path: &str) -> Option<&'static str> {
    Some(match path.rsplit_once('.')?.1 {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "woff2" => "font/woff2",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => return None,
    })
}
