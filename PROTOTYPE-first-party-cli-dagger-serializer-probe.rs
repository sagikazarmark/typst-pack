//! Compile-only serializer reachability probe for the issue-69 adapter contract.

#![allow(dead_code)]

extern crate typst_pack_interface as tp;

fn creation_sources(
    limits: &tp::creation::CreationResourceLimits,
    request: &tp::creation::CreationOperationRequest,
) {
    let spec = limits.spec();
    let _ = (
        spec.package_files,
        spec.largest_package_file_bytes,
        spec.font_candidates,
        request.requested_ready_jobs,
        request.requested_queue,
        request.requested_workers,
        request.placement,
        request.interruption,
    );
}

fn compilation_sources(
    limits: &tp::compilation::CompilationResourceLimits,
    request: &tp::compilation::CompilationOperationRequest,
) {
    let spec = limits.spec();
    let _ = (
        spec.dependencies,
        spec.aggregate_artifact_bytes,
        spec.diagnostic_projection_entries,
        request.requested_ready_jobs,
        request.requested_queue,
        request.requested_workers,
        request.placement,
        request.interruption,
    );
}

fn inspect_engine_width(admission: tp::EngineWidthAdmission) {
    match admission {
        tp::EngineWidthAdmission::InheritedUnmanaged => {}
        tp::EngineWidthAdmission::Exact {
            requested,
            admitted,
            constraints,
        } => {
            // issue69-source: compilation.operational_inventory.role_execution.engine_width.admitted
            let _ = (requested, admitted, constraints);
        }
    }
}

fn creation_report_sources(report: &tp::creation::CreationReport) {
    let inventory = report.operational_inventory();
    let admission = inventory.admission();
    let resources = inventory.resources();
    let dependencies = inventory.dependency_execution();
    let attempt = inventory.attempt_control();
    let reporting = inventory.reporting();
    let _ = (
        admission.admitted_network,
        resources.admitted,
        dependencies.evidence.class(),
        dependencies.packages.class(),
        dependencies.fonts.class(),
        attempt.admitted_interruption,
        reporting.fine_engine_timing,
    );
    match inventory.role_execution() {
        tp::creation::CreationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
        } => {
            let _ = domain;
            inspect_engine_width(engine_width);
        }
        tp::creation::CreationExecutionInventoryView::Facility {
            descriptor,
            domain,
            engine_width,
            capacity,
            queue_reached,
            dispatch_reached,
            worker_terminated,
            worker_reaped,
        } => {
            let _ = (
                descriptor.class(),
                domain,
                capacity.admitted_workers,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
            );
            inspect_engine_width(engine_width);
        }
    }
}

fn compilation_report_sources(report: &tp::compilation::CompilationReport) {
    let inventory = report.operational_inventory();
    let admission = inventory.admission();
    // issue69-source: compilation.operational_inventory.admission.admitted_network
    let _ = admission.admitted_network;
    let resources = inventory.resources();
    // issue69-source: compilation.operational_inventory.resources.admitted
    let _ = resources.admitted;
    let dependencies = inventory.dependency_execution();
    let _ = (
        dependencies.packages.class(),
        dependencies.fonts.class(),
        dependencies.cache_descriptor,
        dependencies.cache_policy,
        dependencies.cache_lookup,
        dependencies.cache_isolation_domain_present,
    );
    let attempt = inventory.attempt_control();
    // issue69-source: compilation.operational_inventory.attempt_control.admitted_interruption
    let _ = attempt.admitted_interruption;
    let reporting = inventory.reporting();
    // issue69-source: compilation.operational_inventory.reporting.fine_engine_timing
    let _ = reporting.fine_engine_timing;
    match inventory.role_execution() {
        tp::compilation::CompilationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
        } => {
            let _ = domain;
            inspect_engine_width(engine_width);
        }
        tp::compilation::CompilationExecutionInventoryView::Facility {
            descriptor,
            domain,
            engine_width,
            capacity,
            queue_reached,
            dispatch_reached,
            worker_terminated,
            worker_reaped,
        } => {
            // issue69-source: compilation.operational_inventory.role_execution.capacity.P
            let _ = (
                descriptor.class(),
                domain,
                capacity.admitted_workers,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
            );
            inspect_engine_width(engine_width);
        }
    }
}

fn format_sources(
    archive: &tp::representation::PackArchiveReadFormatReceipt,
    materialization: &tp::representation::ProjectMaterializationProjectionReceipt,
) {
    let common = archive.common();
    let _ = (
        common.role(),
        common.terminal(),
        common.stage(),
        common.counters(),
        common.timing(),
        common.admission(),
        common.publication(),
        common.cleanup(),
        archive.asserted_archive_encoding_identity(),
        archive.encoding_assertion(),
    );

    let _ = (
        materialization.common().role(),
        materialization.pack_identity(),
        materialization.file_count(),
        materialization.aggregate_bytes(),
        materialization.files().count(),
    );
}

fn transport_sources(receipt: &tp::transport::SpoolTransportReceipt) {
    match receipt.state() {
        tp::transport::SpoolTransportReceiptStateView::Refused(refusal) => {
            // issue69-source: transport.receipt.refused
            let _ = (
                refusal.common.requested_trust,
                refusal.common.resource_profile,
                refusal.common.requested_concurrency,
                refusal.common.reason,
                refusal.descriptor.descriptor_version(),
            );
        }
        tp::transport::SpoolTransportReceiptStateView::Admitted {
            admission,
            stage_ledger,
        } => {
            // issue69-source: transport.receipt.stage_ledger.stages
            // issue69-source: transport.receipt.stage_ledger.actual_commit
            let _ = (
                admission.common.admitted_trust,
                admission.common.resource_profile,
                admission.common.admitted_concurrency,
                stage_ledger.stages().count(),
                stage_ledger.primary_terminal_stage(),
                stage_ledger.actual_commit_strength(),
                stage_ledger.timing().status,
            );
        }
    }
}

fn archive_encoding_report_sources(report: &tp::representation::PackArchiveEncodingReport) {
    let format_receipt = report.receipt();
    let _ = (
        format_receipt.common().role(),
        format_receipt.source_pack_identity(),
        format_receipt.archive_encoding_identity(),
        format_receipt.output_archive_identity(),
    );
    if let Some(spool_receipt) = report.spool_receipt() {
        transport_sources(spool_receipt);
    }
}

fn session_sources(view: tp::session::SessionView<'_>) {
    let _ = (
        view.session_instance(),
        view.lifecycle(),
        view.latest_revision(),
        view.latest_evaluation(),
        view.active_attempt(),
        view.pending_revision(),
    );
    if let Some(publication) = view.publication() {
        let _ = (
            publication.sequence(),
            publication.revision(),
            publication.evaluation(),
            publication.terminal(),
            publication.currentness(),
        );
        match publication.terminal() {
            tp::session::SessionPublicationTerminalRef::RequestRejected(rejection) => {
                // issue69-source: session.publication.request_rejected
                let _ = rejection.issues().count();
            }
            tp::session::SessionPublicationTerminalRef::Report(report) => {
                compilation_report_sources(report);
            }
            tp::session::SessionPublicationTerminalRef::IngestionFailure(failure) => {
                // issue69-source: session.publication.ingestion_failure
                let _ = (
                    failure.safe_code(),
                    failure.failed_request_sources().count(),
                );
            }
        }
    }
    if let Some(last) = view.last_successful() {
        let _ = (
            last.revision,
            last.evaluation,
            last.publication_sequence,
            last.result,
            last.currentness,
        );
    }
}

fn archive_recipe_source() {
    let recipe = tp::representation::ArchiveEncodingIdentity::epoch_2_all_stored_v1();
    let _ = recipe.as_str();
}

fn main() {}
