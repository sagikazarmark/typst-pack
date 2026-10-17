//! Command-line arguments. Shared options mirror Typst 0.15.1's spelling,
//! defaults, and help.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use typst::Feature;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_pack::{OutputFormat, PageRange, TypstTarget, parse_page_selection};
use typst_pdf::PdfStandard;

const ENV_PATH_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

/// Pack, inspect, extract, and compile portable Typst project packs.
#[derive(Debug, Parser)]
#[command(name = "typst-pack", version = typst_pack::VERSION, about)]
pub struct Cli {
    /// Whether to use color. When set to `auto` if the terminal to supports it.
    #[arg(
        long,
        default_value = "auto",
        default_missing_value = "always",
        num_args = 0..=1,
        value_parser = ["auto", "always", "never"]
    )]
    pub color: String,

    /// Path to a custom CA certificate to use when making network requests.
    #[arg(long, env = "TYPST_CERT", value_name = "PATH")]
    pub cert: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
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
pub struct CompilationArgs {
    /// Add a string key-value pair visible through `sys.inputs`.
    #[arg(long = "input", value_name = "key=value", value_parser = parse_input)]
    pub inputs: Vec<(String, String)>,

    /// Add a file's bytes under a key visible through `sys.inputs`, for
    /// example to pass an image to `image(sys.inputs.logo)`.
    #[arg(long = "input-file", value_name = "key=path", value_parser = parse_input_file())]
    pub input_files: Vec<(String, PathBuf)>,

    /// Enables in-development features that may be changed or removed at any
    /// time.
    #[arg(
        long = "features",
        value_name = "FEATURES",
        value_delimiter = ',',
        env = "TYPST_FEATURES",
        value_parser = one_of(FEATURES)
    )]
    pub features: Vec<Feature>,
}

#[derive(Debug, Args)]
pub struct FontArgs {
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
    pub font_paths: Vec<PathBuf>,

    /// Ensures system fonts won't be searched, unless explicitly included via
    /// `--font-path`.
    #[arg(long, env = "TYPST_IGNORE_SYSTEM_FONTS")]
    pub ignore_system_fonts: bool,

    /// Ensures fonts embedded into Typst won't be considered.
    #[arg(long, env = "TYPST_IGNORE_EMBEDDED_FONTS")]
    pub ignore_embedded_fonts: bool,
}

#[derive(Debug, Args)]
pub struct PackageArgs {
    /// Custom path to local packages, defaults to system-dependent location.
    #[arg(long, env = "TYPST_PACKAGE_PATH", value_name = "DIR")]
    pub package_path: Option<PathBuf>,

    /// Custom path to package cache, defaults to system-dependent location.
    #[arg(long, env = "TYPST_PACKAGE_CACHE_PATH", value_name = "DIR")]
    pub package_cache_path: Option<PathBuf>,

    /// Disallow network access; package dependencies must already be available
    /// in the local package directories.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct AutomationArgs {
    /// Number of parallel jobs spawned during compilation. Defaults to number
    /// of CPUs. Setting it to 1 disables parallelism.
    #[arg(long, short)]
    pub jobs: Option<usize>,

    /// The document's creation date formatted as a UNIX timestamp.
    ///
    /// For more information, see <https://reproducible-builds.org/specs/source-date-epoch/>.
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "UNIX_TIMESTAMP")]
    pub creation_timestamp: Option<i64>,

    /// The format to emit diagnostics in.
    #[arg(long, default_value = "human", value_parser = one_of(DIAGNOSTIC_FORMATS))]
    pub diagnostic_format: DiagnosticFormat,

    /// Produces performance timings of the compilation process. (experimental)
    ///
    /// The resulting JSON file can be loaded into a tracing tool such as
    /// https://ui.perfetto.dev. It does not contain any sensitive information
    /// apart from file names and line numbers.
    #[arg(long, value_name = "OUTPUT_JSON")]
    pub timings: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Path to the input Typst file.
    #[arg(help_heading = "Project")]
    pub input: PathBuf,

    /// Configures the project root (for absolute paths).
    #[arg(long, env = "TYPST_ROOT", value_name = "DIR", help_heading = "Project")]
    pub root: Option<PathBuf>,

    /// Target for the creation compile [default: paged].
    #[arg(long, value_name = "TARGET", value_parser = one_of(TARGETS), help_heading = "Creation")]
    pub target: Option<TypstTarget>,

    /// Also discover dependencies with these string inputs replacing or adding
    /// to `--input`. Repeat for several variants.
    #[arg(
        long = "variant",
        value_name = "key=value,...",
        value_parser = parse_variant,
        help_heading = "Creation"
    )]
    pub variants: Vec<Variant>,

    #[command(flatten, next_help_heading = "Creation")]
    pub compilation: CompilationArgs,

    /// Path to the output Pack [default: INPUT with its extension replaced by .typk].
    #[arg(help_heading = "Pack Contents")]
    pub output: Option<PathBuf>,

    /// Embed the fonts used by the document into the pack.
    #[arg(long, help_heading = "Pack Contents")]
    pub embed_fonts: bool,

    /// When embedding fonts, also embed the fonts Typst itself ships.
    #[arg(long, requires = "embed_fonts", help_heading = "Pack Contents")]
    pub include_typst_embedded_fonts: bool,

    #[command(flatten, next_help_heading = "Fonts")]
    pub fonts: FontArgs,

    /// Do not store package files in the pack; record them as unvendored
    /// dependencies instead.
    #[arg(long = "no-vendor-packages", help_heading = "Packages")]
    pub no_vendor_packages: bool,

    #[command(flatten, next_help_heading = "Packages")]
    pub packages: PackageArgs,

    /// A human-readable name recorded in the pack metadata.
    #[arg(long, help_heading = "Metadata")]
    pub name: Option<String>,

    /// A description recorded in the pack metadata.
    #[arg(long, help_heading = "Metadata")]
    pub description: Option<String>,

    /// Authors recorded in the pack metadata.
    #[arg(long = "author", value_name = "AUTHOR", help_heading = "Metadata")]
    pub authors: Vec<String>,

    #[command(flatten, next_help_heading = "Diagnostics & Automation")]
    pub automation: AutomationArgs,
}

#[derive(Debug, Args)]
pub struct InspectArgs {
    /// The pack file to inspect.
    pub pack: PathBuf,
}

#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// The pack file to extract.
    pub pack: PathBuf,

    /// The directory to extract into [default: <pack name>/].
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Also extract vendored packages to packages/.
    #[arg(long)]
    pub packages: bool,

    /// Also extract embedded fonts to fonts/.
    #[arg(long)]
    pub fonts: bool,

    /// Extract everything (same as --packages --fonts).
    #[arg(long)]
    pub all: bool,

    /// Overwrite existing files.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct CompileArgs {
    /// The pack file to compile.
    #[arg(help_heading = "Compilation")]
    pub pack: PathBuf,

    /// Path to output file (PDF, PNG, SVG, or HTML). Use `-` to write output to
    /// stdout.
    ///
    /// For output formats emitting one file per page (PNG & SVG), a page number
    /// template must be present if the source document renders to multiple
    /// pages. Use `{p}` for page numbers, `{0p}` for zero padded page numbers
    /// and `{t}` for page count. For example, `page-{0p}-of-{t}.png` creates
    /// `page-01-of-10.png`, `page-02-of-10.png`, and so on.
    #[arg(help_heading = "Output")]
    pub output: Option<PathBuf>,

    /// The format of the output file, inferred from the extension by default.
    #[arg(short, long, value_parser = one_of(FORMATS), help_heading = "Output")]
    pub format: Option<OutputFormat>,

    /// Whether to pretty-print produced output.
    ///
    /// This formats the output in a more human-readable, but less
    /// space-efficient way. Affects HTML, SVG, and PDF export, but not PNG
    /// export.
    #[arg(long, help_heading = "Output")]
    pub pretty: bool,

    #[command(flatten, next_help_heading = "Compilation")]
    pub compilation: CompilationArgs,

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
    #[arg(long, value_delimiter = ',', value_parser = parse_page_range, help_heading = "Output")]
    pub pages: Vec<PageRange>,

    /// The PPI (pixels per inch) to use for PNG export.
    #[arg(long, default_value_t = 144.0, help_heading = "Output")]
    pub ppi: f64,

    /// One (or multiple comma-separated) PDF standards that Typst will enforce
    /// conformance with.
    #[arg(
        long = "pdf-standard",
        value_name = "PDF_STANDARD",
        value_delimiter = ',',
        value_parser = one_of(PDF_STANDARDS),
        help_heading = "PDF"
    )]
    pub pdf_standards: Vec<PdfStandardArg>,

    /// By default, even when not producing a `PDF/UA-1` document, a tagged PDF
    /// document is written to provide a baseline of accessibility. In some
    /// circumstances (for example when trying to reduce the size of a document)
    /// it can be desirable to disable tagged PDF.
    #[arg(long = "no-pdf-tags", help_heading = "PDF")]
    pub no_pdf_tags: bool,

    #[command(flatten, next_help_heading = "Fonts")]
    pub fonts: FontArgs,

    #[command(flatten, next_help_heading = "Packages")]
    pub packages: PackageArgs,

    #[command(flatten, next_help_heading = "Diagnostics & Automation")]
    pub automation: AutomationArgs,

    /// File path to which a list of current compilation's dependencies will be
    /// written. Use `-` to write to stdout.
    #[arg(long, value_name = "PATH", help_heading = "Diagnostics & Automation")]
    pub deps: Option<PathBuf>,

    /// File format to use for dependencies.
    #[arg(
        long,
        default_value = "json",
        value_enum,
        help_heading = "Diagnostics & Automation"
    )]
    pub deps_format: DepsFormat,

    /// Opens the output file with the default viewer or a specific program
    /// after compilation. Ignored if output is stdout.
    #[arg(
        long,
        value_name = "VIEWER",
        num_args = 0..=1,
        help_heading = "Diagnostics & Automation"
    )]
    pub open: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DepsFormat {
    /// Encodes as JSON, failing for non-Unicode paths.
    Json,
    /// Separates paths with NULL bytes and can express all paths.
    Zero,
    /// Emits in Make format, omitting inexpressible paths.
    Make,
}

/// A parser accepting exactly the given names.
fn one_of<T: Copy + Send + Sync + 'static>(
    values: &'static [(&'static str, T)],
) -> impl clap::builder::TypedValueParser<Value = T> {
    use clap::builder::TypedValueParser;
    let names = values.iter().map(|(name, _)| *name);
    clap::builder::PossibleValuesParser::new(names).map(|name| {
        let value = values.iter().find(|(candidate, _)| *candidate == name);
        value.expect("clap accepted a listed name").1
    })
}

const DIAGNOSTIC_FORMATS: &[(&str, DiagnosticFormat)] = &[
    ("human", DiagnosticFormat::Human),
    ("short", DiagnosticFormat::Short),
];

const FEATURES: &[(&str, Feature)] = &[
    ("html", Feature::Html),
    ("a11y-extras", Feature::A11yExtras),
];

const TARGETS: &[(&str, TypstTarget)] =
    &[("paged", TypstTarget::Paged), ("html", TypstTarget::Html)];

const FORMATS: &[(&str, OutputFormat)] = &[
    ("pdf", OutputFormat::Pdf),
    ("png", OutputFormat::Png),
    ("svg", OutputFormat::Svg),
    ("html", OutputFormat::Html),
];

/// A PDF standard as Typst's CLI names it, and the name of a standard that
/// requires accessibility tags.
pub type PdfStandardArg = (PdfStandard, Option<&'static str>);

const PDF_STANDARDS: &[(&str, PdfStandardArg)] = &[
    ("1.4", (PdfStandard::V_1_4, None)),
    ("1.5", (PdfStandard::V_1_5, None)),
    ("1.6", (PdfStandard::V_1_6, None)),
    ("1.7", (PdfStandard::V_1_7, None)),
    ("2.0", (PdfStandard::V_2_0, None)),
    ("a-1b", (PdfStandard::A_1b, None)),
    ("a-1a", (PdfStandard::A_1a, Some("PDF/A-1a"))),
    ("a-2b", (PdfStandard::A_2b, None)),
    ("a-2u", (PdfStandard::A_2u, None)),
    ("a-2a", (PdfStandard::A_2a, Some("PDF/A-2a"))),
    ("a-3b", (PdfStandard::A_3b, None)),
    ("a-3u", (PdfStandard::A_3u, None)),
    ("a-3a", (PdfStandard::A_3a, Some("PDF/A-3a"))),
    ("a-4", (PdfStandard::A_4, None)),
    ("a-4f", (PdfStandard::A_4f, None)),
    ("a-4e", (PdfStandard::A_4e, None)),
    ("ua-1", (PdfStandard::Ua_1, Some("PDF/UA-1"))),
];

fn parse_input(pair: &str) -> Result<(String, String), String> {
    let (key, value) = pair
        .split_once('=')
        .ok_or("input must be a key and a value separated by an equal sign")?;
    let key = key.trim();
    if key.is_empty() {
        return Err("the key was missing or empty".to_owned());
    }
    Ok((key.to_owned(), value.trim().to_owned()))
}

fn parse_input_file() -> impl clap::builder::TypedValueParser<Value = (String, PathBuf)> {
    use clap::builder::TypedValueParser;
    clap::builder::OsStringValueParser::new().try_map(|pair| {
        let bytes = pair.as_encoded_bytes();
        let split = bytes.iter().position(|&byte| byte == b'=');
        let split = split.ok_or("input must be a key and a path separated by an equal sign")?;
        let key = std::str::from_utf8(&bytes[..split]).map_err(|_| "the key must be UTF-8")?;
        // SAFETY: the bytes come from an `OsStr` and are split right after an
        // ASCII `=`, which is a valid boundary for its encoding.
        let path = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(&bytes[split + 1..]) };
        match key.trim() {
            "" => Err("the key was missing or empty"),
            key => Ok((key.to_owned(), PathBuf::from(path))),
        }
    })
}

/// The string inputs of one discovery variant.
#[derive(Debug, Clone)]
pub struct Variant(pub Vec<(String, String)>);

fn parse_variant(variant: &str) -> Result<Variant, String> {
    variant
        .split(',')
        .map(parse_input)
        .collect::<Result<_, _>>()
        .map(Variant)
}

fn parse_page_range(value: &str) -> Result<PageRange, String> {
    let selection = parse_page_selection(value)?;
    Ok(selection.ranges()[0].clone())
}
