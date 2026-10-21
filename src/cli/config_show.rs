//! @agents-index The config show subcommand: loads the configuration and prints every value with its source, secrets masked.

use super::ConfigArgs;
use crate::config::{loader, show};
use std::io::IsTerminal;
use std::process::ExitCode;

pub fn run(args: &ConfigArgs, human: bool) -> ExitCode {
    match crate::provider::legacy::check_process()
        .and_then(|()| loader::load_from_process(&args.as_flags()))
    {
        Ok(cfg) => {
            let table = human || std::io::stdout().is_terminal();
            print!("{}", show::render(&cfg, table));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
