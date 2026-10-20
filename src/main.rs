//! @agents-index Entry point of the loupe binary; parses the command line and hands control to the subcommand.

mod account;
mod capture;
mod cli;
mod config;
mod log;
mod parse;
mod proxy;
mod serve;
mod session;
mod store;
mod token;

fn main() -> std::process::ExitCode {
    cli::run()
}
