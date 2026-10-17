mod cli;
mod commands;
mod output;

use std::process::ExitCode;

fn main() -> ExitCode {
    sigpipe::reset();
    commands::run()
}
