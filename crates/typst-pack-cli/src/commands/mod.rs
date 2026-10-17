//! Running commands, and what they share: error reporting, diagnostics,
//! timings, and the filesystem environment.

mod compile;
mod create;
mod extract;
mod inspect;

use std::fs::File;
use std::io::{BufWriter, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use typst::WorldExt;
use typst::diag::SourceDiagnostic;
use typst::syntax::Span;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::{
    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
};
use typst_pack::archive::{self, DecodeLimits};
use typst_pack::compile::{CreationTimestamp, DocumentTime, Input, Options};
use typst_pack::{Pack, Sources, fs};

use crate::cli::{AutomationArgs, Cli, Command, CompilationArgs, FontArgs, PackageArgs};

/// Why a command failed.
pub enum Failure {
    /// The failure was already printed.
    Reported,
    /// A message, and hints, still to print.
    Error { message: String, hints: Vec<String> },
}

impl<T: Into<Box<dyn std::error::Error>>> From<T> for Failure {
    /// A message, or an error and its sources.
    fn from(error: T) -> Self {
        Self::Error {
            message: chain(&*error.into()),
            hints: Vec::new(),
        }
    }
}

/// An error followed by each of its sources, `error: source: ...`. Library
/// errors expose their causes as sources rather than in their own messages.
pub fn chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
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
        Command::Inspect(args) => inspect::run(args, terminal),
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

/// Reads the pack at `path`, or from stdin for `-`.
fn read_pack(path: &Path) -> Result<Pack, Failure> {
    if path == Path::new("-") {
        archive::read(std::io::stdin().lock(), DecodeLimits::default()).map_err(|error| {
            // Name the step that failed, as `fs::read_archive` does for files.
            let action = match error {
                archive::DecodeError::Io(_) => "read",
                _ => "decode",
            };
            format!("cannot {action} pack from stdin: {}", chain(&error)).into()
        })
    } else {
        // The error names the path.
        Ok(fs::read_archive(path, DecodeLimits::default())?)
    }
}

/// Warns when a pack records a Typst version other than the embedded one: the
/// same pack may then render differently than where it was created.
fn warn_typst_version(pack: &Pack, terminal: Terminal) {
    if let Some(version) = pack.typst_version()
        && version != typst_pack::TYPST_VERSION
    {
        let message = format!(
            "the pack was created with Typst {}, but this typst-pack embeds Typst {}; \
             the output may differ",
            escape(version),
            typst_pack::TYPST_VERSION
        );
        terminal.label("warning", Color::Yellow, &message);
    }
}

/// Warns about each font file or directory that could not be read.
fn warn_skipped_fonts(skipped: &[fs::Error], terminal: Terminal) {
    for skipped in skipped {
        let message = format!("skipped font: {}", chain(skipped));
        terminal.label("warning", Color::Yellow, &message);
    }
}

/// The only issue, or a summary followed by one issue per line.
fn listed(summary: &str, issues: impl IntoIterator<Item = String>) -> String {
    let mut issues = issues.into_iter().collect::<Vec<_>>();
    match issues.len() {
        1 => issues.remove(0),
        _ => format!("{summary}:\n  {}", issues.join("\n  ")),
    }
}

/// `text` with control characters escaped, for printing untrusted names.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
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

/// What `datetime.today()` returns and the PDF creation date, as in Typst's
/// CLI: the creation timestamp read in UTC, or the current time read in the
/// local timezone.
fn time(automation: &AutomationArgs) -> (DocumentTime, CreationTimestamp) {
    match automation.creation_timestamp {
        Some(seconds) => (
            DocumentTime::UnixTimestamp(seconds),
            CreationTimestamp::Utc(seconds),
        ),
        None => {
            let now = chrono::Local::now();
            let (seconds, utc_offset_seconds) = (now.timestamp(), now.offset().local_minus_utc());
            (
                DocumentTime::Local {
                    seconds,
                    utc_offset_seconds,
                },
                CreationTimestamp::Local {
                    seconds,
                    utc_offset_seconds,
                },
            )
        }
    }
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
        // Which flag would win is not obvious from the command line.
        if args.inputs.iter().any(|(string, _)| string == key) {
            return Err(format!("input `{key}` is given by both --input and --input-file").into());
        }
        let data = std::fs::read(path)
            .map_err(|error| format!("cannot read input file `{}`: {error}", path.display()))?;
        inputs.push((key.clone(), Input::from(data)));
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
