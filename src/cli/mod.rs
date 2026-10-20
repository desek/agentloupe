//! @agents-index Command-line surface of loupe: the clap definitions and the dispatch to each subcommand.

mod config_show;
mod gen_key;
mod serve;

use clap::{Args, Parser, Subcommand};
use std::process::ExitCode;

/// loupe records AI agent activity and publishes it as a daily line.
#[derive(Parser)]
#[command(name = "loupe", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load the configuration, open and migrate the database, then listen on LOUPE_LISTEN.
    Serve(ConfigArgs),
    /// Print one new random 32-byte key in standard base64 with padding. Stores nothing.
    GenKey,
    /// Inspect the configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Print each effective value with its source (flag, env, file, default). Secrets print as set or unset.
    Show {
        #[command(flatten)]
        args: ConfigArgs,
        /// Print a table instead of one JSON object per line.
        #[arg(long)]
        human: bool,
    },
}

/// Flags that override the environment, the file for the environment, and the defaults.
#[derive(Args, Clone, Default)]
pub struct ConfigArgs {
    /// Name of the environment; selects config/<env>.toml. Overrides LOUPE_ENV. Default: local.
    #[arg(long)]
    pub env: Option<String>,
    /// Address to listen on, as host:port. Overrides LOUPE_LISTEN.
    #[arg(long)]
    pub listen: Option<String>,
    /// Directory for the database and the logs. Overrides LOUPE_DATA_DIR.
    #[arg(long)]
    pub data_dir: Option<String>,
    /// Seconds without a call before a session ends. Overrides LOUPE_IDLE_SECS.
    #[arg(long)]
    pub idle_secs: Option<String>,
}

impl ConfigArgs {
    /// The flags as variable name and value pairs, the shape that the loader takes.
    pub fn as_flags(&self) -> Vec<(String, String)> {
        [
            ("LOUPE_ENV", &self.env),
            ("LOUPE_LISTEN", &self.listen),
            ("LOUPE_DATA_DIR", &self.data_dir),
            ("LOUPE_IDLE_SECS", &self.idle_secs),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.clone().map(|v| (k.to_string(), v)))
        .collect()
    }
}

/// Parses the command line and runs the selected subcommand.
pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Serve(args) => serve::run(&args),
        Command::GenKey => gen_key::run(),
        Command::Config {
            command: ConfigCommand::Show { args, human },
        } => config_show::run(&args, human),
    }
}
