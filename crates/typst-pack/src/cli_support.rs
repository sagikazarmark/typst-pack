//! First-party support for the separately packaged command-line adapter.

use std::path::PathBuf;

use typst::diag::SourceDiagnostic;
use typst::syntax::FileId;
use typst_kit::diagnostics::DiagnosticFormat;
use typst_kit::diagnostics::termcolor::WriteColor;

use crate::compile::{
    PackCompilationExecution, PackCompilationPreparation, PackCompilationPresentation,
    compile_pack_kernel, prepare_pack_compilation,
};
use crate::fs_assembly::{PackAssemblyDiagnosticContext, PackAssemblyReport};
use crate::{
    CompilationReport, CompilationRequestRejection, CompilationResult, PackCompilationRequest,
    PdfStandardsValidationError,
};

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

/// The accepted terminal produced through the first-party timing adapter.
pub enum CliCompilationOutcome {
    Execution(CliCompilationExecution),
    Operation(CompilationReport),
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
    outcome: Option<CliCompilationOutcome>,
    timing_error: Option<String>,
}

impl TimedCliCompilation {
    pub fn into_parts(self) -> (Option<CliCompilationOutcome>, Option<String>) {
        (self.outcome, self.timing_error)
    }
}

pub fn compile_with_timing(
    request: PackCompilationRequest,
    timings: Option<PathBuf>,
) -> Result<TimedCliCompilation, CompilationRequestRejection> {
    let (mut world, kernel) = match prepare_pack_compilation(request) {
        PackCompilationPreparation::Execute { world, kernel } => (world, kernel),
        PackCompilationPreparation::Rejected(rejection) => {
            return Err(rejection);
        }
        PackCompilationPreparation::Report(report) => {
            return Ok(TimedCliCompilation {
                outcome: Some(CliCompilationOutcome::Operation(report)),
                timing_error: None,
            });
        }
    };
    let mut timer = typst_kit::timer::Timer::new_or_placeholder(timings);
    let mut execution = None;
    let timing_result = timer.record(&mut world, |world| {
        execution = Some(compile_pack_kernel(world, kernel));
    });
    let outcome = execution.map(|execution| {
        let file_dependencies = world.file_dependencies();
        CliCompilationOutcome::Execution(CliCompilationExecution {
            world,
            execution,
            file_dependencies,
        })
    });
    Ok(TimedCliCompilation {
        outcome,
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
