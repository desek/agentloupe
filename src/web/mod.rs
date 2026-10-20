//! @agents-index The dashboard files: the built-in web/ files with their content types and ETags, and the static handler.

pub mod embed;
pub mod etag;
pub mod headers;
pub mod path;
pub mod serve;
pub mod title;

use std::borrow::Cow;
use std::collections::HashMap;

/// One servable file: the bytes, the quoted ETag of those bytes, and the content type.
pub struct File {
    pub body: Cow<'static, [u8]>,
    pub etag: String,
    pub content_type: &'static str,
}

/// Every built-in file by its path relative to web/, made once at start. `index.html` holds the
/// site title, so its ETag is of the content after the title is put in.
pub struct Site {
    files: HashMap<&'static str, File>,
}

impl Site {
    /// Puts `site_title` into index.html and computes every ETag. The title is already validated.
    pub fn new(site_title: &str) -> Site {
        let files = embed::FILES
            .iter()
            .filter_map(|(path, bytes)| {
                let content_type = embed::content_type(path)?;
                let body: Cow<'static, [u8]> = if *path == "index.html" {
                    Cow::Owned(
                        title::apply(&String::from_utf8_lossy(bytes), site_title).into_bytes(),
                    )
                } else {
                    Cow::Borrowed(*bytes)
                };
                let etag = etag::of(&body);
                Some((
                    *path,
                    File {
                        body,
                        etag,
                        content_type,
                    },
                ))
            })
            .collect();
        Site { files }
    }

    pub fn get(&self, path: &str) -> Option<&File> {
        self.files.get(path)
    }
}
