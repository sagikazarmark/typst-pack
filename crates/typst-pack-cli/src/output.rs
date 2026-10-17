//! Where compiled artifacts and dependency lists go, following Typst's CLI.

use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use typst_pack::compile::{Compiled, OutputFormat};
use typst_pack::fs::{self, WritePolicy};

use crate::cli::DepsFormat;

/// The paths the compiled artifacts go to: the output (default: the pack
/// path with the format's extension), expanded once per page for PNG and SVG.
pub fn targets(
    output: Option<&Path>,
    pack: &Path,
    format: OutputFormat,
    compiled: &Compiled,
) -> Result<Vec<PathBuf>, String> {
    let default = pack.with_extension(format.extension());
    let output = output.unwrap_or(&default);
    let targets =
        if output == Path::new("-") || matches!(format, OutputFormat::Pdf | OutputFormat::Html) {
            vec![output.to_path_buf()]
        } else {
            expand(output, compiled)?
        };
    let mut unique = HashSet::with_capacity(targets.len());
    for target in &targets {
        if !unique.insert(normalize(target)) {
            return Err(format!(
                "multiple artifacts expand to the same output path `{}`",
                target.display()
            ));
        }
    }
    Ok(targets)
}

/// Expands `{p}`, `{0p}`, `{n}`, and `{t}` once per page artifact.
fn expand(template: &Path, compiled: &Compiled) -> Result<Vec<PathBuf>, String> {
    let artifacts = &compiled.artifacts;
    let indexable = template
        .to_str()
        .filter(|text| ["{p}", "{0p}", "{n}"].iter().any(|key| text.contains(key)));
    let Some(text) = indexable else {
        return match artifacts.len() {
            0 => Ok(Vec::new()),
            1 => Ok(vec![template.to_path_buf()]),
            _ => Err(
                "cannot export multiple images without a page number template \
                      ({p}, {0p}) in the output path"
                    .to_owned(),
            ),
        };
    };
    let total = compiled.page_count.unwrap_or(artifacts.len());
    let width = total.to_string().len();
    Ok(artifacts
        .iter()
        .enumerate()
        .map(|(index, artifact)| {
            let page = artifact.page.map_or(index + 1, |page| page.get());
            PathBuf::from(
                text.replace("{p}", &page.to_string())
                    .replace("{0p}", &format!("{page:0width$}"))
                    .replace("{n}", &format!("{page:0width$}"))
                    .replace("{t}", &total.to_string()),
            )
        })
        .collect())
}

/// Removes `.` and resolvable `..` components, so aliases compare equal.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
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

/// Writes the artifacts to their targets, or the one artifact to stdout.
pub fn write(targets: &[PathBuf], compiled: &Compiled) -> Result<(), String> {
    if targets.iter().any(|target| target == Path::new("-")) {
        let [artifact] = compiled.artifacts.as_slice() else {
            return Err(
                "cannot write output to stdout unless exactly one file is emitted".to_owned(),
            );
        };
        return std::io::stdout()
            .lock()
            .write_all(&artifact.bytes)
            .map_err(|error| format!("cannot write output to stdout: {error}"));
    }
    fs::write_artifacts(&compiled.artifacts, targets, WritePolicy::Overwrite)
        .map_err(|error| error.to_string())
}

/// Serializes the inputs and outputs of a compilation in one of Typst's
/// dependency formats. A failed compilation has no outputs.
pub fn dependencies(
    format: DepsFormat,
    inputs: &BTreeSet<PathBuf>,
    outputs: Option<&[PathBuf]>,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    match format {
        DepsFormat::Json => {
            let text = |path: &PathBuf, kind: &str| {
                path.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{kind} {path:?} is not valid UTF-8"))
            };
            let inputs = inputs
                .iter()
                .map(|path| text(path, "input"))
                .collect::<Result<Vec<_>, _>>()?;
            let outputs = outputs
                .map(|outputs| {
                    outputs
                        .iter()
                        .filter(|path| path.as_path() != Path::new("-"))
                        .map(|path| text(path, "output"))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            let json = serde_json::json!({ "inputs": inputs, "outputs": outputs });
            bytes = serde_json::to_vec(&json).map_err(|error| error.to_string())?;
        }
        DepsFormat::Zero => {
            for input in inputs {
                bytes.extend_from_slice(input.as_os_str().as_encoded_bytes());
                bytes.push(0);
            }
        }
        DepsFormat::Make => {
            let Some(outputs) = outputs else {
                return Ok(bytes);
            };
            if outputs.iter().any(|output| output == Path::new("-")) {
                return Err(
                    "make dependencies contain the output path, but the output was stdout"
                        .to_owned(),
                );
            }
            for (index, output) in outputs.iter().enumerate() {
                let Some(output) = output.to_str() else {
                    continue;
                };
                if index != 0 {
                    bytes.push(b' ');
                }
                bytes.extend_from_slice(munge(output).as_bytes());
            }
            bytes.push(b':');
            for input in inputs.iter().filter_map(|input| input.to_str()) {
                bytes.push(b' ');
                bytes.extend_from_slice(munge(input).as_bytes());
            }
            bytes.push(b'\n');
        }
    }
    Ok(bytes)
}

/// Escapes a path for a Make rule, as Typst's CLI does.
fn munge(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut slashes = 0;
    for character in path.chars() {
        match character {
            '\\' => slashes += 1,
            '$' => {
                result.push('$');
                slashes = 0;
            }
            ':' | '#' => {
                result.push('\\');
                slashes = 0;
            }
            ' ' | '\t' => {
                result.extend(std::iter::repeat_n('\\', slashes + 1));
                slashes = 0;
            }
            _ => slashes = 0,
        }
        result.push(character);
    }
    result
}
