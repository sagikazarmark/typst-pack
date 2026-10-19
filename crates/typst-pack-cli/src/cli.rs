//! Command-line arguments. Shared options mirror Typst 0.15.1's spelling and
//! defaults; their help follows Typst's, shortened in places.

use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand, ValueEnum};
use typst::Feature;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_pack::compile::{OutputFormat, PageSelection};
use typst_pack::create::Target;
use typst_pdf::PdfStandard;

const ENV_PATH_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

/// Pack, inspect, extract, and compile portable Typst project packs.
#[derive(Debug, Parser)]
#[command(name = "typst-pack", version = typst_pack::VERSION)]
pub struct Cli {
    /// Whether to use color. When set to `auto`, color is used if the terminal
    /// supports it.
    #[arg(long, default_value = "auto", default_missing_value = "always", num_args = 0..=1,
          value_parser = ["auto", "always", "never"])]
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

    /// Add a file's bytes under a key visible through `sys.inputs`.
    #[arg(long = "input-file", value_name = "key=path", value_parser = parse_input_file())]
    pub input_files: Vec<(String, PathBuf)>,

    /// Enables in-development features that may be changed or removed at any
    /// time.
    #[arg(long = "features", value_name = "FEATURES", value_delimiter = ',',
          env = "TYPST_FEATURES", value_parser = one_of(FEATURES))]
    pub features: Vec<Feature>,
}

const FEATURES: &[(&str, Feature)] = &[
    ("html", Feature::Html),
    ("a11y-extras", Feature::A11yExtras),
];

#[derive(Debug, Args)]
pub struct FontArgs {
    /// Adds additional directories that are recursively searched for fonts,
    /// separated by the system's path separator.
    #[arg(long = "font-path", env = "TYPST_FONT_PATHS", value_name = "DIR",
          value_delimiter = ENV_PATH_SEPARATOR)]
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

    /// Disallow network access; packages must already be available locally.
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
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "UNIX_TIMESTAMP")]
    pub creation_timestamp: Option<i64>,

    /// The format to emit diagnostics in.
    #[arg(long, default_value = "human", value_parser = one_of(DIAGNOSTIC_FORMATS))]
    pub diagnostic_format: DiagnosticFormat,

    /// Produces performance timings of the compilation process as JSON for a
    /// tracing tool such as https://ui.perfetto.dev. (experimental)
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
    #[arg(long, value_name = "TARGET", help_heading = "Creation",
          value_parser = one_of(&[("paged", Target::Paged), ("html", Target::Html)]))]
    pub target: Option<Target>,

    /// Also discover dependencies with these string inputs replacing or adding
    /// to `--input` and `--input-file`. Repeat for several variants.
    #[arg(long = "variant", value_name = "key=value,...", value_parser = parse_variant,
          help_heading = "Creation")]
    pub variants: Vec<Variant>,

    #[command(flatten, next_help_heading = "Creation")]
    pub compilation: CompilationArgs,

    /// Path to the output pack, or `-` for stdout [default: INPUT with its
    /// extension replaced by .typk].
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

    /// Record packages as unvendored dependencies instead of storing them.
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
    /// The pack file to inspect. Use `-` to read from stdin.
    pub pack: PathBuf,
}

#[derive(Debug, Args)]
pub struct ExtractArgs {
    /// The pack file to extract. Use `-` to read from stdin.
    pub pack: PathBuf,

    /// The directory to extract into [default: the pack's file name without
    /// its extension; required for stdin].
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
    /// The pack file to compile. Use `-` to read from stdin.
    #[arg(help_heading = "Compilation")]
    pub pack: PathBuf,

    /// Path to output file (PDF, PNG, SVG, or HTML). Use `-` for stdout
    /// [default: PACK with its extension replaced by the format's; required
    /// when PACK is `-`].
    ///
    /// PNG and SVG emit one file per page: with several pages, the path needs
    /// `{p}` or `{0p}` (zero padded) and may add `{t}` (page count), as in
    /// `page-{0p}-of-{t}.png`.
    #[arg(help_heading = "Output")]
    pub output: Option<PathBuf>,

    /// The format of the output file, inferred from the extension by default.
    #[arg(short, long, help_heading = "Output",
          value_parser = PossibleValuesParser::new(["pdf", "png", "svg", "html"])
              .map(|name| OutputFormat::from_extension(&name).expect("a listed format")))]
    pub format: Option<OutputFormat>,

    /// Whether to pretty-print HTML, SVG, and PDF output.
    #[arg(long, help_heading = "Output")]
    pub pretty: bool,

    #[command(flatten, next_help_heading = "Compilation")]
    pub compilation: CompilationArgs,

    /// Which pages to export, as comma-separated one-indexed physical page
    /// numbers or ranges (e.g. `2,3-6,8-`). When unspecified, all pages are
    /// exported.
    #[arg(long, value_delimiter = ',', help_heading = "Output")]
    pub pages: Vec<PageSelection>,

    /// The PPI (pixels per inch) to use for PNG export.
    #[arg(long, default_value_t = 144.0, help_heading = "Output")]
    pub ppi: f64,

    /// One (or multiple comma-separated) PDF standards that Typst will enforce
    /// conformance with.
    #[arg(long = "pdf-standard", value_name = "PDF_STANDARD", value_delimiter = ',',
          help_heading = "PDF", value_parser = pdf_standard())]
    pub pdf_standards: Vec<PdfStandard>,

    /// Do not write a tagged PDF, which is otherwise written for a baseline of
    /// accessibility.
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
    #[arg(long, value_name = "VIEWER", num_args = 0..=1,
          help_heading = "Diagnostics & Automation")]
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

/// Typst's names of the PDF standards, which are also their serde names.
const PDF_STANDARDS: [&str; 17] = [
    "1.4", "1.5", "1.6", "1.7", "2.0", "a-1b", "a-1a", "a-2b", "a-2u", "a-2a", "a-3b", "a-3u",
    "a-3a", "a-4", "a-4f", "a-4e", "ua-1",
];

fn pdf_standard() -> impl TypedValueParser<Value = PdfStandard> {
    PossibleValuesParser::new(PDF_STANDARDS)
        .map(|name| serde_json::from_value(name.into()).expect("Typst names every listed standard"))
}

const DIAGNOSTIC_FORMATS: &[(&str, DiagnosticFormat)] = &[
    ("human", DiagnosticFormat::Human),
    ("short", DiagnosticFormat::Short),
];

/// A parser accepting exactly the given names.
fn one_of<T: Copy + Send + Sync + 'static>(
    values: &'static [(&'static str, T)],
) -> impl TypedValueParser<Value = T> {
    let names = values.iter().map(|(name, _)| *name);
    PossibleValuesParser::new(names).map(|name| {
        let value = values.iter().find(|(candidate, _)| *candidate == name);
        value.expect("clap accepted a listed name").1
    })
}

fn parse_input(pair: &str) -> Result<(String, String), String> {
    let (key, value) = pair
        .split_once('=')
        .ok_or("input must be a key and a value separated by an equal sign")?;
    match key.trim() {
        "" => Err("the key was missing or empty".to_owned()),
        key => Ok((key.to_owned(), value.trim().to_owned())),
    }
}

fn parse_input_file() -> impl TypedValueParser<Value = (String, PathBuf)> {
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
