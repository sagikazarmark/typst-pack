//! First-party support for the separately packaged command-line adapter.

use std::path::PathBuf;

use typst::diag::SourceDiagnostic;
use typst::syntax::FileId;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::WriteColor;

use crate::compile::{
    PackCompilationExecution, PackCompilationPresentation, compile_pack_kernel,
    prepare_pack_compilation,
};
use crate::fs_assembly::{PackAssemblyDiagnosticContext, PackAssemblyReport};
use crate::{CompilationResult, PackCompilationRequest, PdfStandardsValidationError};

pub use crate::FilesystemPackageAuthority;

/// The process-facing outcome of an embedded compilation.
pub enum CliCompilationPresentation<'a> {
    Succeeded,
    Diagnostics,
    PngExport { error: &'a str },
}

/// An opaque completed compilation with its diagnostic source context.
pub struct CliCompilationExecution {
    world: crate::world::PackWorld,
    execution: PackCompilationExecution,
    file_dependencies: Vec<FileId>,
}

impl CliCompilationExecution {
    pub fn presentation(&self) -> CliCompilationPresentation<'_> {
        match &self.execution.presentation {
            PackCompilationPresentation::Succeeded { .. } => CliCompilationPresentation::Succeeded,
            PackCompilationPresentation::Diagnostics { .. } => {
                CliCompilationPresentation::Diagnostics
            }
            PackCompilationPresentation::PngExport { error, .. } => {
                CliCompilationPresentation::PngExport { error }
            }
        }
    }

    pub fn result(&self) -> &CompilationResult {
        &self.execution.result
    }

    pub fn file_dependencies(&self) -> &[FileId] {
        &self.file_dependencies
    }

    pub fn emit_diagnostics(&self, output: &mut impl WriteColor, format: DiagnosticFormat) {
        let diagnostics: Box<dyn Iterator<Item = &SourceDiagnostic> + '_> =
            match &self.execution.presentation {
                PackCompilationPresentation::Succeeded {
                    warnings,
                    pack_warnings,
                } => Box::new(warnings.iter().chain(pack_warnings)),
                PackCompilationPresentation::Diagnostics {
                    errors,
                    warnings,
                    pack_warnings,
                } => Box::new(warnings.iter().chain(pack_warnings).chain(errors)),
                PackCompilationPresentation::PngExport {
                    warnings,
                    pack_warnings,
                    ..
                } => Box::new(warnings.iter().chain(pack_warnings)),
            };
        let _ = typst_kit::diagnostics::emit(output, &self.world, diagnostics, format);
    }
}

/// A timed operation may fail to execute if the timing adapter cannot start.
pub struct TimedCliCompilation {
    execution: Option<CliCompilationExecution>,
    timing_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CliCompilationError(String);

impl TimedCliCompilation {
    pub fn into_parts(self) -> (Option<CliCompilationExecution>, Option<String>) {
        (self.execution, self.timing_error)
    }
}

pub fn compile_with_timing(
    request: PackCompilationRequest,
    timings: Option<PathBuf>,
) -> Result<TimedCliCompilation, CliCompilationError> {
    let (mut world, kernel) = prepare_pack_compilation(request)
        .map_err(|error| CliCompilationError(error.to_string()))?;
    let mut timer = typst_kit::timer::Timer::new_or_placeholder(timings);
    let mut execution = None;
    let timing_result = timer.record(&mut world, |world| {
        execution = Some(compile_pack_kernel(world, kernel));
    });
    let execution = execution.map(|execution| {
        let file_dependencies = world.file_dependencies();
        CliCompilationExecution {
            world,
            execution,
            file_dependencies,
        }
    });
    Ok(TimedCliCompilation {
        execution,
        timing_error: timing_result.err().map(|error| error.to_string()),
    })
}

pub fn emit_creation_error_diagnostics<'a>(
    context: &PackAssemblyDiagnosticContext,
    diagnostics: impl IntoIterator<Item = &'a SourceDiagnostic>,
    output: &mut impl WriteColor,
    format: DiagnosticFormat,
) {
    let _ = typst_kit::diagnostics::emit(output, &context.world, diagnostics, format);
}

pub fn emit_creation_warnings(
    report: &PackAssemblyReport,
    output: &mut impl WriteColor,
    format: DiagnosticFormat,
) {
    let _ = typst_kit::diagnostics::emit(output, &report.world, report.warnings().iter(), format);
}

pub fn validate_pdf_standards(
    standards: &[typst_pdf::PdfStandard],
) -> Result<typst_pdf::PdfStandards, PdfStandardsValidationError> {
    crate::compile::validate_pdf_standards(standards)
}

pub fn pdf_standard_requiring_tags(standards: &[typst_pdf::PdfStandard]) -> Option<&'static str> {
    crate::compile::pdf_standard_requiring_tags(standards)
}
