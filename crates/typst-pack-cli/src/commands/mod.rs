//! Running commands, and what they share: error reporting, diagnostics,
//! timings, and the filesystem environment.

mod compile;
mod create;
mod extract;
mod inspect;

use std::fs::File;
use std::io::{BufWriter, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use typst::WorldExt;
use typst::diag::SourceDiagnostic;
use typst::syntax::Span;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::{
    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
};
use typst_pack::compile::{DocumentTime, Input, Options};
use typst_pack::{Sources, fs};

use crate::cli::{AutomationArgs, Cli, Command, CompilationArgs, FontArgs, PackageArgs};

/// Why a command failed.
pub enum Failure {
    /// The failure was already printed.
    Reported,
    /// A message, and hints, still to print.
    Error { message: String, hints: Vec<String> },
}

impl<T: ToString> From<T> for Failure {
    fn from(message: T) -> Self {
        Self::Error {
            message: message.to_string(),
            hints: Vec::new(),
        }
    }
}

type CommandResult = Result<(), Failure>;

/// Terminal output for diagnostics and messages.
#[derive(Clone, Copy)]
pub struct Terminal {
    color: ColorChoice,
    format: DiagnosticFormat,
}

impl Terminal {
    pub fn error(self, message: &str) {
        self.label("error", Color::Red, message);
    }

    /// Prints `label: message` with the label styled in `color`.
    pub fn label(self, label: &str, color: Color, message: &str) {
        let mut stream = StandardStream::stderr(self.color);
        let _ = stream.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true));
        let _ = write!(stream, "{label}");
        let _ = stream.reset();
        let _ = writeln!(stream, ": {message}");
    }

    /// Prints Typst diagnostics with their source context.
    pub fn diagnostics<'a>(
        self,
        sources: &Sources,
        diagnostics: impl IntoIterator<Item = &'a SourceDiagnostic>,
    ) {
        let mut stream = StandardStream::stderr(self.color);
        let _ = typst_kit::diagnostics::emit(&mut stream, sources, diagnostics, self.format);
    }
}

/// Runs the CLI and returns the process exit code.
pub fn run() -> ExitCode {
    let Cli {
        color,
        cert,
        command,
    } = Cli::parse();
    let color = match color.as_str() {
        "always" => ColorChoice::Always,
        "never" => ColorChoice::Never,
        _ if std::io::stderr().is_terminal() => ColorChoice::Auto,
        _ => ColorChoice::Never,
    };
    let format = match &command {
        Command::Create(args) => args.automation.diagnostic_format,
        Command::Compile(args) => args.automation.diagnostic_format,
        Command::Inspect(_) | Command::Extract(_) => DiagnosticFormat::default(),
    };
    let terminal = Terminal { color, format };
    let result = match command {
        Command::Create(args) => create::run(args, terminal, cert),
        Command::Inspect(args) => inspect::run(args),
        Command::Extract(args) => extract::run(args),
        Command::Compile(args) => compile::run(args, terminal, cert),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Reported) => ExitCode::FAILURE,
        Err(Failure::Error { message, hints }) => {
            terminal.error(&message);
            for hint in hints {
                terminal.label("hint", Color::Cyan, &hint);
            }
            ExitCode::FAILURE
        }
    }
}

/// Validates the creation timestamp as Typst's CLI does, before any I/O, and
/// sizes the thread pool.
fn prepare(automation: &AutomationArgs) -> CommandResult {
    if let Some(seconds) = automation.creation_timestamp
        && chrono::DateTime::from_timestamp(seconds, 0).is_none()
    {
        return Err("creation timestamp is out of range".into());
    }
    if let Some(jobs) = automation.jobs {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .use_current_thread();
        pool.build_global().ok();
    }
    Ok(())
}

/// The current UNIX time and local UTC offset in minutes.
fn now() -> (i64, i32) {
    let now = chrono::Local::now();
    (now.timestamp(), now.offset().local_minus_utc() / 60)
}

/// Compile options from the shared arguments, and the input files read.
fn compile_options(
    args: &CompilationArgs,
    document_time: DocumentTime,
) -> Result<(Options, Vec<PathBuf>), Failure> {
    let strings = args.inputs.iter();
    let mut inputs = strings
        .map(|(key, value)| (key.clone(), Input::from(value.as_str())))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    for (key, path) in &args.input_files {
        let data = std::fs::read(path)
            .map_err(|error| format!("cannot read input file `{}`: {error}", path.display()))?;
        inputs.push((key.clone(), Input::Bytes(data)));
        files.push(path.clone());
    }
    let features = args.features.clone();
    Ok((
        Options {
            inputs,
            document_time,
            features,
        },
        files,
    ))
}

/// The host fonts and packages the arguments select.
fn packer(fonts: &FontArgs, packages: &PackageArgs, cert: Option<PathBuf>) -> fs::Packer {
    fs::Packer {
        font_paths: fonts.font_paths.clone(),
        system_fonts: !fonts.ignore_system_fonts,
        typst_embedded_fonts: !fonts.ignore_embedded_fonts,
        package_path: packages.package_path.clone(),
        package_cache_path: packages.package_cache_path.clone(),
        download_packages: !packages.offline,
        certificate: cert,
        ..fs::Packer::default()
    }
}

/// Performance timing of one command, as Typst's `--timings` records it.
struct Timings(Option<PathBuf>);

impl Timings {
    fn start(path: Option<PathBuf>) -> Self {
        if path.is_some() {
            typst_timing::enable();
            typst_timing::clear();
        }
        Self(path)
    }

    /// Writes the recorded events, resolving spans against `sources`.
    fn finish(self, sources: Option<&Sources>) -> Option<String> {
        let path = self.0?;
        let path = PathBuf::from(path.to_string_lossy().replace("{n}", "0"));
        let file = match File::create(&path) {
            Ok(file) => file,
            Err(error) => return Some(format!("failed to create file: {error}")),
        };
        let writer = BufWriter::with_capacity(1 << 20, file);
        typst_timing::export_json(writer, |span| {
            sources
                .and_then(|sources| locate(sources, Span::from_raw(span)))
                .unwrap_or_else(|| ("unknown".to_owned(), 0))
        })
        .err()
    }
}

/// The file and one-based line of a span.
fn locate(sources: &Sources, span: Span) -> Option<(String, u32)> {
    let id = span.id()?;
    let start = sources.range(span)?.start;
    let line = match typst::World::source(sources, id) {
        Ok(source) => source.lines().byte_to_line(start)?,
        Err(_) => typst::World::file(sources, id)
            .ok()?
            .lines()
            .ok()?
            .byte_to_line(start)?,
    };
    Some((format!("{id:?}"), line as u32 + 1))
}
