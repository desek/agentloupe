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
}
