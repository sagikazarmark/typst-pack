//! The `typst-pack` command line interface.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Timelike};
use clap::{Args, Parser, Subcommand, ValueEnum};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue};
use typst::syntax::VirtualRoot;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::{
    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
};
use typst_kit::fonts::FontSource;
use typst_pdf::{PdfStandard, Timestamp};

use typst_pack::cli_support::{
    CliCompilationExecution, CliCompilationOutcome, CliCompilationPresentation,
    FilesystemPackageAuthority, compile_with_timing, emit_creation_error_diagnostics,
    emit_creation_warnings, pdf_standard_requiring_tags, validate_pdf_standards,
};
use typst_pack::pack_archive::{
    AcquisitionLimits, DecodeLimits, EncodeLimits, FORMAT_VERSION, FileAcquisitionError,
    FilePublicationPolicy, OpenPackError, open_pack as open_pack_archive,
    read_pack as read_pack_archive, save_pack, write_pack,
};
use typst_pack::{
    CompilationArtifact, CompilationArtifactPathPublicationError,
    CompilationArtifactPublicationError, CompilationFulfillmentSet, CompilationLimits,
    CompilationOutputSpecification, CompilationReportOutcome, CompilationStatus, CreationTimestamp,
    DocumentTime, FontContainer, FontContainerFulfillment, HtmlOutputSpecification, OutputFormat,
    PackCompilationRequest, PackOverrideSet, PackageTreeFulfillment, PageRange, PageSelection,
    PdfOutputSpecification, PngOutputSpecification, SvgOutputSpecification, TypstTarget,
    parse_page_selection, plan_compilation_artifact_publication,
    publish_compilation_artifact_plan_to_filesystem_paths,
};
use typst_pack::{
    FILE_EXTENSION, FilesystemMergePolicy, FilesystemPackAssembler, FilesystemPackAssemblerConfig,
    FilesystemPackAssemblyError, FilesystemPackAssemblyRequest,
    FilesystemPublicationPreflightIssue, FontContainerIdentity, Pack, PackCreationError,
    PackExtractionPublicationError, PackExtractionSelection, PackMetadata, plan_pack_extraction,
    publish_pack_extraction_plan_to_filesystem,
};

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
    version = typst_pack::VERSION,
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

    /// Target for the representative creation compilation [default: paged].
    #[arg(long = "target", value_name = "TARGET", help_heading = "Creation")]
    target: Option<TypstTargetArg>,

    #[command(flatten, next_help_heading = "Creation")]
    compilation: SharedCompilationArgs,

    /// Path to the output Pack [default: INPUT with its extension replaced by .typk].
    #[arg(help_heading = "Pack Contents")]
    output: Option<PathBuf>,

    /// Embed the fonts used by the document into the pack.
    #[arg(long, help_heading = "Pack Contents")]
    embed_fonts: bool,

    /// When embedding fonts, also embed the fonts Typst itself ships.
    #[arg(long, requires = "embed_fonts", help_heading = "Pack Contents")]
    include_typst_embedded_fonts: bool,

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
        help_heading = "Overrides"
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
enum TypstTargetArg {
    Paged,
    Html,
}

impl From<TypstTargetArg> for TypstTarget {
    fn from(value: TypstTargetArg) -> Self {
        match value {
            TypstTargetArg::Paged => Self::Paged,
            TypstTargetArg::Html => Self::Html,
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

    let mut config = FilesystemPackAssemblerConfig::new()
        .typst_embedded_fonts(!args.fonts.ignore_embedded_fonts)
        .system_fonts(!args.fonts.ignore_system_fonts)
        .offline(args.packages.offline)
        .certificate(cert.map(Path::to_path_buf));
    for path in &args.fonts.font_paths {
        config = config.font_path(path);
    }
    if let Some(path) = &args.packages.package_path {
        config = config.package_path(path);
    }
    if let Some(path) = &args.packages.package_cache_path {
        config = config.package_cache_path(path);
    }

    let mut request = FilesystemPackAssemblyRequest::new(&root, &input)
        .vendor_packages(!args.no_vendor_packages)
        .embed_fonts(args.embed_fonts)
        .include_typst_embedded_fonts(args.include_typst_embedded_fonts)
        .timings(args.automation.timings.clone())
        .inputs(parse_inputs(&args.compilation.inputs));
    if let Some(timestamp) = args.automation.creation_timestamp {
        request = request.document_time(DocumentTime::UnixTimestamp(timestamp));
    }
    if let Some(target) = args.target {
        request = request.target(target.into());
    }
    for feature in args.compilation.features {
        request = request.feature(feature.into());
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
        request = request.metadata(metadata);
    }

    let assembler = FilesystemPackAssembler::new(config);
    let (report, timing_error) = assembler.assemble_with_timing(request);
    let timing_error = timing_error.map(|error| error.to_string());
    let report = match report {
        Ok(report) => report,
        Err(FilesystemPackAssemblyError::ProjectGather(
            typst_pack::FilesystemProjectGatherError::Snapshot(error),
        )) if matches!(
            error.issues(),
            [typst_pack::ProjectSnapshotIssue::MissingEntrypoint { .. }]
        ) =>
        {
            let [typst_pack::ProjectSnapshotIssue::MissingEntrypoint { path }] = error.issues()
            else {
                unreachable!("the match guard accepted only one missing entrypoint issue")
            };
            return Err(
                format!("entrypoint `{path}` is excluded by the Project Ignore Policy").into(),
            );
        }
        Err(FilesystemPackAssemblyError::Creation(error))
            if matches!(
                error.error(),
                PackCreationError::DependencyDiscoveryRejected(_)
            ) =>
        {
            let (context, error, _) = error.into_parts();
            let PackCreationError::DependencyDiscoveryRejected(rejection) = error else {
                unreachable!("the match guard accepted only discovery rejection");
            };
            let mut stream = StandardStream::stderr(color);
            emit_creation_error_diagnostics(
                &context,
                rejection.diagnostics().iter().chain(rejection.warnings()),
                &mut stream,
                diagnostic_format,
            );
            if let Some(error) = timing_error {
                return Err(error.into());
            }
            return Err(CliError::Reported);
        }
        Err(err) => {
            if let Some(error) = timing_error {
                emit_owned_error(&err.to_string(), color);
                return Err(error.into());
            }
            return Err(err.to_string().into());
        }
    };

    let mut stream = StandardStream::stderr(color);
    emit_creation_warnings(&report, &mut stream, diagnostic_format);
    if let Some(error) = timing_error {
        return Err(error.into());
    }

    if output == Path::new("-") {
        write_pack(
            std::io::stdout().lock(),
            report.pack(),
            EncodeLimits::reference_v1(),
        )
        .map_err(|err| format!("cannot write Pack to stdout: {err}"))?;
        return Ok(());
    }

    let policy = if output
        .try_exists()
        .map_err(|error| format!("cannot inspect `{}`: {error}", output.display()))?
    {
        FilePublicationPolicy::ReplaceExisting
    } else {
        FilePublicationPolicy::CreateNew
    };
    save_pack(&output, report.pack(), EncodeLimits::reference_v1(), policy)
        .map_err(|error| error.to_string())?;

    let project_file_count = report.pack().files().count();
    let vendored_package_count = report
        .pack()
        .package_requirements()
        .iter()
        .filter(|requirement| requirement.is_embedded())
        .count();
    let unvendored_packages = report
        .pack()
        .package_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
        .collect::<Vec<_>>();
    let font_count = report.pack().font_catalog().len();
    println!(
        "packed {} project file(s), {} package(s), {} font(s) into `{}`",
        project_file_count,
        vendored_package_count,
        font_count,
        output.display(),
    );
    if !unvendored_packages.is_empty() {
        println!(
            "note: {} package(s) were not vendored and must be available when compiling:",
            unvendored_packages.len()
        );
        for requirement in unvendored_packages {
            println!("  {}", requirement.spec());
        }
    }
    Ok(())
}

fn inspect(args: InspectArgs) -> CliResult {
    let pack = read_pack(&args.pack)?;

    println!("pack: {}", args.pack.display());
    println!("format version: {FORMAT_VERSION}");
    println!("entrypoint: {}", pack.entrypoint());
    if let Some(metadata) = pack.metadata() {
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
    let unvendored = pack
        .package_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
        .collect::<Vec<_>>();
    if !unvendored.is_empty() {
        println!("\nunvendored packages:");
        for requirement in unvendored {
            println!("  {}", requirement.spec());
        }
    }

    if !pack.fonts().is_empty() {
        println!("\nembedded fonts:");
        for font in pack.fonts() {
            let identity = font.identity();
            let digest = identity
                .container()
                .digest()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            println!(
                "  {}:{}:{}:{digest} face {} ({}) - {}",
                identity.container().kind(),
                identity.container().schema(),
                identity.container().algorithm(),
                identity.index(),
                human_size(font.data().len()),
                font.info().family,
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

    let plan = plan_pack_extraction(
        &pack,
        PackExtractionSelection::new(args.packages || args.all, args.fonts || args.all),
    )
    .map_err(|error| error.to_string())?;
    let policy = if args.force {
        FilesystemMergePolicy::MergeReplaceExactFiles
    } else {
        FilesystemMergePolicy::MergeCreateOnly
    };
    let receipt = publish_pack_extraction_plan_to_filesystem(&plan, &output, policy)
        .map_err(|error| format_extraction_publication_error(&error))?;

    println!(
        "extracted {} file(s) into `{}`",
        receipt.progress().committed_files().len(),
        output.display()
    );
    Ok(())
}

fn format_extraction_publication_error(error: &PackExtractionPublicationError) -> String {
    match error.preflight_issues().and_then(|issues| issues.first()) {
        Some(FilesystemPublicationPreflightIssue::ExistingTarget { relative_path }) => format!(
            "`{}` already exists (pass force to overwrite)",
            error.destination().join(relative_path).display()
        ),
        Some(FilesystemPublicationPreflightIssue::ConflictingTarget { relative_path, .. }) => {
            format!(
                "existing destination entry `{}` conflicts with extraction",
                error.destination().join(relative_path).display()
            )
        }
        Some(FilesystemPublicationPreflightIssue::ConflictingAncestor { ancestor, .. })
        | Some(FilesystemPublicationPreflightIssue::ConflictingDestinationRoot {
            path: ancestor,
            ..
        }) => format!(
            "existing destination entry `{}` conflicts with extraction",
            ancestor.display()
        ),
        _ => error.to_string(),
    }
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
    let pdf_tags = if args.no_pdf_tags {
        typst::foundations::Smart::Custom(false)
    } else {
        typst::foundations::Smart::Auto
    };
    if let Err(error) = validate_pdf_standards(&standards) {
        let (message, hints) = error.into_parts();
        return Err(CliError::Hinted { message, hints });
    }
    let tags_disabled = args.no_pdf_tags || !page_selection.ranges().is_empty();
    if tags_disabled && let Some(name) = pdf_standard_requiring_tags(&standards) {
        let message = format!("cannot disable PDF tags when exporting a {name} document");
        return Err(if args.no_pdf_tags {
            CliError::Message(message)
        } else {
            CliError::Hinted {
                message,
                hints: vec!["using --pages implies --no-pdf-tags".to_owned()],
            }
        });
    }

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
    let packages = FilesystemPackageAuthority::new(
        args.packages.package_path.as_deref(),
        args.packages.package_cache_path.as_deref(),
        args.packages.offline,
    )
    .certificate(cert.map(Path::to_path_buf));
    let mut package_roots = BTreeMap::new();
    let mut package_fulfillments = Vec::new();
    for requirement in pack
        .package_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
    {
        let acquired = packages.acquire(requirement.spec()).map_err(|error| {
            CliError::Message(format!(
                "external package fulfillment for {} is unavailable: {}",
                requirement.spec(),
                error
            ))
        })?;
        let (tree, root) = acquired.into_parts();
        if let Some(root) = root {
            package_roots.insert(requirement.spec().to_string(), root);
        }
        package_fulfillments.push((requirement.spec().clone(), tree));
    }
    let pdf_creation_timestamp = match creation_timestamp {
        Some(timestamp) => convert_datetime(timestamp)
            .map(Timestamp::new_utc)
            .map_or(CreationTimestamp::Omit, CreationTimestamp::Explicit),
        None => system_time
            .as_ref()
            .and_then(local_timestamp)
            .map(CreationTimestamp::Explicit)
            .unwrap_or(CreationTimestamp::Omit),
    };
    let output_specification = match format {
        OutputFormat::Pdf => CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            page_selection,
            standards,
            identifier: typst::foundations::Smart::Auto,
            creator: typst::foundations::Smart::Auto,
            tags: pdf_tags,
            creation_timestamp: pdf_creation_timestamp,
            pretty: args.pretty,
        }),
        OutputFormat::Png => CompilationOutputSpecification::Png(PngOutputSpecification {
            page_selection,
            pixels_per_inch: Some(args.ppi),
            render_bleed: false,
        }),
        OutputFormat::Svg => CompilationOutputSpecification::Svg(SvgOutputSpecification {
            page_selection,
            render_bleed: false,
            pretty: args.pretty,
        }),
        OutputFormat::Html => CompilationOutputSpecification::Html(HtmlOutputSpecification {
            pretty: args.pretty,
        }),
    };

    let document_timestamp = creation_timestamp_seconds.unwrap_or_else(|| {
        system_time
            .as_ref()
            .expect("system time is frozen when no explicit timestamp is supplied")
            .with_timezone(&chrono::Utc)
            .timestamp()
    });
    let external_font_identities = pack
        .font_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
        .map(|requirement| requirement.container_identity())
        .collect::<BTreeSet<_>>();
    let mut request =
        PackCompilationRequest::new_with_adapter_resolved_output(pack, output_specification)
            .adapter_resolved_inputs(parse_inputs(&args.compilation.inputs))
            .adapter_resolved_document_time(DocumentTime::UnixTimestamp(document_timestamp));
    if !args.overrides.is_empty() {
        request = request.overrides(overrides);
    }
    for feature in &args.compilation.features {
        request = request.adapter_resolved_feature((*feature).into());
    }
    let font_fulfillments = supplied_fonts
        .into_iter()
        .filter(|(identity, _)| external_font_identities.contains(identity))
        .map(|(identity, data)| {
            FontContainer::new(data.to_vec())
                .map(|container| FontContainerFulfillment::new(identity, container))
                .map_err(|error| CliError::Message(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let package_fulfillments = package_fulfillments
        .into_iter()
        .map(|(spec, tree)| PackageTreeFulfillment::new(spec, tree))
        .collect::<Vec<_>>();
    let fulfillments = CompilationFulfillmentSet::new(package_fulfillments, font_fulfillments)
        .expect("filesystem acquisition supplies each exact dependency at most once");
    request = request.fulfillments(fulfillments);
    let timed = compile_with_timing(
        request,
        reference_compilation_limits(rayon::current_num_threads()),
        args.automation.timings.clone(),
    )
    .map_err(|error| CliError::Message(error.to_string()))?;
    let (outcome, timing_error) = timed.into_parts();

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

    let (execution, mut command_result) = match outcome {
        Some(CliCompilationOutcome::Execution(execution)) => (Some(execution), None),
        Some(CliCompilationOutcome::Operation(report)) => {
            let CompilationReportOutcome::Operation { outcome, .. } = report.outcome() else {
                unreachable!("the timing adapter returned a result as an operation")
            };
            (None, Some(Err(CliError::Message(outcome.to_string()))))
        }
        None => (None, None),
    };
    if let Some(execution) = execution {
        command_result = Some((|| -> CliResult {
            for id in execution.file_dependencies() {
                if let VirtualRoot::Package(spec) = id.root()
                    && let Some(root) = package_roots.get(&spec.to_string())
                {
                    host_dependencies
                        .lock()
                        .expect("host dependency lock poisoned")
                        .insert(root.join(id.vpath().get_without_slash()));
                }
            }
            match execution.presentation() {
                CliCompilationPresentation::Succeeded => {}
                CliCompilationPresentation::Diagnostics => {
                    emit_compilation_diagnostics(&execution, diagnostic_format, color);
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
                CliCompilationPresentation::PngExport { error } => {
                    emit_owned_error(error, color);
                    emit_compilation_diagnostics(&execution, diagnostic_format, color);
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
            }
            debug_assert_eq!(execution.result().status(), CompilationStatus::Succeeded);
            let output = execution.result();

            let export_result = (|| {
                let plan = plan_compilation_artifact_publication(output)
                    .map_err(|error| error.to_string())?;
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
                        .write_all(plan.entries()[0].bytes())
                        .map_err(|err| format!("cannot write output to stdout: {err}"))?;
                } else {
                    let (destination, relative_paths) = filesystem_publication_paths(&targets)?;
                    publish_compilation_artifact_plan_to_filesystem_paths(
                        &plan,
                        destination,
                        &relative_paths,
                        FilesystemMergePolicy::MergeReplaceExactFiles,
                    )
                    .map_err(|error| format_compilation_publication_error(&error))?;
                }
                Ok::<_, String>((targets, output_is_stdout))
            })();
            let (targets, output_is_stdout) = match export_result {
                Ok(exported) => exported,
                Err(error) => {
                    emit_owned_error(&error, color);
                    if matches!(
                        execution.presentation(),
                        CliCompilationPresentation::Succeeded
                    ) {
                        emit_compilation_diagnostics(&execution, diagnostic_format, color);
                    }
                    write_requested_dependencies(None)?;
                    return Err(CliError::Reported);
                }
            };

            if matches!(
                execution.presentation(),
                CliCompilationPresentation::Succeeded
            ) {
                emit_compilation_diagnostics(&execution, diagnostic_format, color);
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
    }
    let Some(command_result) = command_result else {
        return Err(timing_error
            .expect("timer did not execute compilation without reporting an error")
            .into());
    };
    if let Some(error) = timing_error {
        emit_owned_error(&error, color);
        return Err(CliError::Reported);
    }
    command_result
}

fn filesystem_publication_paths(targets: &[PathBuf]) -> Result<(PathBuf, Vec<PathBuf>), String> {
    if targets.is_empty() {
        let current = Path::new(".")
            .canonicalize()
            .map_err(|error| format!("cannot resolve the current directory: {error}"))?;
        return Ok((current, Vec::new()));
    }
    let resolved_targets = targets
        .iter()
        .map(|target| {
            let parent = target.parent().unwrap_or_else(|| Path::new("."));
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            let parent = parent.canonicalize().map_err(|error| {
                format!(
                    "cannot resolve output directory `{}`: {error}",
                    parent.display()
                )
            })?;
            let file_name = target.file_name().ok_or_else(|| {
                format!("output path `{}` does not name a file", target.display())
            })?;
            Ok(parent.join(file_name))
        })
        .collect::<Result<Vec<PathBuf>, String>>()?;
    let mut destination = resolved_targets[0]
        .parent()
        .expect("a resolved output file has a parent")
        .to_owned();
    while resolved_targets
        .iter()
        .any(|target| !target.starts_with(&destination))
    {
        if !destination.pop() {
            return Err("output paths do not share a filesystem root".to_owned());
        }
    }
    let relative_paths = resolved_targets
        .iter()
        .map(|target| {
            target
                .strip_prefix(&destination)
                .expect("the selected destination is a common path prefix")
                .to_owned()
        })
        .collect();
    Ok((destination, relative_paths))
}

fn format_compilation_publication_error(error: &CompilationArtifactPathPublicationError) -> String {
    error.publication_error().map_or_else(
        || error.to_string(),
        format_compilation_filesystem_publication_error,
    )
}

fn format_compilation_filesystem_publication_error(
    error: &CompilationArtifactPublicationError,
) -> String {
    if let Some(issue) = error.preflight_issues().and_then(|issues| issues.first()) {
        let relative_path = match issue {
            FilesystemPublicationPreflightIssue::ExistingTarget { relative_path }
            | FilesystemPublicationPreflightIssue::ConflictingTarget { relative_path, .. }
            | FilesystemPublicationPreflightIssue::ConflictingAncestor { relative_path, .. } => {
                Some(relative_path)
            }
            _ => None,
        };
        if let Some(relative_path) = relative_path {
            return format!(
                "cannot write `{}`: {issue}",
                error.destination().join(relative_path).display()
            );
        }
    }
    if let Some(target) = error.failed_target() {
        return format!("cannot write `{}`: {}", target.display(), error.cause());
    }
    error.to_string()
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
    open_pack_archive(
        path,
        AcquisitionLimits::reference_v1(),
        DecodeLimits::reference_v1(),
    )
    .map_err(|error| match error {
        OpenPackError::Acquire(FileAcquisitionError::Open { source, .. }) => {
            format!("cannot open `{}`: {source}", path.display())
        }
        error => error.to_string(),
    })
}

fn read_pack_input(path: &Path) -> Result<Pack, String> {
    if path == Path::new("-") {
        return read_pack_archive(
            std::io::stdin().lock(),
            AcquisitionLimits::reference_v1(),
            DecodeLimits::reference_v1(),
        )
        .map_err(|error| format!("cannot read Pack from stdin: {error}"));
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

fn reference_compilation_limits(worker_count: usize) -> CompilationLimits {
    let limits = CompilationLimits::reference_v1();
    let worker_count = u64::try_from(worker_count)
        .unwrap_or(limits.export_workers())
        .clamp(1, limits.export_workers());
    limits
        .with_export_workers(worker_count)
        .expect("the first-party worker ceiling is finite and nonzero")
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

fn emit_compilation_diagnostics(
    execution: &CliCompilationExecution,
    format: DiagnosticFormat,
    color: ColorChoice,
) {
    let mut stream = StandardStream::stderr(color);
    execution.emit_diagnostics(&mut stream, format);
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

fn emit_owned_hint(message: &str, color: ColorChoice) {
    let mut stream = StandardStream::stderr(color);
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(Color::Cyan)).set_bold(true);
    let _ = stream.set_color(&spec);
    let _ = write!(stream, "hint");
    let _ = stream.reset();
    let _ = writeln!(stream, ": {message}");
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

#[cfg(test)]
mod parser_tests {
    use clap::Parser as _;

    use super::{Cli, reference_compilation_limits};

    #[test]
    fn uses_typst_embedded_font_terminology() {
        assert!(
            Cli::try_parse_from([
                "typst-pack",
                "create",
                "project",
                "--embed-fonts",
                "--include-typst-embedded-fonts",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "typst-pack",
                "create",
                "project",
                "--embed-fonts",
                "--include-default-fonts",
            ])
            .is_err()
        );
    }

    #[test]
    fn reference_compilation_workers_honor_jobs_without_exceeding_the_profile() {
        assert_eq!(reference_compilation_limits(1).export_workers(), 1);
        assert_eq!(reference_compilation_limits(4).export_workers(), 4);
        assert_eq!(reference_compilation_limits(64).export_workers(), 4);
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
    fn artifact_publication_resolves_symlink_and_parent_components_natively() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        let nested = actual.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        symlink(&nested, directory.path().join("linked")).unwrap();
        let target = directory.path().join("linked/../output.pdf");

        let (destination, relative) = filesystem_publication_paths(&[target]).unwrap();

        assert_eq!(destination, actual.canonicalize().unwrap());
        assert_eq!(relative, [PathBuf::from("output.pdf")]);
    }

    #[test]
    fn artifact_publication_resolves_distinct_native_output_parents() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        let first = actual.join("first/nested");
        let second = actual.join("second/nested");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        symlink(&first, directory.path().join("first-link")).unwrap();
        symlink(&second, directory.path().join("second-link")).unwrap();
        let targets = [
            directory.path().join("first-link/../output-1.svg"),
            directory.path().join("second-link/../output-2.svg"),
        ];

        let (destination, relative) = filesystem_publication_paths(&targets).unwrap();

        assert_eq!(destination, actual.canonicalize().unwrap());
        assert_eq!(
            relative,
            [
                PathBuf::from("first/output-1.svg"),
                PathBuf::from("second/output-2.svg"),
            ]
        );
    }

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
