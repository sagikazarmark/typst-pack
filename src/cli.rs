//! The `typst-pack` command line interface.

#![cfg(feature = "cli")]

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{Datelike, Timelike};
use clap::{Args, Parser, Subcommand};
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue};
use typst::syntax::{FileId, VirtualRoot};
use typst_kit::diagnostics::termcolor::{ColorChoice, StandardStream, WriteColor};
use typst_kit::diagnostics::{DiagnosticFormat, DiagnosticWorld};
use typst_kit::files::{FileLoader, FsRoot};
use typst_pdf::{PdfStandard, PdfStandards, Timestamp};

use crate::compile::{CompileError, CompileOptions, OutputFormat, compile, parse_pages};
use crate::extract::{ExtractOptions, extract};
use crate::manifest::Metadata;
use crate::pack::{FILE_EXTENSION, Pack};
use crate::packer::{DiscoveryWorld, Packer, PackerError, ProjectResourcePolicy};
use crate::world::{PackWorld, PackWorldError, SystemPackageLoader};

/// Pack, inspect, extract, and compile portable Typst project packs.
#[derive(Debug, Parser)]
#[command(name = "typst-pack", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Packs a Typst project into a single portable file.
    Create(CreateArgs),
    /// Shows what is inside a pack.
    Inspect(InspectArgs),
    /// Extracts a pack into a directory.
    Extract(ExtractArgs),
    /// Compiles a pack to PDF, PNG, or SVG.
    Compile(CompileArgs),
}

#[derive(Debug, Args)]
struct CreateArgs {
    /// The project directory or the entrypoint .typ file.
    project: PathBuf,

    /// Path to write the pack to [default: <project name>.typk].
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// The entrypoint, relative to the project root [default: main.typ].
    #[arg(long)]
    entrypoint: Option<PathBuf>,

    /// The project root directory [default: the entrypoint's directory].
    #[arg(long)]
    root: Option<PathBuf>,

    /// Do not store package files in the pack; record them as external
    /// dependencies instead.
    #[arg(long)]
    no_packages: bool,

    /// Embed the fonts used by the document into the pack.
    #[arg(long)]
    embed_fonts: bool,

    /// When embedding fonts, also embed fonts identical to Typst's default
    /// embedded fonts.
    #[arg(long, requires = "embed_fonts")]
    include_default_fonts: bool,

    /// Additional files or directories (inside the project root) to pack
    /// beyond what the discovery compile finds.
    #[arg(long = "include", value_name = "PATH")]
    include: Vec<PathBuf>,

    /// Directories that act as roots for External Project Resources during discovery.
    #[arg(long = "resource-path", value_name = "DIR")]
    resource_paths: Vec<PathBuf>,

    /// Root-relative resource paths to declare as external even if discovery does not load them.
    #[arg(long = "external-resource", value_name = "PATH")]
    external_resources: Vec<String>,

    /// String key-value pairs visible through `sys.inputs` during the
    /// discovery compile.
    #[arg(long = "input", value_name = "KEY=VALUE")]
    inputs: Vec<String>,

    /// Additional directories to search for fonts during the discovery
    /// compile.
    #[arg(long = "font-path", value_name = "DIR")]
    font_paths: Vec<PathBuf>,

    /// Do not use system fonts during the discovery compile.
    #[arg(long)]
    ignore_system_fonts: bool,

    /// Custom path to local packages, defaults to system-dependent location.
    #[arg(long, value_name = "DIR")]
    package_path: Option<PathBuf>,

    /// Custom path to the package cache, defaults to system-dependent
    /// location.
    #[arg(long, value_name = "DIR", env = "TYPST_PACK_PACKAGE_CACHE_PATH")]
    package_cache_path: Option<PathBuf>,

    /// Disallow network access; package dependencies must already exist in
    /// the local package directories.
    #[arg(long)]
    offline: bool,

    /// A human-readable name recorded in the pack metadata.
    #[arg(long)]
    name: Option<String>,

    /// A description recorded in the pack metadata.
    #[arg(long)]
    description: Option<String>,

    /// Authors recorded in the pack metadata.
    #[arg(long = "author", value_name = "AUTHOR")]
    authors: Vec<String>,
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
    pack: PathBuf,

    /// Path to the output file. For PNG and SVG output of multi-page
    /// documents, the filename must contain `{p}`, `{0p}`, or `{t}`
    /// placeholders (page number, zero-padded page number, total pages)
    /// [default: <pack name>.<format extension>].
    output: Option<PathBuf>,

    /// The output format. `html` is experimental and additionally requires
    /// `--features html` [default: inferred from the output extension, or
    /// pdf].
    #[arg(short, long, value_parser = ["pdf", "png", "svg", "html"])]
    format: Option<String>,

    /// Enables experimental Typst features.
    #[arg(
        long = "features",
        value_name = "FEATURE",
        value_delimiter = ',',
        env = "TYPST_FEATURES",
        value_parser = ["html"]
    )]
    features: Vec<String>,

    /// Which pages to export, e.g. `1,3-5,9-`.
    #[arg(long)]
    pages: Option<String>,

    /// The PPI (pixels per inch) for PNG export.
    #[arg(long, default_value_t = 144.0)]
    ppi: f32,

    /// String key-value pairs visible through `sys.inputs`.
    #[arg(long = "input", value_name = "KEY=VALUE")]
    inputs: Vec<String>,

    /// Directories that act as roots for External Project Resources.
    #[arg(long = "resource-path", value_name = "DIR")]
    resource_paths: Vec<PathBuf>,

    /// Additional directories to search for fonts.
    #[arg(long = "font-path", value_name = "DIR")]
    font_paths: Vec<PathBuf>,

    /// Do not use system fonts.
    #[arg(long)]
    ignore_system_fonts: bool,

    /// Do not use Typst's embedded default fonts.
    #[arg(long)]
    ignore_embedded_fonts: bool,

    /// One or more PDF standards that Typst will enforce conformance with.
    #[arg(long = "pdf-standard", value_name = "STANDARD")]
    pdf_standards: Vec<String>,

    /// The document's creation date as a UNIX timestamp (for reproducible
    /// builds). Falls back to the SOURCE_DATE_EPOCH environment variable.
    #[arg(long, value_name = "UNIX_TIMESTAMP")]
    creation_timestamp: Option<i64>,

    /// Custom path to local packages, defaults to system-dependent location.
    #[arg(long, value_name = "DIR")]
    package_path: Option<PathBuf>,

    /// Custom path to the package cache, defaults to system-dependent
    /// location.
    #[arg(long, value_name = "DIR", env = "TYPST_PACK_PACKAGE_CACHE_PATH")]
    package_cache_path: Option<PathBuf>,

    /// Disallow network access; packages that are not vendored in the pack
    /// must already exist in the local package directories.
    #[arg(long)]
    offline: bool,

    /// The format to emit diagnostics in.
    #[arg(long, default_value = "human", value_parser = ["human", "short"])]
    diagnostic_format: String,

    /// Opens the output file with the default viewer after compilation.
    #[arg(long)]
    open: bool,
}

/// Runs the CLI and returns the process exit code.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Create(args) => create(args),
        Command::Inspect(args) => inspect(args),
        Command::Extract(args) => extract_command(args),
        Command::Compile(args) => compile_command(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn create(args: CreateArgs) -> Result<(), String> {
    let project = args
        .project
        .canonicalize()
        .map_err(|err| format!("cannot access `{}`: {err}", args.project.display()))?;

    let (root, entrypoint) = if project.is_dir() {
        let root = args.root.unwrap_or_else(|| project.clone());
        let entrypoint = args.entrypoint.unwrap_or_else(|| PathBuf::from("main.typ"));
        (root, entrypoint)
    } else {
        if args.entrypoint.is_some() {
            return Err("--entrypoint only applies when PROJECT is a directory".into());
        }
        let root = match args.root {
            Some(root) => root,
            None => project
                .parent()
                .ok_or("cannot determine project root")?
                .to_path_buf(),
        };
        (root, project.clone())
    };

    let output = args.output.unwrap_or_else(|| {
        let stem = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_owned());
        PathBuf::from(format!("{stem}.{FILE_EXTENSION}"))
    });

    let mut packer = Packer::new(&root, &entrypoint)
        .vendor_packages(!args.no_packages)
        .embed_fonts(args.embed_fonts)
        .include_default_fonts(args.include_default_fonts)
        .system_fonts(!args.ignore_system_fonts)
        .offline(args.offline)
        .inputs(parse_inputs(&args.inputs)?);
    for path in &args.include {
        packer = packer.include(path);
    }
    if !args.resource_paths.is_empty() {
        packer = packer.project_resource_policy(ProjectResourcePolicy::AllowExternalFallback);
    }
    for path in &args.resource_paths {
        packer = packer.external_resource_loader(ProjectResourceRoot::new(path.clone()));
    }
    for path in args.external_resources {
        packer = packer.external_resource(path);
    }
    for path in &args.font_paths {
        packer = packer.font_path(path);
    }
    if let Some(path) = &args.package_path {
        packer = packer.package_path(path);
    }
    if let Some(path) = &args.package_cache_path {
        packer = packer.package_cache_path(path);
    }
    if args.name.is_some() || args.description.is_some() || !args.authors.is_empty() {
        packer = packer.metadata(Metadata {
            name: args.name,
            description: args.description,
            authors: args.authors,
        });
    }

    let outcome = match packer.pack() {
        Ok(outcome) => outcome,
        Err(PackerError::Compile {
            world,
            errors,
            warnings,
        }) => {
            emit_diagnostics(world.as_ref(), errors.iter().chain(&warnings));
            return Err("the discovery compile failed".into());
        }
        Err(err) => return Err(err.to_string()),
    };

    emit_diagnostics(&outcome.world, outcome.report.compile_warnings.iter());
    for warning in &outcome.report.warnings {
        eprintln!("warning: {warning}");
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
    if !report.packages_external.is_empty() {
        println!(
            "note: {} package(s) were not vendored and must be available when compiling:",
            report.packages_external.len()
        );
        for spec in &report.packages_external {
            println!("  {spec}");
        }
    }
    if !report.external_resources.is_empty() {
        println!(
            "note: {} External Project Resource path(s) are declared and must be supplied if requested:",
            report.external_resources.len()
        );
        for path in &report.external_resources {
            println!("  {path}");
        }
    }
    Ok(())
}

fn inspect(args: InspectArgs) -> Result<(), String> {
    let pack = read_pack(&args.pack)?;
    let manifest = pack.manifest();

    println!("pack: {}", args.pack.display());
    println!("format version: {}", manifest.format_version);
    println!("entrypoint: {}", pack.entrypoint());
    if let Some(metadata) = &manifest.metadata {
        if let Some(name) = &metadata.name {
            println!("name: {name}");
        }
        if let Some(description) = &metadata.description {
            println!("description: {description}");
        }
        if !metadata.authors.is_empty() {
            println!("authors: {}", metadata.authors.join(", "));
        }
    }

    println!("\npacked project files:");
    for (path, data) in pack.files() {
        println!("  {path} ({})", human_size(data.len()));
    }

    if !manifest.project.external_resources.is_empty() {
        println!("\nexternal project resources:");
        for path in &manifest.project.external_resources {
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
    if !manifest.packages.external.is_empty() {
        println!("\nexternal packages (not vendored):");
        for spec in &manifest.packages.external {
            println!("  {spec}");
        }
    }

    if !pack.fonts().is_empty() {
        println!("\nembedded fonts:");
        for font in pack.fonts() {
            println!(
                "  {} ({}){}",
                font.entry.path,
                human_size(font.data.len()),
                if font.entry.families.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", font.entry.families.join(", "))
                }
            );
        }
    }

    Ok(())
}

fn extract_command(args: ExtractArgs) -> Result<(), String> {
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
    Ok(())
}

fn compile_command(args: CompileArgs) -> Result<(), String> {
    let pack = read_pack(&args.pack)?;

    let format = match &args.format {
        Some(name) => match name.as_str() {
            "pdf" => OutputFormat::Pdf,
            "png" => OutputFormat::Png,
            "svg" => OutputFormat::Svg,
            "html" => OutputFormat::Html,
            other => return Err(format!("unknown format `{other}`")),
        },
        None => match args.output.as_ref().and_then(|path| path.extension()) {
            Some(ext) if ext == "png" => OutputFormat::Png,
            Some(ext) if ext == "svg" => OutputFormat::Svg,
            Some(ext) if ext == "pdf" => OutputFormat::Pdf,
            Some(ext) if ext == "html" || ext == "htm" => OutputFormat::Html,
            Some(other) => {
                return Err(format!(
                    "cannot infer output format from extension `{}`; pass --format",
                    other.to_string_lossy()
                ));
            }
            None => OutputFormat::Pdf,
        },
    };

    let creation_timestamp = args
        .creation_timestamp
        .or_else(|| {
            std::env::var("SOURCE_DATE_EPOCH")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .map(|seconds| {
            datetime_from_timestamp(seconds)
                .ok_or_else(|| format!("timestamp {seconds} is out of range"))
        })
        .transpose()?;

    let mut builder = PackWorld::builder(pack)
        .embedded_fonts(!args.ignore_embedded_fonts)
        .inputs(parse_inputs(&args.inputs)?)
        .package_loader(system_package_loader(
            args.package_path.as_deref(),
            args.package_cache_path.as_deref(),
            args.offline,
        ));
    if args.features.iter().any(|feature| feature == "html") {
        builder = builder.feature(typst::Feature::Html);
    }
    for path in &args.resource_paths {
        builder = builder.external_resource_loader(ProjectResourceRoot::new(path.clone()));
    }
    builder = match creation_timestamp {
        Some(datetime) => builder.fixed_date(datetime),
        None => builder.system_date(),
    };
    for path in &args.font_paths {
        builder = builder.extra_fonts(typst_kit::fonts::scan(path));
    }
    if !args.ignore_system_fonts {
        builder = builder.extra_fonts(typst_kit::fonts::system());
    }

    let world = builder
        .build()
        .map_err(|err: PackWorldError| err.to_string())?;

    let mut standards = Vec::new();
    for name in &args.pdf_standards {
        standards.push(parse_pdf_standard(name)?);
    }

    let options = CompileOptions {
        pages: match &args.pages {
            Some(text) => parse_pages(text)?,
            None => Vec::new(),
        },
        ppi: Some(args.ppi),
        pdf_standards: PdfStandards::new(&standards).map_err(|err| err.message().to_string())?,
        creation_timestamp: creation_timestamp.map(Timestamp::new_utc),
    };

    let diagnostic_format = match args.diagnostic_format.as_str() {
        "short" => DiagnosticFormat::Short,
        _ => DiagnosticFormat::Human,
    };

    let output = match compile(&world, format, &options) {
        Ok(output) => {
            emit_diagnostics_with(&world, output.warnings.iter(), diagnostic_format);
            output
        }
        Err(CompileError::Diagnostics { errors, warnings }) => {
            emit_diagnostics_with(&world, errors.iter().chain(&warnings), diagnostic_format);
            return Err("compilation failed".into());
        }
        Err(err) => return Err(err.to_string()),
    };

    let stem = args
        .pack
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_owned());
    let extension = format.extension();

    let targets: Vec<PathBuf> = match &args.output {
        Some(path) => expand_output_template(path, output.outputs.len())?,
        None if output.outputs.len() == 1 => {
            vec![PathBuf::from(format!("{stem}.{extension}"))]
        }
        None => {
            let template = PathBuf::from(format!("{stem}-{{0p}}.{extension}"));
            expand_output_template(&template, output.outputs.len())?
        }
    };

    for (target, data) in targets.iter().zip(&output.outputs) {
        std::fs::write(target, data)
            .map_err(|err| format!("cannot write `{}`: {err}", target.display()))?;
    }
    println!(
        "compiled `{}` to {}",
        args.pack.display(),
        match targets.as_slice() {
            [single] => format!("`{}`", single.display()),
            many => format!("{} files", many.len()),
        }
    );

    if args.open
        && let Some(first) = targets.first()
    {
        open::that_detached(first).map_err(|err| err.to_string())?;
    }

    Ok(())
}

/// Expands `{p}`, `{0p}`, and `{t}` placeholders into one path per page.
fn expand_output_template(template: &Path, count: usize) -> Result<Vec<PathBuf>, String> {
    let text = template.to_string_lossy();
    let has_placeholder = text.contains("{p}") || text.contains("{0p}") || text.contains("{t}");
    if !has_placeholder {
        if count > 1 {
            return Err(format!(
                "the document has {count} pages; the output filename must contain \
                 `{{p}}`, `{{0p}}`, or `{{t}}` placeholders"
            ));
        }
        return Ok(vec![template.to_path_buf()]);
    }
    let width = count.to_string().len();
    Ok((1..=count)
        .map(|page| {
            PathBuf::from(
                text.replace("{p}", &page.to_string())
                    .replace("{0p}", &format!("{page:0width$}"))
                    .replace("{t}", &count.to_string()),
            )
        })
        .collect())
}

fn read_pack(path: &Path) -> Result<Pack, String> {
    let file =
        File::open(path).map_err(|err| format!("cannot open `{}`: {err}", path.display()))?;
    Pack::read(BufReader::new(file)).map_err(|err| err.to_string())
}

fn default_output_dir(pack: &Path) -> PathBuf {
    match pack.file_stem() {
        Some(stem) => PathBuf::from(stem),
        None => PathBuf::from("extracted"),
    }
}

fn parse_inputs(pairs: &[String]) -> Result<Dict, String> {
    let mut dict = Dict::new();
    for pair in pairs {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("expected KEY=VALUE, got `{pair}`"))?;
        dict.insert(key.into(), value.into_value());
    }
    Ok(dict)
}

fn system_package_loader(
    package_path: Option<&Path>,
    package_cache_path: Option<&Path>,
    offline: bool,
) -> SystemPackageLoader {
    use typst_kit::downloader::SystemDownloader;
    use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

    let data = match package_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_data(),
    };
    let cache = match package_cache_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_cache(),
    };
    let universe = if offline {
        UniversePackages::new(crate::world::OfflineDownloader)
    } else {
        UniversePackages::new(SystemDownloader::new(concat!(
            "typst-pack/",
            env!("CARGO_PKG_VERSION")
        )))
    };
    SystemPackageLoader(SystemPackages::from_parts(data, cache, universe))
}

struct ProjectResourceRoot(FsRoot);

impl ProjectResourceRoot {
    fn new(root: PathBuf) -> Self {
        Self(FsRoot::new(root))
    }
}

impl FileLoader for ProjectResourceRoot {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => self.0.load(id.vpath()),
            VirtualRoot::Package(_) => Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            ))),
        }
    }
}

fn parse_pdf_standard(name: &str) -> Result<PdfStandard, String> {
    Ok(match name {
        "1.4" => PdfStandard::V_1_4,
        "1.5" => PdfStandard::V_1_5,
        "1.6" => PdfStandard::V_1_6,
        "1.7" => PdfStandard::V_1_7,
        "2.0" => PdfStandard::V_2_0,
        "a-1b" => PdfStandard::A_1b,
        "a-1a" => PdfStandard::A_1a,
        "a-2b" => PdfStandard::A_2b,
        "a-2u" => PdfStandard::A_2u,
        "a-2a" => PdfStandard::A_2a,
        "a-3b" => PdfStandard::A_3b,
        "a-3u" => PdfStandard::A_3u,
        "a-3a" => PdfStandard::A_3a,
        "a-4" => PdfStandard::A_4,
        "a-4f" => PdfStandard::A_4f,
        "a-4e" => PdfStandard::A_4e,
        "ua-1" => PdfStandard::Ua_1,
        other => return Err(format!("unknown PDF standard `{other}`")),
    })
}

/// Converts a UNIX timestamp to a Typst datetime.
fn datetime_from_timestamp(seconds: i64) -> Option<Datetime> {
    let utc = chrono::DateTime::from_timestamp(seconds, 0)?;
    Datetime::from_ymd_hms(
        utc.year(),
        utc.month().try_into().ok()?,
        utc.day().try_into().ok()?,
        utc.hour().try_into().ok()?,
        utc.minute().try_into().ok()?,
        utc.second().try_into().ok()?,
    )
}

fn emit_diagnostics<'a>(
    world: &dyn DiagnosticWorld,
    diagnostics: impl IntoIterator<Item = &'a SourceDiagnostic>,
) {
    emit_diagnostics_with(world, diagnostics, DiagnosticFormat::Human);
}

fn emit_diagnostics_with<'a>(
    world: &dyn DiagnosticWorld,
    diagnostics: impl IntoIterator<Item = &'a SourceDiagnostic>,
    format: DiagnosticFormat,
) {
    let mut diagnostics = diagnostics.into_iter().peekable();
    if diagnostics.peek().is_none() {
        return;
    }
    let mut stream = StandardStream::stderr(ColorChoice::Auto);
    let _ = typst_kit::diagnostics::emit(&mut stream, world, diagnostics, format);
    let _ = stream.reset();
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

impl DiagnosticWorld for DiscoveryWorld {
    fn name(&self, id: FileId) -> String {
        display_file_id(id)
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
