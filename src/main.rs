//! @agents-index Entry point of the loupe binary; parses the command line and hands control to the subcommand.

fn main() -> std::process::ExitCode {
    agent_loupe::cli::run()
}
