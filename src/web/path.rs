//! @agents-index Maps a request path of the dashboard to a built-in file name, and rejects traversal and encoded separators.

/// True for the paths that the static handler owns: `/`, `/favicon.ico`, and `/assets/...`.
pub fn is_static(path: &str) -> bool {
    path == "/" || path == "/favicon.ico" || path == "/assets" || path.starts_with("/assets/")
}

/// The file name relative to web/ for a static path, or None for a path that must answer 404.
/// The raw path is checked before any lookup, so `..`, a backslash, or an encoded slash never
/// reaches the map, even in an encoded form.
pub fn resolve(path: &str) -> Option<&str> {
    let lower = path.to_ascii_lowercase();
    let bad = ["..", "\\", "%2f", "%5c", "%2e"];
    if bad.iter().any(|b| lower.contains(b)) {
        return None;
    }
    match path {
        "/" => Some("index.html"),
        "/favicon.ico" => Some("favicon.ico"),
        _ => path.strip_prefix("/").filter(|p| p.starts_with("assets/")),
    }
}
