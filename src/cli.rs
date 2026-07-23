//! The `typst-pack` command line interface.

#![cfg(feature = "cli")]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Timelike};
use clap::{Args, Parser, Subcommand, ValueEnum};
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue};
use typst::syntax::{FileId, VirtualRoot};
use typst_kit::diagnostics::termcolor::{
    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
};
use typst_kit::diagnostics::{DiagnosticFormat, DiagnosticWorld};
use typst_kit::files::{FileLoader, FsRoot};
use typst_kit::fonts::FontSource;
use typst_pdf::{PdfStandard, Timestamp};

use crate::compile::{
    CompilationArtifact, CompilationAttempt, CompilationExecutionControls,
    CompilationOperationOutcome, CompilationStatus, CompileOptions, CreationTimestamp,
    FontContainerFulfillment, OutputFormat, PackCompilationPresentation, PackCompilationRequest,
    PackOverrideSet, PackageTreeFulfillment, PageRange, PageSelection, compile_pack_kernel,
    parse_page_selection, pdf_standard_requiring_tags, prepare_pack_compilation,
    validate_pdf_standards,
};
use crate::extract::{ExtractOptions, extract};
use crate::manifest::PackMetadata;
use crate::pack::{FILE_EXTENSION, FontContainerIdentity, Pack};
use crate::packer::{DiscoveryTarget, DiscoveryWorld, Packer, PackerError};
use crate::world::PackWorld;

const ENV_PATH_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

enum CliError {
    Reported,
    Message(String),
    Hinted { message: String, hints: Vec<String> },
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}

type CliResult = Result<(), CliError>;

/// Pack, inspect, extract, and compile portable Typst project packs.
#[derive(Debug, Parser)]
#[command(
    name = "typst-pack",
    version = concat!(env!("CARGO_PKG_VERSION"), " (Typst 0.15.0)"),
    about
)]
pub struct Cli {
    /// Whether to use color. When set to `auto` if the terminal to supports it.
    #[arg(
        long,
        default_value = "auto",
        default_missing_value = "always",
        num_args = 0..=1,
        value_parser = ["auto", "always", "never"]
    )]
    color: String,

    /// Path to a custom CA certificate to use when making network requests.
    #[arg(long, env = "TYPST_CERT", value_name = "PATH")]
    cert: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct SharedCompilationArgs {
    /// Add a string key-value pair visible through `sys.inputs`.
    #[arg(
        long = "input",
        value_name = "key=value",
        value_parser = parse_input
    )]
    inputs: Vec<(String, String)>,

    /// Enables in-development features that may be changed or removed at any
    /// time.
    #[arg(
        long = "features",
        value_name = "FEATURES",
        value_delimiter = ',',
        env = "TYPST_FEATURES"
    )]
    features: Vec<FeatureArg>,
}

#[derive(Debug, Args)]
struct SharedFontArgs {
    /// Adds additional directories that are recursively searched for fonts.
    ///
    /// If multiple paths are specified, they are separated by the system's path
    /// separator (`:` on Unix-like systems and `;` on Windows).
    #[arg(
        long = "font-path",
        env = "TYPST_FONT_PATHS",
        value_name = "DIR",
        value_delimiter = ENV_PATH_SEPARATOR
    )]
    font_paths: Vec<PathBuf>,

    /// Ensures system fonts won't be searched, unless explicitly included via
    /// `--font-path`.
    #[arg(long, env = "TYPST_IGNORE_SYSTEM_FONTS")]
    ignore_system_fonts: bool,

    /// Ensures fonts embedded into Typst won't be considered.
    #[arg(long, env = "TYPST_IGNORE_EMBEDDED_FONTS")]
    ignore_embedded_fonts: bool,
}

#[derive(Debug, Args)]
struct SharedPackageArgs {
    /// Custom path to local packages, defaults to system-dependent location.
    #[arg(long, env = "TYPST_PACKAGE_PATH", value_name = "DIR")]
    package_path: Option<PathBuf>,

    /// Custom path to package cache, defaults to system-dependent location.
    #[arg(long, env = "TYPST_PACKAGE_CACHE_PATH", value_name = "DIR")]
    package_cache_path: Option<PathBuf>,

    /// Disallow network access; package dependencies must already be available
    /// in the local package directories.
    #[arg(long)]
    offline: bool,
}

#[derive(Debug, Args)]
struct SharedAutomationArgs {
    /// Number of parallel jobs spawned during compilation. Defaults to number
    /// of CPUs. Setting it to 1 disables parallelism.
    #[arg(long, short)]
    jobs: Option<usize>,

    /// The document's creation date formatted as a UNIX timestamp.
    ///
    /// For more information, see <https://reproducible-builds.org/specs/source-date-epoch/>.
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "UNIX_TIMESTAMP")]
    creation_timestamp: Option<i64>,

    /// The format to emit diagnostics in.
    #[arg(long, default_value = "human")]
    diagnostic_format: DiagnosticFormatArg,

    /// Produces performance timings of the compilation process. (experimental)
    ///
    /// The resulting JSON file can be loaded into a tracing tool such as
    /// https://ui.perfetto.dev. It does not contain any sensitive information
    /// apart from file names and line numbers.
    #[arg(long, value_name = "OUTPUT_JSON")]
    timings: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Packs a Typst project into a single portable file.
    Create(CreateArgs),
    /// Shows what is inside a pack.
    Inspect(InspectArgs),
    /// Extracts a pack into a directory.
    Extract(ExtractArgs),
    /// Compiles a pack to PDF, PNG, SVG, or HTML.
    #[command(visible_alias = "c")]
    Compile(CompileArgs),
}

#[derive(Debug, Args)]
struct CreateArgs {
    /// Path to the input Typst file.
    #[arg(help_heading = "Project")]
    input: PathBuf,

    /// Configures the project root (for absolute paths).
    #[arg(long, env = "TYPST_ROOT", value_name = "DIR", help_heading = "Project")]
    root: Option<PathBuf>,

    /// Compilation targets whose dependencies should be discovered.
    #[arg(
        long = "target",
        value_name = "TARGET",
        value_delimiter = ',',
        help_heading = "Discovery"
    )]
    targets: Vec<DiscoveryTargetArg>,

    #[command(flatten, next_help_heading = "Discovery")]
    compilation: SharedCompilationArgs,

    /// Path to the output Pack [default: INPUT with its extension replaced by .typk].
    #[arg(help_heading = "Pack Contents")]
    output: Option<PathBuf>,

    /// Embed the fonts used by the document into the pack.
    #[arg(long, help_heading = "Pack Contents")]
    embed_fonts: bool,

    /// When embedding fonts, also embed fonts identical to Typst's embedded
    /// fonts.
    #[arg(long, requires = "embed_fonts", help_heading = "Pack Contents")]
    include_typst_embedded_fonts: bool,

    /// Additional files or directories (inside the project root) to pack
    /// beyond what the discovery compile finds.
    #[arg(long = "include", value_name = "PATH", help_heading = "Pack Contents")]
    include: Vec<PathBuf>,

    /// Project-shaped directories that provide Resource Slot bytes during discovery.
    #[arg(
        long = "resource-path",
        value_name = "DIR",
        help_heading = "Resource Slots"
    )]
    resource_paths: Vec<PathBuf>,

    /// Root-relative Resource Slot paths to declare even if discovery does not request them.
    #[arg(
        long = "resource-slot",
        value_name = "PATH",
        help_heading = "Resource Slots"
    )]
    resource_slots: Vec<String>,

    #[command(flatten, next_help_heading = "Fonts")]
    fonts: SharedFontArgs,

    /// Do not store package files in the pack; record them as unvendored
    /// dependencies instead.
    #[arg(long = "no-vendor-packages", help_heading = "Packages")]
    no_vendor_packages: bool,

    #[command(flatten, next_help_heading = "Packages")]
    packages: SharedPackageArgs,

    /// A human-readable name recorded in the pack metadata.
    #[arg(long, help_heading = "Metadata")]
    name: Option<String>,

    /// A description recorded in the pack metadata.
    #[arg(long, help_heading = "Metadata")]
    description: Option<String>,

    /// Authors recorded in the pack metadata.
    #[arg(long = "author", value_name = "AUTHOR", help_heading = "Metadata")]
    authors: Vec<String>,

    #[command(flatten, next_help_heading = "Diagnostics & Automation")]
    automation: SharedAutomationArgs,
}

#[derive(Debug, Args)]
struct InspectArgs {
    /// The pack file to inspect.
    pack: PathBuf,
}

#[derive(Debug, Args)]
struct ExtractArgs {
    /// The pack file to extract.
    pack: PathBuf,

    /// The directory to extract into [default: <pack name>/].
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Also extract vendored packages to packages/.
    #[arg(long)]
    packages: bool,

    /// Also extract embedded fonts to fonts/.
    #[arg(long)]
    fonts: bool,

    /// Extract everything (same as --packages --fonts).
    #[arg(long)]
    all: bool,

    /// Overwrite existing files.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct CompileArgs {
    /// The pack file to compile.
    #[arg(help_heading = "Compilation")]
    pack: PathBuf,

    /// Path to output file (PDF, PNG, SVG, or HTML). Use `-` to write output to
    /// stdout.
    ///
    /// For output formats emitting one file per page (PNG & SVG), a page number
    /// template must be present if the source document renders to multiple
    /// pages. Use `{p}` for page numbers, `{0p}` for zero padded page numbers
    /// and `{t}` for page count. For example, `page-{0p}-of-{t}.png` creates
    /// `page-01-of-10.png`, `page-02-of-10.png`, and so on.
    #[arg(help_heading = "Output")]
    output: Option<PathBuf>,

    /// The format of the output file, inferred from the extension by default.
    #[arg(short, long, help_heading = "Output")]
    format: Option<OutputFormatArg>,

    /// Whether to pretty-print produced output.
    ///
    /// This formats the output in a more human-readable, but less
    /// space-efficient way. Affects HTML, SVG, and PDF export, but not PNG
    /// export.
    #[arg(long, help_heading = "Output")]
    pretty: bool,

    #[command(flatten, next_help_heading = "Compilation")]
    compilation: SharedCompilationArgs,

    /// Replaces one contained project file for this compilation.
    #[arg(
        long = "override",
        value_names = ["PACK_PATH", "FILE"],
        num_args = 2,
        help_heading = "Compilation"
    )]
    overrides: Vec<OsString>,

    /// Which pages to export. When unspecified, all pages are exported.
    ///
    /// Pages to export are separated by commas, and can be either simple page
    /// numbers (e.g. '2,5' to export only pages 2 and 5) or page ranges (e.g.
    /// '2,3-6,8-' to export page 2, pages 3 to 6 (inclusive), page 8 and any
    /// pages after it).
    ///
    /// Page numbers are one-indexed and correspond to physical page numbers in
    /// the document (therefore not being affected by the document's page
    /// counter).
    #[arg(long, value_delimiter = ',', help_heading = "Output")]
    pages: Vec<PageRangeArg>,

    /// The PPI (pixels per inch) to use for PNG export.
    #[arg(long, default_value_t = 144.0, help_heading = "Output")]
    ppi: f64,

    /// One (or multiple comma-separated) PDF standards that Typst will enforce
    /// conformance with.
    #[arg(
        long = "pdf-standard",
        value_name = "PDF_STANDARD",
        value_delimiter = ',',
        help_heading = "PDF"
    )]
    pdf_standards: Vec<PdfStandardArg>,

    /// By default, even when not producing a `PDF/UA-1` document, a tagged PDF
    /// document is written to provide a baseline of accessibility. In some
    /// circumstances (for example when trying to reduce the size of a document)
    /// it can be desirable to disable tagged PDF.
    #[arg(long = "no-pdf-tags", help_heading = "PDF")]
    no_pdf_tags: bool,

    /// Project-shaped directories that provide Resource Slot bytes during compilation.
    #[arg(
        long = "resource-path",
        value_name = "DIR",
        help_heading = "Resource Slots"
    )]
    resource_paths: Vec<PathBuf>,

    #[command(flatten, next_help_heading = "Fonts")]
    fonts: SharedFontArgs,

    #[command(flatten, next_help_heading = "Packages")]
    packages: SharedPackageArgs,

    #[command(flatten, next_help_heading = "Diagnostics & Automation")]
    automation: SharedAutomationArgs,

    /// File path to which a list of current compilation's dependencies will be
    /// written. Use `-` to write to stdout.
    #[arg(long, value_name = "PATH", help_heading = "Diagnostics & Automation")]
    deps: Option<PathBuf>,

    /// File format to use for dependencies.
    #[arg(
        long,
        default_value = "json",
        value_enum,
        help_heading = "Diagnostics & Automation"
    )]
    deps_format: DepsFormat,

    /// Opens the output file with the default viewer or a specific program
    /// after compilation. Ignored if output is stdout.
    #[arg(
        long,
        value_name = "VIEWER",
        num_args = 0..=1,
        help_heading = "Diagnostics & Automation"
    )]
    open: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DepsFormat {
    /// Encodes as JSON, failing for non-Unicode paths.
    Json,
    /// Separates paths with NULL bytes and can express all paths.
    Zero,
    /// Emits in Make format, omitting inexpressible paths.
    Make,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DiagnosticFormatArg {
    Human,
    Short,
}

impl From<DiagnosticFormatArg> for DiagnosticFormat {
    fn from(value: DiagnosticFormatArg) -> Self {
        match value {
            DiagnosticFormatArg::Human => Self::Human,
            DiagnosticFormatArg::Short => Self::Short,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FeatureArg {
    Html,
    A11yExtras,
}

impl From<FeatureArg> for typst::Feature {
    fn from(value: FeatureArg) -> Self {
        match value {
            FeatureArg::Html => Self::Html,
            FeatureArg::A11yExtras => Self::A11yExtras,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DiscoveryTargetArg {
    Paged,
    Html,
}

impl From<DiscoveryTargetArg> for DiscoveryTarget {
    fn from(value: DiscoveryTargetArg) -> Self {
        match value {
            DiscoveryTargetArg::Paged => Self::Paged,
            DiscoveryTargetArg::Html => Self::Html,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Pdf,
    Png,
    Svg,
    Html,
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(value: OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Pdf => Self::Pdf,
            OutputFormatArg::Png => Self::Png,
            OutputFormatArg::Svg => Self::Svg,
            OutputFormatArg::Html => Self::Html,
        }
    }
}

#[derive(Debug, Clone)]
struct PageRangeArg(PageRange);

impl std::str::FromStr for PageRangeArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let selection = parse_page_selection(value)?;
        Ok(Self(
            selection
                .ranges()
                .first()
                .expect("one range is parsed from one CLI value")
                .clone(),
        ))
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, ValueEnum)]
enum PdfStandardArg {
    /// PDF 1.4.
    #[value(name = "1.4")]
    V_1_4,
    /// PDF 1.5.
    #[value(name = "1.5")]
    V_1_5,
    /// PDF 1.6.
    #[value(name = "1.6")]
    V_1_6,
    /// PDF 1.7.
    #[value(name = "1.7")]
    V_1_7,
    /// PDF 2.0.
    #[value(name = "2.0")]
    V_2_0,
    /// PDF/A-1b.
    #[value(name = "a-1b")]
    A_1b,
    /// PDF/A-1a.
    #[value(name = "a-1a")]
    A_1a,
    /// PDF/A-2b.
    #[value(name = "a-2b")]
    A_2b,
    /// PDF/A-2u.
    #[value(name = "a-2u")]
    A_2u,
    /// PDF/A-2a.
    #[value(name = "a-2a")]
    A_2a,
    /// PDF/A-3b.
    #[value(name = "a-3b")]
    A_3b,
    /// PDF/A-3u.
    #[value(name = "a-3u")]
    A_3u,
    /// PDF/A-3a.
    #[value(name = "a-3a")]
    A_3a,
    /// PDF/A-4.
    #[value(name = "a-4")]
    A_4,
    /// PDF/A-4f.
    #[value(name = "a-4f")]
    A_4f,
    /// PDF/A-4e.
    #[value(name = "a-4e")]
    A_4e,
    /// PDF/UA-1.
    #[value(name = "ua-1")]
    Ua_1,
}

impl From<PdfStandardArg> for PdfStandard {
    fn from(value: PdfStandardArg) -> Self {
        match value {
            PdfStandardArg::V_1_4 => Self::V_1_4,
            PdfStandardArg::V_1_5 => Self::V_1_5,
            PdfStandardArg::V_1_6 => Self::V_1_6,
            PdfStandardArg::V_1_7 => Self::V_1_7,
            PdfStandardArg::V_2_0 => Self::V_2_0,
            PdfStandardArg::A_1b => Self::A_1b,
            PdfStandardArg::A_1a => Self::A_1a,
            PdfStandardArg::A_2b => Self::A_2b,
            PdfStandardArg::A_2u => Self::A_2u,
            PdfStandardArg::A_2a => Self::A_2a,
            PdfStandardArg::A_3b => Self::A_3b,
            PdfStandardArg::A_3u => Self::A_3u,
            PdfStandardArg::A_3a => Self::A_3a,
            PdfStandardArg::A_4 => Self::A_4,
            PdfStandardArg::A_4f => Self::A_4f,
            PdfStandardArg::A_4e => Self::A_4e,
            PdfStandardArg::Ua_1 => Self::Ua_1,
        }
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
    let result = match command {
        Command::Create(args) => create(args, color, cert.as_deref()),
        Command::Inspect(args) => inspect(args),
        Command::Extract(args) => extract_command(args),
        Command::Compile(args) => compile_command(args, color, cert.as_deref()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Reported) => ExitCode::FAILURE,
        Err(CliError::Message(error)) => {
            emit_owned_error(&error, color);
            ExitCode::FAILURE
        }
        Err(CliError::Hinted { message, hints }) => {
            emit_owned_error(&message, color);
            for hint in hints {
                emit_owned_hint(&hint, color);
            }
            ExitCode::FAILURE
        }
    }
}

fn create(args: CreateArgs, color: ColorChoice, cert: Option<&Path>) -> CliResult {
    validate_creation_timestamp(args.automation.creation_timestamp)?;
    initialize_jobs(args.automation.jobs);
    let diagnostic_format = args.automation.diagnostic_format.into();
    if args.input == Path::new("-") {
        return Err("create input must be a named Typst source file, not stdin".into());
    }

    let input = args
        .input
        .canonicalize()
        .map_err(|err| format!("cannot access `{}`: {err}", args.input.display()))?;
    if !input.is_file() {
        return Err(format!(
            "create input must be a Typst source file: `{}`",
            args.input.display()
        )
        .into());
    }

    let root = match args.root {
        Some(root) => root,
        None => input
            .parent()
            .ok_or("cannot determine project root")?
            .to_path_buf(),
    };
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension(FILE_EXTENSION));

    let mut packer = Packer::new(&root, &input)
        .vendor_packages(!args.no_vendor_packages)
        .embed_fonts(args.embed_fonts)
        .include_typst_embedded_fonts(args.include_typst_embedded_fonts)
        .typst_embedded_fonts(!args.fonts.ignore_embedded_fonts)
        .system_fonts(!args.fonts.ignore_system_fonts)
        .offline(args.packages.offline)
        .certificate(cert.map(Path::to_path_buf))
        .creation_timestamp(args.automation.creation_timestamp)
        .timings(args.automation.timings.clone())
        .inputs(parse_inputs(&args.compilation.inputs));
    for path in &args.include {
        packer = packer.include(path);
    }
    for path in &args.resource_paths {
        packer = packer.resource_provider(FilesystemResourceProvider::new(path.clone()));
    }
    for path in args.resource_slots {
        packer = packer.resource_slot(path);
    }
    for target in args.targets {
        packer = packer.target(target.into());
    }
    for feature in args.compilation.features {
        packer = packer.feature(feature.into());
    }
    for path in &args.fonts.font_paths {
        packer = packer.font_path(path);
    }
    if let Some(path) = &args.packages.package_path {
        packer = packer.package_path(path);
    }
    if let Some(path) = &args.packages.package_cache_path {
        packer = packer.package_cache_path(path);
    }
    if args.name.is_some() || args.description.is_some() || !args.authors.is_empty() {
        let mut metadata = PackMetadata::new();
        if let Some(name) = args.name {
            metadata = metadata.with_name(name);
        }
        if let Some(description) = args.description {
            metadata = metadata.with_description(description);
        }
        for author in args.authors {
            metadata = metadata.with_author(author);
        }
        packer = packer.metadata(metadata);
    }

    let (outcome, timing_error) = packer.pack_with_timing();
    let timing_error = timing_error.map(|error| error.to_string());
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(PackerError::Compile {
            world,
            errors,
            warnings,
        }) => {
            emit_diagnostics_with(
                world.world(),
                errors.iter().chain(&warnings),
                diagnostic_format,
                color,
            );
            if let Some(error) = timing_error {
                return Err(error.into());
            }
            return Err(CliError::Reported);
        }
        Err(PackerError::ResourceSlotUnavailable { path }) => {
            let message = format!(
                "requested Resource Slot `{path}` is unavailable for discovery; place representative bytes at `{path}` in the source project or supply them via `--resource-path`; representative bytes are not stored in the Pack"
            );
            if let Some(error) = timing_error {
                emit_owned_error(&message, color);
                return Err(error.into());
            }
            return Err(message.into());
        }
        Err(err) => {
            if let Some(error) = timing_error {
                emit_owned_error(&err.to_string(), color);
                return Err(error.into());
            }
            return Err(err.to_string().into());
        }
    };

    emit_diagnostics_with(
        &outcome.world,
        outcome.report.compile_warnings.iter(),
        diagnostic_format,
        color,
    );
    for warning in &outcome.report.warnings {
        emit_owned_warning(warning, color);
    }
    if let Some(error) = timing_error {
        return Err(error.into());
    }

    if output == Path::new("-") {
        let bytes = outcome.pack.to_bytes().map_err(|err| err.to_string())?;
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|err| format!("cannot write Pack to stdout: {err}"))?;
        return Ok(());
    }

    let file = File::create(&output)
        .map_err(|err| format!("cannot create `{}`: {err}", output.display()))?;
    outcome
        .pack
        .write(std::io::BufWriter::new(file))
        .map_err(|err| err.to_string())?;

    let report = &outcome.report;
    println!(
        "packed {} project file(s), {} package(s), {} font(s) into `{}`",
        report.files.len(),
        report.packages_vendored.len(),
        report.fonts.len(),
        output.display(),
    );
    if !report.packages_unvendored.is_empty() {
        println!(
            "note: {} package(s) were not vendored and must be available when compiling:",
            report.packages_unvendored.len()
        );
        for spec in &report.packages_unvendored {
            println!("  {spec}");
        }
    }
    if !report.resource_slots.is_empty() {
        println!(
            "note: {} Resource Slot path(s) are declared and must be supplied if requested:",
            report.resource_slots.len()
        );
        for path in &report.resource_slots {
            println!("  {path}");
        }
    }
    Ok(())
}

fn inspect(args: InspectArgs) -> CliResult {
    let pack = read_pack(&args.pack)?;
    let manifest = pack.manifest();

    println!("pack: {}", args.pack.display());
    println!("format version: {}", manifest.format_version());
    println!("entrypoint: {}", pack.entrypoint());
    if let Some(metadata) = manifest.metadata() {
        if let Some(name) = metadata.name() {
            println!("name: {name}");
        }
        if let Some(description) = metadata.description() {
            println!("description: {description}");
        }
        if !metadata.authors().is_empty() {
            println!("authors: {}", metadata.authors().join(", "));
        }
    }

    println!("\npacked project files:");
    for (path, data) in pack.files() {
        println!("  {path} ({})", human_size(data.len()));
    }

    if manifest.project().resource_slots().next().is_some() {
        println!("\nResource Slots:");
        for path in manifest.project().resource_slots() {
            println!("  {path}");
        }
    }

    let vendored: Vec<_> = pack.packages().collect();
    if !vendored.is_empty() {
        println!("\nvendored packages:");
        for (spec, files) in vendored {
            let (count, size) = files.fold((0usize, 0usize), |(count, size), (_, data)| {
                (count + 1, size + data.len())
            });
            println!("  {spec} ({count} files, {})", human_size(size));
        }
    }
    if !manifest.packages().unvendored().is_empty() {
        println!("\nunvendored packages:");
        for spec in manifest.packages().unvendored() {
            println!("  {spec}");
        }
    }

    if !pack.fonts().is_empty() {
        println!("\nembedded fonts:");
        for font in pack.fonts() {
            println!(
                "  {} ({}){}",
                font.manifest().path(),
                human_size(font.data().len()),
                if font.manifest().families().is_empty() {
                    String::new()
                } else {
                    format!(" - {}", font.manifest().families().join(", "))
                }
            );
        }
    }

    Ok(())
}

fn extract_command(args: ExtractArgs) -> CliResult {
    let pack = read_pack(&args.pack)?;
    let output = args
        .output
        .unwrap_or_else(|| default_output_dir(&args.pack));

    let report = extract(
        &pack,
        &output,
        &ExtractOptions {
            packages: args.packages || args.all,
            fonts: args.fonts || args.all,
            force: args.force,
        },
    )
    .map_err(|err| err.to_string())?;

    println!(
        "extracted {} file(s) into `{}`",
        report.written.len(),
        output.display()
    );
    if !report.resource_slots.is_empty() {
        println!("\nResource Slots (not extracted):");
        for path in &report.resource_slots {
            println!("  {}", path.display());
        }
    }
    Ok(())
}

fn compile_command(args: CompileArgs, color: ColorChoice, cert: Option<&Path>) -> CliResult {
    if args.pack == Path::new("-") && args.output.is_none() {
        return Err("an explicit output is required when the Pack is read from stdin".into());
    }

    let format = match args.format {
        Some(format) => format.into(),
        None => match args.output.as_deref() {
            Some(path) if path != Path::new("-") => {
                match path.extension().and_then(|extension| extension.to_str()) {
                    Some(ext) if ext.eq_ignore_ascii_case("png") => OutputFormat::Png,
                    Some(ext) if ext.eq_ignore_ascii_case("svg") => OutputFormat::Svg,
                    Some(ext) if ext.eq_ignore_ascii_case("pdf") => OutputFormat::Pdf,
                    Some(ext) if ext.eq_ignore_ascii_case("html") => OutputFormat::Html,
                    Some(other) => {
                        return Err(format!(
                            "cannot infer output format from extension `{}`; pass --format",
                            other
                        )
                        .into());
                    }
                    None => {
                        return Err("cannot infer output format; pass --format".into());
                    }
                }
            }
            _ => OutputFormat::Pdf,
        },
    };

    let page_selection =
        PageSelection::new(args.pages.iter().map(|range| range.0.clone()).collect());
    let standards = args
        .pdf_standards
        .iter()
        .copied()
        .map(PdfStandard::from)
        .collect::<Vec<_>>();
    if args.no_pdf_tags || !page_selection.ranges().is_empty() {
        if let Some(name) = pdf_standard_requiring_tags(&standards) {
            let message = format!("cannot disable PDF tags when exporting a {name} document");
            return if args.no_pdf_tags {
                Err(message.into())
            } else {
                Err(CliError::Hinted {
                    message,
                    hints: vec!["using --pages implies --no-pdf-tags".to_owned()],
                })
            };
        }
    }
    validate_pdf_standards(&standards).map_err(|error| {
        let (message, hints) = error.into_parts();
        CliError::Hinted { message, hints }
    })?;

    if args.output.as_deref() == Some(Path::new("-"))
        && args.deps.as_deref() == Some(Path::new("-"))
    {
        return Err("cannot write both output and dependencies to stdout".into());
    }

    let creation_timestamp_seconds = args.automation.creation_timestamp;
    let creation_timestamp = validate_creation_timestamp(creation_timestamp_seconds)?;
    let system_time = creation_timestamp_seconds
        .is_none()
        .then(chrono::Local::now);
    initialize_jobs(args.automation.jobs);

    let pack = read_pack_input(&args.pack)?;
    let mut override_preflight = PackOverrideSet::new(&pack);
    for pair in args.overrides.chunks_exact(2) {
        let pack_path = pair[0]
            .to_str()
            .ok_or("Pack Override project path must be valid UTF-8")?;
        override_preflight = override_preflight
            .replace(pack_path, Vec::new())
            .map_err(|error| CliError::Message(error.to_string()))?;
    }
    let host_dependencies = Arc::new(Mutex::new(BTreeSet::new()));
    let mut overrides = PackOverrideSet::new(&pack);
    for pair in args.overrides.chunks_exact(2) {
        let pack_path = pair[0]
            .to_str()
            .expect("Pack Override paths were validated before filesystem access");
        let source = PathBuf::from(&pair[1]);
        let data = std::fs::read(&source).map_err(|error| {
            CliError::Message(format!(
                "failed to read Pack Override source `{}`: {error}",
                source.display()
            ))
        })?;
        overrides = overrides
            .replace(pack_path, data)
            .map_err(|error| CliError::Message(error.to_string()))?;
        host_dependencies
            .lock()
            .expect("host dependency lock poisoned")
            .insert(source);
    }

    let mut supplied_fonts = BTreeMap::<FontContainerIdentity, Bytes>::new();
    if pack
        .font_requirements()
        .iter()
        .any(|requirement| !requirement.is_embedded())
    {
        let mut load = |font: typst::text::Font| {
            supplied_fonts
                .entry(FontContainerIdentity::from_bytes(font.data().as_slice()))
                .or_insert_with(|| font.data().clone());
        };
        if !args.fonts.ignore_system_fonts {
            for (source, _) in typst_kit::fonts::system() {
                if let Some(font) = source.load() {
                    load(font);
                }
            }
        }
        if !args.fonts.ignore_embedded_fonts {
            for (font, _) in typst_kit::fonts::embedded() {
                load(font);
            }
        }
        for path in &args.fonts.font_paths {
            for (source, _) in typst_kit::fonts::scan(path) {
                if let Some(font) = source.load() {
                    load(font);
                }
            }
        }
    }
    let packages = crate::world::system_packages(
        args.packages.package_path.as_deref(),
        args.packages.package_cache_path.as_deref(),
        args.packages.offline,
        cert,
    );
    let mut package_roots = BTreeMap::new();
    let mut package_fulfillments = Vec::new();
    for requirement in pack
        .package_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
    {
        let root = packages.obtain(requirement.spec()).map_err(|error| {
            CliError::Message(
                CompilationOperationOutcome::UnavailableExternalPackageFulfillment {
                    spec: requirement.spec().clone(),
                    message: error.to_string(),
                }
                .to_string(),
            )
        })?;
        let files =
            crate::world::read_complete_package_tree(root.path()).map_err(CliError::Message)?;
        package_roots.insert(requirement.spec().to_string(), root.path().to_owned());
        package_fulfillments.push((requirement.spec().clone(), files));
    }
    let options = CompileOptions {
        page_selection,
        ppi: Some(args.ppi),
        render_bleed: false,
        pretty: args.pretty,
        pdf_standards: standards,
        pdf_identifier: typst::foundations::Smart::Auto,
        pdf_creator: typst::foundations::Smart::Auto,
        pdf_tags: if args.no_pdf_tags {
            typst::foundations::Smart::Custom(false)
        } else {
            typst::foundations::Smart::Auto
        },
        creation_timestamp: match creation_timestamp {
            Some(timestamp) => convert_datetime(timestamp)
                .map(Timestamp::new_utc)
                .map_or(CreationTimestamp::Omit, CreationTimestamp::Explicit),
            None => system_time
                .as_ref()
                .and_then(local_timestamp)
                .map(CreationTimestamp::Explicit)
                .unwrap_or(CreationTimestamp::Omit),
        },
    };

    let document_timestamp = creation_timestamp_seconds.unwrap_or_else(|| {
        system_time
            .as_ref()
            .expect("system time is frozen when no explicit timestamp is supplied")
            .with_timezone(&chrono::Utc)
            .timestamp()
    });
    let mut request = PackCompilationRequest::new(pack, format)
        .adapter_resolved_inputs(parse_inputs(&args.compilation.inputs))
        .adapter_resolved_options(options)
        .adapter_resolved_document_timestamp(document_timestamp)
        .map_err(|_| "creation timestamp is out of range")?;
    let mut controls = CompilationExecutionControls::default();
    for path in &args.resource_paths {
        controls = controls.resource_provider(FilesystemResourceProvider::tracked(
            path.clone(),
            Arc::clone(&host_dependencies),
        ));
    }
    if !args.overrides.is_empty() {
        request = request.overrides(overrides);
    }
    for feature in &args.compilation.features {
        request = request.adapter_resolved_feature((*feature).into());
    }
    for (identity, data) in supplied_fonts {
        request = request.font_fulfillment(identity, FontContainerFulfillment::new(data.to_vec()));
    }
    for (spec, files) in package_fulfillments {
        request = request.package_fulfillment(
            spec,
            PackageTreeFulfillment::new(
                files.into_iter().map(|(path, data)| (path, data.to_vec())),
            ),
        );
    }
    let prepared = prepare_pack_compilation(CompilationAttempt::new(request, controls))
        .map_err(|error| CliError::Message(error.to_string()))?;
    let (mut world, kernel) = prepared.into_parts();

    let diagnostic_format = args.automation.diagnostic_format.into();
    let write_requested_dependencies = |outputs: Option<&[PathBuf]>| {
        let Some(destination) = &args.deps else {
            return Ok(());
        };
        let mut inputs = host_dependencies
            .lock()
            .expect("host dependency lock poisoned")
            .clone();
        if args.pack != Path::new("-") {
            inputs.insert(args.pack.clone());
        }
        write_dependencies(destination, args.deps_format, &inputs, outputs)
    };

    let mut timer = typst_kit::timer::Timer::new_or_placeholder(args.automation.timings.clone());
    let mut command_result = None;
    let timings = timer.record(&mut world, |world| {
        command_result = Some((|| -> CliResult {
            let execution = compile_pack_kernel(world, kernel);
            for id in world.file_dependencies() {
                if let VirtualRoot::Package(spec) = id.root()
                    && let Some(root) = package_roots.get(&spec.to_string())
                {
                    host_dependencies
                        .lock()
                        .expect("host dependency lock poisoned")
                        .insert(root.join(id.vpath().get_without_slash()));
                }
            }
            match &execution.presentation {
                PackCompilationPresentation::Succeeded { .. } => {}
                PackCompilationPresentation::Diagnostics {
                    errors,
                    warnings,
                    pack_warnings,
                } => {
                    emit_diagnostics_with(
                        world,
                        warnings
                            .iter()
                            .chain(pack_warnings.iter())
                            .chain(errors.iter()),
                        diagnostic_format,
                        color,
                    );
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
                PackCompilationPresentation::PngExport {
                    error,
                    warnings,
                    pack_warnings,
                } => {
                    emit_owned_error(error, color);
                    emit_diagnostics_with(
                        world,
                        warnings.iter().chain(pack_warnings.iter()),
                        diagnostic_format,
                        color,
                    );
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
            }
            debug_assert_eq!(execution.result.status(), CompilationStatus::Succeeded);
            let output = &execution.result;

            let export_result = (|| {
                let default_output = args.pack.with_extension(format.extension());
                let targets: Vec<PathBuf> = match &args.output {
                    Some(path) if path == Path::new("-") => vec![path.clone()],
                    Some(path) if matches!(format, OutputFormat::Pdf | OutputFormat::Html) => {
                        vec![path.clone()]
                    }
                    Some(path) => expand_output_template(
                        path,
                        output.artifacts(),
                        output
                            .source_page_count()
                            .unwrap_or(output.artifacts().len()),
                    )?,
                    None if matches!(format, OutputFormat::Pdf | OutputFormat::Html) => {
                        vec![default_output]
                    }
                    None => expand_output_template(
                        &default_output,
                        output.artifacts(),
                        output
                            .source_page_count()
                            .unwrap_or(output.artifacts().len()),
                    )?,
                };
                let mut unique_targets = std::collections::HashSet::with_capacity(targets.len());
                for target in &targets {
                    if !unique_targets.insert(normalize_output_path(target)) {
                        return Err(format!(
                            "multiple artifacts expand to the same output path `{}`",
                            target.display()
                        ));
                    }
                }

                let output_is_stdout = targets.iter().any(|target| target == Path::new("-"));
                if output_is_stdout {
                    if output.artifacts().len() != 1 {
                        return Err(
                            "cannot write output to stdout unless exactly one file is emitted"
                                .to_owned(),
                        );
                    }
                    std::io::stdout()
                        .lock()
                        .write_all(output.artifacts()[0].bytes())
                        .map_err(|err| format!("cannot write output to stdout: {err}"))?;
                } else {
                    for (target, artifact) in targets.iter().zip(output.artifacts()) {
                        std::fs::write(target, artifact.bytes())
                            .map_err(|err| format!("cannot write `{}`: {err}", target.display()))?;
                    }
                }
                Ok::<_, String>((targets, output_is_stdout))
            })();
            let (targets, output_is_stdout) = match export_result {
                Ok(exported) => exported,
                Err(error) => {
                    emit_owned_error(&error, color);
                    if let PackCompilationPresentation::Succeeded {
                        warnings,
                        pack_warnings,
                    } = &execution.presentation
                    {
                        emit_diagnostics_with(
                            world,
                            warnings.iter().chain(pack_warnings),
                            diagnostic_format,
                            color,
                        );
                    }
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
            };

            if let PackCompilationPresentation::Succeeded {
                warnings,
                pack_warnings,
            } = &execution.presentation
            {
                emit_diagnostics_with(
                    world,
                    warnings.iter().chain(pack_warnings),
                    diagnostic_format,
                    color,
                );
            }

            if !output_is_stdout
                && let Some(viewer) = args.open.as_ref()
                && let Some(first) = targets.first()
            {
                let first = first
                    .canonicalize()
                    .map_err(|err| format!("failed to canonicalize path ({err})"))?;
                match viewer.as_deref() {
                    Some(viewer) => open::with_detached(&first, viewer),
                    None => open::that_detached(&first),
                }
                .map_err(|err| err.to_string())?;
            }

            write_requested_dependencies(Some(&targets))?;

            Ok(())
        })());
    });
    let Some(command_result) = command_result else {
        return Err(timings
            .expect_err("timer did not execute compilation")
            .to_string()
            .into());
    };
    let timing_error = timings.err().map(|error| error.to_string());
    if let Some(error) = timing_error {
        emit_owned_error(&error, color);
        return Err(CliError::Reported);
    }
    command_result
}

/// Expands Typst page templates into one path per Page Format artifact.
fn expand_output_template(
    template: &Path,
    artifacts: &[CompilationArtifact],
    total_source_pages: usize,
) -> Result<Vec<PathBuf>, String> {
    if artifacts.is_empty() {
        return Ok(Vec::new());
    }
    let Some(text) = template.to_str() else {
        return if artifacts.len() > 1 {
            Err(missing_page_template_error())
        } else {
            Ok(vec![template.to_path_buf()])
        };
    };
    let has_page_placeholder = has_indexable_page_template(text);
    let count = artifacts.len();
    if count > 1 && !has_page_placeholder {
        return Err(missing_page_template_error());
    }
    if !has_page_placeholder {
        return Ok(vec![template.to_path_buf()]);
    }
    let width = total_source_pages.to_string().len();
    Ok(artifacts
        .iter()
        .enumerate()
        .map(|(index, artifact)| {
            let page = artifact
                .source_page_number()
                .map_or(index + 1, |number| number.get());
            PathBuf::from(
                text.replace("{p}", &page.to_string())
                    .replace("{0p}", &format!("{page:0width$}"))
                    .replace("{n}", &format!("{page:0width$}"))
                    .replace("{t}", &total_source_pages.to_string()),
            )
        })
        .collect())
}

fn has_indexable_page_template(output: &str) -> bool {
    output.contains("{p}") || output.contains("{0p}") || output.contains("{n}")
}

fn missing_page_template_error() -> String {
    "cannot export multiple images without a page number template ({p}, {0p}) in the output path"
        .to_owned()
}

fn normalize_output_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn read_pack(path: &Path) -> Result<Pack, String> {
    let file =
        File::open(path).map_err(|err| format!("cannot open `{}`: {err}", path.display()))?;
    Pack::read(BufReader::new(file)).map_err(|err| err.to_string())
}

fn read_pack_input(path: &Path) -> Result<Pack, String> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::stdin()
            .lock()
            .read_to_end(&mut bytes)
            .map_err(|err| format!("cannot read Pack from stdin: {err}"))?;
        return Pack::from_bytes(bytes).map_err(|err| err.to_string());
    }
    read_pack(path)
}

fn default_output_dir(pack: &Path) -> PathBuf {
    match pack.file_stem() {
        Some(stem) => PathBuf::from(stem),
        None => PathBuf::from("extracted"),
    }
}

fn parse_input(pair: &str) -> Result<(String, String), String> {
    let (key, value) = pair
        .split_once('=')
        .ok_or_else(|| "input must be a key and a value separated by an equal sign".to_owned())?;
    let key = key.trim();
    if key.is_empty() {
        return Err("the key was missing or empty".to_owned());
    }
    Ok((key.to_owned(), value.trim().to_owned()))
}

fn parse_inputs(pairs: &[(String, String)]) -> Dict {
    let mut dict = Dict::new();
    for (key, value) in pairs {
        dict.insert(key.as_str().into(), value.as_str().into_value());
    }
    dict
}

fn validate_creation_timestamp(
    timestamp: Option<i64>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    timestamp
        .map(|seconds| {
            chrono::DateTime::from_timestamp(seconds, 0)
                .ok_or_else(|| "creation timestamp is out of range".to_owned())
        })
        .transpose()
}

fn initialize_jobs(jobs: Option<usize>) {
    if let Some(jobs) = jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .use_current_thread()
            .build_global()
            .ok();
    }
}

struct FilesystemResourceProvider {
    root: FsRoot,
    path: PathBuf,
    dependencies: Arc<Mutex<BTreeSet<PathBuf>>>,
}

impl FilesystemResourceProvider {
    fn new(root: PathBuf) -> Self {
        Self::tracked(root, Arc::new(Mutex::new(BTreeSet::new())))
    }

    fn tracked(root: PathBuf, dependencies: Arc<Mutex<BTreeSet<PathBuf>>>) -> Self {
        Self {
            root: FsRoot::new(root.clone()),
            path: root,
            dependencies,
        }
    }
}

impl FileLoader for FilesystemResourceProvider {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => {
                let result = self.root.load(id.vpath());
                if result.is_ok() {
                    self.dependencies
                        .lock()
                        .expect("host dependency lock poisoned")
                        .insert(self.path.join(id.vpath().get_without_slash()));
                }
                result
            }
            VirtualRoot::Package(_) => Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            ))),
        }
    }
}

fn write_dependencies(
    destination: &Path,
    format: DepsFormat,
    inputs: &BTreeSet<PathBuf>,
    outputs: Option<&[PathBuf]>,
) -> Result<(), String> {
    let mut bytes = Vec::new();
    match format {
        DepsFormat::Json => {
            let inputs = inputs
                .iter()
                .map(|path| {
                    path.to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("input {path:?} is not valid UTF-8"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let outputs = outputs
                .map(|outputs| {
                    outputs
                        .iter()
                        .filter(|path| path.as_path() != Path::new("-"))
                        .map(|path| {
                            path.to_str()
                                .map(str::to_owned)
                                .ok_or_else(|| format!("output {path:?} is not valid UTF-8"))
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            bytes = serde_json::to_vec(&serde_json::json!({
                "inputs": inputs,
                "outputs": outputs,
            }))
            .map_err(|error| error.to_string())?;
        }
        DepsFormat::Zero => {
            for input in inputs {
                bytes.extend_from_slice(input.as_os_str().as_encoded_bytes());
                bytes.push(0);
            }
        }
        DepsFormat::Make => {
            let Some(outputs) = outputs else {
                return Ok(());
            };
            for (index, output) in outputs.iter().enumerate() {
                if output == Path::new("-") {
                    return Err(
                        "make dependencies contain the output path, but the output was stdout"
                            .to_owned(),
                    );
                }
                let Some(output) = output.to_str() else {
                    continue;
                };
                if index != 0 {
                    bytes.push(b' ');
                }
                bytes.extend_from_slice(munge_make_path(output).as_bytes());
            }
            bytes.push(b':');
            for input in inputs {
                if let Some(input) = input.to_str() {
                    bytes.push(b' ');
                    bytes.extend_from_slice(munge_make_path(input).as_bytes());
                }
            }
            bytes.push(b'\n');
        }
    }

    if destination == Path::new("-") {
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|error| format!("cannot write dependencies to stdout: {error}"))
    } else {
        std::fs::write(destination, bytes).map_err(|error| {
            format!(
                "cannot write dependencies to `{}`: {error}",
                destination.display()
            )
        })
    }
}

fn munge_make_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut slashes = 0;
    for character in path.chars() {
        match character {
            '\\' => slashes += 1,
            '$' => {
                result.push('$');
                slashes = 0;
            }
            ':' => {
                result.push('\\');
                slashes = 0;
            }
            ' ' | '\t' => {
                for _ in 0..slashes + 1 {
                    result.push('\\');
                }
                slashes = 0;
            }
            '#' => {
                result.push('\\');
                slashes = 0;
            }
            _ => slashes = 0,
        }
        result.push(character);
    }
    result
}

/// Converts a Chrono datetime to a Typst datetime.
fn convert_datetime<Tz: chrono::TimeZone>(date_time: chrono::DateTime<Tz>) -> Option<Datetime> {
    Datetime::from_ymd_hms(
        date_time.year(),
        date_time.month().try_into().ok()?,
        date_time.day().try_into().ok()?,
        date_time.hour().try_into().ok()?,
        date_time.minute().try_into().ok()?,
        date_time.second().try_into().ok()?,
    )
}

fn local_timestamp(local: &chrono::DateTime<chrono::Local>) -> Option<Timestamp> {
    let datetime = Datetime::from_ymd_hms(
        local.year(),
        local.month().try_into().ok()?,
        local.day().try_into().ok()?,
        local.hour().try_into().ok()?,
        local.minute().try_into().ok()?,
        local.second().try_into().ok()?,
    )?;
    Timestamp::new_local(datetime, local.offset().local_minus_utc() / 60)
}

fn emit_diagnostics_with<'a>(
    world: &dyn DiagnosticWorld,
    diagnostics: impl IntoIterator<Item = &'a SourceDiagnostic>,
    format: DiagnosticFormat,
    color: ColorChoice,
) {
    let mut diagnostics = diagnostics.into_iter().peekable();
    if diagnostics.peek().is_none() {
        return;
    }
    let mut stream = StandardStream::stderr(color);
    let _ = typst_kit::diagnostics::emit(&mut stream, world, diagnostics, format);
    let _ = stream.reset();
}

fn emit_owned_error(message: &str, color: ColorChoice) {
    let mut stream = StandardStream::stderr(color);
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(Color::Red)).set_bold(true);
    let _ = stream.set_color(&spec);
    let _ = write!(stream, "error");
    let _ = stream.reset();
    let _ = writeln!(stream, ": {message}");
}

fn emit_owned_warning(message: &str, color: ColorChoice) {
    let mut stream = StandardStream::stderr(color);
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(Color::Yellow)).set_bold(true);
    let _ = stream.set_color(&spec);
    let _ = write!(stream, "warning");
    let _ = stream.reset();
    let _ = writeln!(stream, ": {message}");
}

fn emit_owned_hint(message: &str, color: ColorChoice) {
    let mut stream = StandardStream::stderr(color);
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(Color::Cyan)).set_bold(true);
    let _ = stream.set_color(&spec);
    let _ = write!(stream, "hint");
    let _ = stream.reset();
    let _ = writeln!(stream, ": {message}");
}

/// Formats a file ID with its package prefix, if any.
fn display_file_id(id: FileId) -> String {
    match id.root() {
        VirtualRoot::Project => id.vpath().get_without_slash().to_owned(),
        VirtualRoot::Package(spec) => {
            format!("{spec}{}", id.vpath().get_with_slash())
        }
    }
}

fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
    if path.is_absolute() != base.is_absolute() {
        return path.is_absolute().then(|| path.to_path_buf());
    }

    let mut path_components = path.components();
    let mut base_components = base.components();
    let mut relative = Vec::new();
    loop {
        match (path_components.next(), base_components.next()) {
            (None, None) => break,
            (Some(component), None) => {
                relative.push(component);
                relative.extend(path_components.by_ref());
                break;
            }
            (None, Some(_)) => relative.push(std::path::Component::ParentDir),
            (Some(path), Some(base)) if relative.is_empty() && path == base => {}
            (Some(path), Some(std::path::Component::CurDir)) => relative.push(path),
            (Some(_), Some(std::path::Component::ParentDir)) => return None,
            (Some(std::path::Component::Prefix(_) | std::path::Component::RootDir), Some(_))
            | (Some(_), Some(std::path::Component::Prefix(_) | std::path::Component::RootDir)) => {
                return path.is_absolute().then(|| path.to_path_buf());
            }
            (Some(path), Some(_)) => {
                relative.push(std::path::Component::ParentDir);
                relative.extend(base_components.map(|_| std::path::Component::ParentDir));
                relative.push(path);
                relative.extend(path_components.by_ref());
                break;
            }
        }
    }

    Some(relative.iter().map(|part| part.as_os_str()).collect())
}

impl DiagnosticWorld for DiscoveryWorld {
    fn name(&self, id: FileId) -> String {
        match id.root() {
            VirtualRoot::Project => id
                .vpath()
                .realize(self.root())
                .ok()
                .and_then(|path| relative_path(&path, self.workdir()?))
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| display_file_id(id)),
            VirtualRoot::Package(_) => display_file_id(id),
        }
    }
}

impl DiagnosticWorld for PackWorld {
    fn name(&self, id: FileId) -> String {
        display_file_id(id)
    }
}

fn human_size(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn incompatible_absolute_prefixes_keep_the_target_absolute() {
        let target = Path::new(r"C:\project\main.typ");
        let base = Path::new(r"D:\cwd");

        assert_eq!(relative_path(target, base), Some(target.to_path_buf()));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    use super::*;

    #[test]
    fn make_dependencies_omit_non_unicode_outputs_with_typst_spacing() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("deps.mk");
        let inputs = BTreeSet::from([PathBuf::from("input.typ")]);
        let outputs = [
            PathBuf::from(OsString::from_vec(b"invalid-\xff.pdf".to_vec())),
            PathBuf::from("valid.pdf"),
        ];

        write_dependencies(&destination, DepsFormat::Make, &inputs, Some(&outputs)).unwrap();

        assert_eq!(
            std::fs::read(destination).unwrap(),
            b" valid.pdf: input.typ\n"
        );
    }
}
