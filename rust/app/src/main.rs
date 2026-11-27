mod cli;
mod units;

use std::process::ExitCode;

fn main() -> ExitCode {
    let matches = cli::command().get_matches();
    let (name, args) = matches.subcommand().unwrap_or((cli::NAME, &matches));
    units::dispatch(name, args).unwrap_or_else(|| {
        eprintln!("{}: unknown command `{name}`", cli::NAME);
        ExitCode::from(2)
    })
}
