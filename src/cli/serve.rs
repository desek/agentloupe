//! @agents-index The serve subcommand: validates the configuration, starts logging, then runs the server until a signal.

use super::ConfigArgs;
use crate::config::loader;
use std::process::ExitCode;

/// Exit code for a configuration that is missing or not valid; the port is never opened.
const EXIT_CONFIG: u8 = 2;
/// Exit code for a runtime failure after the configuration passed.
const EXIT_RUNTIME: u8 = 1;

pub fn run(args: &ConfigArgs) -> ExitCode {
    // The guards of the role public run before the validation of the values and before any file opens.
    let cfg = match crate::provider::legacy::check_process()
        .and_then(|()| loader::load_from_process(&args.as_flags()))
        .map_err(|e| e.to_string())
        .and_then(|c| {
            let role = c.role().map_err(|e| e.to_string())?;
            if role == crate::config::role::Role::Public {
                crate::serve::public::guard::no_secrets()?;
            }
            let cfg = c.validate_for(role).map_err(|e| e.to_string())?;
            if role == crate::config::role::Role::Public {
                crate::serve::public::guard::no_private_db(&cfg.data_dir)?;
            }
            Ok(cfg)
        }) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let public = cfg.role == crate::config::role::Role::Public;
    // The allowlist is checked before the log starts and before the port opens.
    // The role public never loads the upstreams file.
    let upstreams = match public {
        true => None,
        false => match crate::proxy::upstreams::load(&cfg.upstreams_file) {
            Ok(u) => Some(u),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(EXIT_CONFIG);
            }
        },
    };
    if let Err(e) = crate::log::init_file(&cfg.data_dir, cfg.role.log_file()) {
        eprintln!(
            "error: cannot write the log file under {}: {e}. Fix: make LOUPE_DATA_DIR writable for this user, or point LOUPE_DATA_DIR at a writable directory. See docs/specification/operations.md for an old container volume. Verify: loupe serve starts and GET /healthz returns 200.",
            cfg.data_dir.display()
        );
        return ExitCode::from(EXIT_RUNTIME);
    }
    // The first line of each start names the build, so a log or a bug report shows which release ran.
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        role = cfg.role.name(),
        "loupe serve starting"
    );
    let result = match upstreams {
        Some(u) => crate::serve::run(&cfg, u),
        None => crate::serve::public::run(&cfg),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "serve stopped with an error");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}
