//! Command handlers.

use std::process::ExitCode;

use clap::ArgMatches;

mod scripts;

pub fn dispatch(name: &str, args: &ArgMatches) -> Option<ExitCode> {
    match name {
        crate::cli::NAME => Some(scripts::run(args)),
        _ => None,
    }
}
