//! @agents-index Entry point of the loupe binary; parses the command line and hands control to the subcommand.

mod capture;
mod cli;
mod config;
mod log;
mod proxy;
mod serve;
mod store;

fn main() -> std::process::ExitCode {
    cli::run()
}
