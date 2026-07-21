//! Compile-only serializer reachability probe for the final issue-78 contract.

#![allow(dead_code)]

extern crate typst_pack_interface as tp;

fn creation_sources(
    limits: &tp::creation::CreationResourceLimits,
    request: &tp::creation::CreationOperationRequest,
) {
    let spec = limits.spec();
    // final-source: creation.request.font_scan_policy
    // final-source: creation.resources.aggregate
    // final-source: operation.capability_scopes
    let _ = (
        spec.package_files,
        spec.largest_package_file_bytes,
        spec.font_candidates,
        spec.aggregate_file_bindings,
        spec.aggregate_logical_bytes,
        request.requested_ready_jobs,
        request.requested_queue,
        request.requested_workers,
        &request.font_scan_policy,
        request.required_capabilities.requested_scopes(),
        request.requested_execution_placement,
        &request.requested_isolation,
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
        request.required_capabilities.requested_scopes(),
        request.requested_execution_placement,
        &request.requested_isolation,
        request.interruption,
    );
}

fn admission_refusal_sources(
    creation: &tp::creation::CreationAdmissionRefusal,
    compilation: &tp::compilation::CompilationAdmissionRefusal,
) {
    // final-source: creation.admission_refusal.stage
    // final-source: compilation.admission_refusal.stage
    let _ = (creation.stage(), compilation.stage());
}

fn inspect_engine_width(admission: tp::EngineWidthAdmission) {
    match admission {
        tp::EngineWidthAdmission::InheritedUnmanaged => {}
        tp::EngineWidthAdmission::Exact {
            requested,
            admitted,
            constraints,
        } => {
            // final-source: compilation.operational_inventory.role_execution.engine_width.admitted
            let _ = (requested, admitted, constraints);
        }
    }
}

fn inspect_domain_selection(selection: tp::compilation::EngineRuntimeDomainSelectionView<'_>) {
    match selection {
        // final-source: compilation.domain.not_selected
        tp::compilation::EngineRuntimeDomainSelectionView::NotSelected => {}
        tp::compilation::EngineRuntimeDomainSelectionView::InheritedUnmanaged => {}
        tp::compilation::EngineRuntimeDomainSelectionView::Managed {
            identity,
            width,
            fine_timing_lease_reached,
        } => {
            let _ = (identity, width, fine_timing_lease_reached);
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
        resources.reached.aggregate_file_bindings,
        resources.reached.aggregate_logical_bytes,
        dependencies.font_scan_policy.requested,
        dependencies.font_scan_policy.admitted,
        dependencies.reached_evidence_scope,
        admission.requested_capability_scopes,
        admission.admitted_capability_scopes,
        admission.requested_execution_placement,
        admission.admitted_execution_placement,
        admission.requested_isolation,
        admission.admitted_isolation,
    );
    match inventory.role_execution() {
        tp::creation::CreationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
            reached_placement,
            reached_isolation,
        } => {
            inspect_domain_selection(domain);
            let _ = (reached_placement, reached_isolation);
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
            reached_scope,
            reached_placement,
            reached_isolation,
        } => {
            let _ = (
                descriptor.class(),
                domain,
                capacity.admitted_workers,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
                reached_scope,
                reached_placement,
                reached_isolation,
            );
            inspect_engine_width(engine_width);
        }
    }
}

fn compilation_report_sources(report: &tp::compilation::CompilationReport) {
    let inventory = report.operational_inventory();
    let admission = inventory.admission();
    // final-source: compilation.operational_inventory.admission.admitted_network
    let _ = admission.admitted_network;
    let resources = inventory.resources();
    // final-source: compilation.operational_inventory.resources.admitted
    let _ = resources.admitted;
    let dependencies = inventory.dependency_execution();
    let _ = (
        dependencies.packages.class(),
        dependencies.fonts.class(),
        dependencies.cache_descriptor,
        dependencies.cache_policy,
        dependencies.cache_lookup,
        dependencies.cache_isolation_domain_present,
        dependencies.reached_package_scope,
        dependencies.reached_font_scope,
        dependencies.reached_cache_scope,
        admission.requested_capability_scopes,
        admission.admitted_capability_scopes,
        admission.requested_execution_placement,
        admission.admitted_execution_placement,
        admission.requested_isolation,
        admission.admitted_isolation,
    );
    let attempt = inventory.attempt_control();
    // final-source: compilation.operational_inventory.attempt_control.admitted_interruption
    let _ = attempt.admitted_interruption;
    let reporting = inventory.reporting();
    // final-source: compilation.operational_inventory.reporting.fine_engine_timing
    let _ = reporting.fine_engine_timing;
    match inventory.role_execution() {
        tp::compilation::CompilationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
            reached_placement,
            reached_isolation,
        } => {
            inspect_domain_selection(domain);
            let _ = (reached_placement, reached_isolation);
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
            reached_scope,
            reached_placement,
            reached_isolation,
        } => {
            // final-source: compilation.operational_inventory.role_execution.capacity.P
            let _ = (
                descriptor.class(),
                domain,
                capacity.admitted_workers,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
                reached_scope,
                reached_placement,
                reached_isolation,
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
        common.accounting(),
        common.timing(),
        common.representation_admission(),
        common.publication(),
        common.cleanup_status(),
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

fn representation_limit_sources(
    archive: &tp::representation::PackArchiveEncodingResourceLimits,
    closure: &tp::representation::ClosureExportResourceLimits,
    materialization: &tp::representation::ProjectMaterializationResourceLimits,
) {
    // final-source: representation.limits.role_specific
    let _ = (
        archive.logical_file_bindings,
        archive.logical_decoded_bytes,
        archive.control_record_bytes,
        archive.physical_blob_bytes,
        archive.largest_physical_blob_bytes,
        archive.representation_entries,
        archive.maximum_expansion_ratio,
        archive.output_bytes,
        closure.logical_file_bindings,
        closure.payload_bytes,
        materialization.files,
        materialization.output_bytes,
    );
}

fn transport_sources(receipt: &tp::transport::SpoolTransportReceipt) {
    match receipt.state() {
        tp::transport::SpoolTransportReceiptStateView::Refused(refusal) => {
            // final-source: transport.receipt.refused
            // final-source: transport.receipt.refusal.stage
            // final-source: transport.capability_scope
            let _ = (
                refusal.common.requested_trust,
                refusal.common.stage,
                refusal.common.resource_profile,
                refusal.common.requested_concurrency,
                refusal.common.reason,
                refusal.common.requested_scope,
                refusal.descriptor.offered_scope(),
                refusal.descriptor.descriptor_version(),
            );
        }
        tp::transport::SpoolTransportReceiptStateView::Admitted {
            admission,
            stage_ledger,
        } => {
            // final-source: transport.receipt.stage_ledger.stages
            // final-source: transport.receipt.stage_ledger.actual_commit
            // final-source: transport.receipt.stage_ledger.object_count
            // final-source: transport.cleanup_outcome
            let _ = (
                admission.common.admitted_trust,
                admission.common.resource_profile,
                admission.common.admitted_concurrency,
                stage_ledger.stages().count(),
                stage_ledger.object_count(),
                stage_ledger.primary_terminal_stage(),
                stage_ledger.actual_commit_strength(),
                stage_ledger.cleanup_outcome(),
                admission.common.requested_scope,
                admission.common.admitted_scope,
                stage_ledger.reached_scope(),
                stage_ledger.timing().status,
            );
        }
    }
}

fn spool_occupancy_sources(controls: &tp::transport::SpoolControls<'_>) {
    // final-source: representation.occupancy.shared_spool
    let _reservation = controls.reserve_occupancy(tp::transport::SpoolBackingClass::Memory, 8, 8);
    let reached = controls.occupancy();
    let _ = (
        reached.live_stable_spool_bytes,
        reached.live_retained_memory_bytes,
        reached.peak_stable_spool_bytes,
        reached.peak_retained_memory_bytes,
    );
}

fn spool_ownership_transfer_source(
    reservation: tp::transport::SpoolOccupancyReservation,
    value: tp::StableByteValue,
) -> tp::StableByteValue {
    reservation.transfer_to_stable_value(value)
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

fn publication_sources(
    archive: &tp::transport::PackArchivePublicationOutcome,
    closure: &tp::transport::ClosureExportPublicationOutcome,
) {
    // final-source: publication.format.transport_refusal.reason
    // final-source: transport.cleanup_requirement
    match archive.format().publication_admission() {
        tp::representation::PublicationFormatAdmissionDispositionView::Refused { transport } => {
            let _ = (transport.stage, transport.reason);
        }
        tp::representation::PublicationFormatAdmissionDispositionView::Admitted { transport } => {
            let _ = (
                transport.requested_cleanup_requirement,
                transport.admitted_cleanup_requirement,
            );
        }
    }
    let _ = (
        archive.format().source_pack_identity(),
        archive.format().source_archive_identity(),
        archive.format().archive_encoding_identity(),
        archive.format().source_tree_identity(),
        archive.format().entries(),
    );
    match closure.format().publication_admission() {
        tp::representation::PublicationFormatAdmissionDispositionView::Refused { transport } => {
            let _ = (transport.stage, transport.reason);
        }
        tp::representation::PublicationFormatAdmissionDispositionView::Admitted { transport } => {
            let _ = transport.admitted_cleanup_requirement;
        }
    }
}

fn session_event_sources(event: &tp::session::SessionEvent) {
    // final-source: session.attempt_admission_refused
    // final-source: compilation.admission.prepared_identity
    if let tp::session::SessionEvent::AttemptAdmissionRefused { token, refusal } = event {
        let _ = (
            token.session_instance(),
            token.evaluation(),
            refusal.stage(),
            refusal.prepared(),
            refusal.compilation_identity(),
        );
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
                // final-source: session.publication.request_rejected
                let _ = rejection.issues().count();
            }
            tp::session::SessionPublicationTerminalRef::Report(report) => {
                compilation_report_sources(report);
            }
            tp::session::SessionPublicationTerminalRef::IngestionFailure(failure) => {
                // final-source: session.publication.ingestion_failure
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
