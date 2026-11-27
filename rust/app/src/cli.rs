use std::ffi::OsString;

use clap::{Arg, Command, value_parser};

pub const NAME: &str = "views_to_slices";

fn tail() -> Arg {
    Arg::new("args")
        .num_args(0..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .value_parser(value_parser!(OsString))
}

pub fn command() -> Command {
    Command::new(NAME)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(tail())
}
