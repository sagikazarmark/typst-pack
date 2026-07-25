mod cli;

use std::process::ExitCode;

fn main() -> ExitCode {
    sigpipe::reset();
    cli::run()
}
