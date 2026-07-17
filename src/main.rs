use std::process::ExitCode;

fn main() -> ExitCode {
    sigpipe::reset();
    typst_pack::cli::run()
}
