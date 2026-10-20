//! @agents-index Configuration of loupe: resolution by precedence, validation, secret handling, and the show output.

pub mod error;
pub mod key;
pub mod loader;
pub mod secret;
pub mod show;
mod validate;
mod vars;

/// The validated values that the running binary reads. Values for later product areas are
/// validated by the loader but not kept, so they cannot change behaviour yet.
pub struct Config {
    pub listen: std::net::SocketAddr,
    pub data_dir: std::path::PathBuf,
    /// Seals the captured bodies and the spill files.
    pub data_key: [u8; key::KEY_LEN],
    /// Keys the HMAC of a credential and of an account ID header.
    pub account_key: [u8; key::KEY_LEN],
    /// The allowlist of upstreams; a relative path resolves against the working directory.
    pub upstreams_file: std::path::PathBuf,
    /// The capture limit: the maximum bytes of one body that the capture keeps.
    pub capture_max_bytes: usize,
    /// The count of exchange records that the queue to the capture writer holds.
    pub capture_queue: usize,
    pub connect_timeout: std::time::Duration,
    /// From the end of the request send to the response headers.
    pub headers_timeout: std::time::Duration,
}
