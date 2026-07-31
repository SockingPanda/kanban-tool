//! Explicit public facade for SQLite-backed application use cases.
//!
//! `service` remains the implementation owner for transactions, state-machine
//! guards, canonical writes, events, runs, and provenance. Adapters should use
//! this curated module instead of relying on crate-root legacy re-exports or
//! the full implementation surface under `service`.

// Adapter-facing DTOs shared with the `kanban-application` vertical slice.
pub use kanban_application::dto::{
    ClaimResult, CreateLabel, CreateTask, DEFAULT_PRIORITY, DispatchOptions, DispatchResult,
    EventListOptions, EventRecord, FinishPolicy, LabelRecord, MAX_TASK_LIST_LIMIT, RunRecord,
    StepPlanState, TaskListOptions, TaskListPage, TaskListSort, TaskPatch, TaskPlanFilter,
    TaskRecord,
};

// Selected SQLite-backed use cases, records, and operator DTOs used by adapters.
/// Lifecycle and runtime guards used by binaries that own process lifetime.
pub mod lifecycle {
    pub use crate::service::{
        DatabaseReplaceGuard, DatabaseRuntimeGuard, begin_database_replace, begin_database_runtime,
    };
}

/// Provider/vector-store seams for adapters and tests that inject derived-store implementations.
pub mod provider {
    pub use crate::service::{
        DisabledLabelProposalProvider, LabelOntologyTrustedValidationInput, LabelProposalAttempt,
        LabelProposalCandidate, LabelProposalCreateOptions, LabelProposalProposeOptions,
        LabelProposalProvider, LabelSuggestionOptions, LabelSuggestionResult,
        ManualLabelProposalProvider, ProjectionArtifactEvidence, ProjectionArtifactManifest,
        ProjectionBatch, ProjectionBatchReceipt, ProjectionCorpusMetadata, ProjectionDelivery,
        ProjectionDestructiveAuthority, ProjectionGenerationBinding, ProjectionGenerationRole,
        ProjectionPublishReceipt, ProjectionSnapshot, ProjectionSnapshotRecord,
        ProjectionStoreBackend, ProjectionStoreDescriptor, begin_projection_generation,
        build_context_pack_with_vector_store, label_atom_index_status_with,
        prepare_projection_snapshot_with, propose_task_label_with,
        propose_task_label_with_create_options, propose_task_label_with_store,
        propose_task_label_with_store_and_create_options, publish_projection_generation_with,
        query_label_atom_index_by_vector_with, query_label_atom_index_with,
        rebuild_label_atom_index_with, rebuild_vector_store_with,
        reconcile_projection_generation_with, recover_projection_generation_with,
        run_projection_batch_with, suggest_task_labels_with, sync_vector_store_with,
        validate_label_ontology_action_with_trusted_suggestions, vector_store_status_with,
    };
}

pub use crate::service::{
    AddDependencyOutcome, AddTaskLabelsResult, BackupResult, BlockedReasonCount, BoardColumnRecord,
    BoardListOptions, BoardRecord, BoardTaskMapOptions, BoardTaskMapRecord, BootstrapTaskLabel,
    BootstrapTaskLabelResult, BootstrapTaskLabelVerification, BootstrapTaskLabelVerifiedResult,
    CheckpointResult, CommentRecord, CompletionCandidateKind, CreateBoard, CreateComment,
    CreateStepInput, DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT, DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT,
    DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS, DEFAULT_LABEL_SUGGESTION_MIN_SCORE,
    DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT, DeleteLabelResult, DependencyEdgeRecord,
    DependencyMutation, DependencySnapshot, DependencyTaskRecord, DerivedStoreStatusRecord,
    DoctorDerivedStoreReport, DoctorIssue, DoctorReport, EntityListOptions, EntityRecord,
    ExportResult, ImportResult, IndexOutboxRecord, LabelAtomExplainAction, LabelAtomExplainRecord,
    LabelAtomExplainSignal, LabelAtomExplainValidation, LabelAtomRecord,
    LabelOntologyActionAtomEffectRecord, LabelOntologyActionInput, LabelOntologyActionRecord,
    LabelOntologyActionType, LabelOntologyActor, LabelOntologyAtomApplyInput,
    LabelOntologyCandidateAtomInput, LabelOntologyObservationRecord,
    LabelOntologyPrecisionRecallAvailability, LabelOntologyProposedAction,
    LabelOntologyQualityDenominator, LabelOntologyQualityDisagreement, LabelOntologyQualityOptions,
    LabelOntologyQualityRates, LabelOntologyQualityReport, LabelOntologyRecordInput,
    LabelOntologyRetargetOptions, LabelOntologyRevertInput, LabelOntologyReviewAtomVariant,
    LabelOntologyReviewGroup, LabelOntologyReviewGroupBy, LabelOntologyReviewLabelRef,
    LabelOntologyReviewOptions, LabelOntologySignalDetail, LabelOntologySignalInput,
    LabelOntologySignalKind, LabelOntologySignalListOptions, LabelOntologySignalRecord,
    LabelOntologySignalStatus, LabelOntologySuggestState, LabelOntologyValidationEffectiveOutcome,
    LabelOntologyValidationInput, LabelOntologyValidationRequirement,
    LabelOntologyValidationStatus, LabelProposalAttempt, LabelProposalCandidate,
    LabelProposalCreateOptions, LabelProposalDecisionOptions, LabelProposalListOptions,
    LabelProposalProposeOptions, LabelProposalStatus, LabelSemanticProposalRecord,
    LabelSemanticsMutationOptions, LabelSemanticsRecord, LabelSuggestionCandidate,
    LabelSuggestionEvidenceAtom, LabelSuggestionOptions, LabelSuggestionResult, MAX_SEARCH_LIMIT,
    MaintenanceLegacyCleanupAction, MaintenanceLegacyCleanupReport, MaintenanceLegacyCleanupRoot,
    MaintenanceMode, MaintenanceRebuildIntent, MaintenanceResult, MaintenanceRunOptions,
    MaintenanceRunReport,
    MaintenanceSession, MaintenanceStoreFailureKind, MaintenanceStoreResult, MaintenanceStoreRun,
    OutboxListOptions, PRIORITY_ERROR, PROJECTION_PROTOCOL_VERSION, ProjectionLease,
    ProjectionMaintenanceOwnerStatus, ProjectionRuntimeAvailability, ProjectionStatus,
    ProjectionStoreStatus, QueueStats, RunLogPathStatus, SearchIndexState, SelectedLabelSuggestion,
    SignalCommentInput, SignalLifecycle, SignalListOptions, SignalObservationRecord, SignalRecord,
    SignalRecordInput, SignalRecordResult, SignalReviewInput, SignalStatus, StaleClaimRecord,
    StatusCount, StepStatus, TaskExecutionPlanRecord, TaskGraphEdgeKind, TaskGraphEdgeRecord,
    TaskGraphMeta, TaskGraphNodeRecord, TaskGraphNodeRole, TaskNeighborhoodOptions,
    TaskNeighborhoodRecord, TaskOntologySignalSummary, TaskOntologySummary, TaskStepRecord,
    UpdateStepInput, UpsertLabelSemantics, abort_projection_generation, accept_label_proposal,
    accept_label_proposal_with_options, acquire_projection_lease, add_dependency,
    add_dependency_with_outcome, add_task_label, add_task_label_by_id, add_task_labels,
    add_task_labels_by_id, add_task_labels_by_id_with_options, add_task_labels_with_options,
    apply_label_ontology_atom, apply_label_ontology_atom_with_options, archive_board, archive_task,
    backup_database, block_task, board_task_map, bootstrap_task_label, bootstrap_task_label_by_id,
    bootstrap_task_label_with_staged_verification, build_context_pack, checkpoint_database,
    claim_task, claim_task_with_profile, claim_task_with_profile_and_metadata,
    clear_label_semantics_by_id_with_options, clear_label_semantics_with_options, complete_step,
    complete_task, complete_task_with_summary, complete_task_with_summary_and_result,
    completion_candidates, create_board, create_comment, create_comment_with_options, create_label,
    create_label_ontology_action, create_label_with_actor, create_step, create_task,
    create_task_with_dependencies, create_task_with_labels,
    create_task_with_labels_and_dependencies, default_priority, delete_label, dependency_edge,
    dependency_snapshot, derived_store_statuses, dispatch_once, doctor_database, execution_plan,
    explain_label_atom, export_jsonl, export_jsonl_to_writer, get_board,
    get_board_including_archived, get_entity, get_label_ontology_signal, get_label_proposal,
    get_label_semantics, get_label_semantics_by_id, get_run_by_id_global, get_signal,
    get_signal_by_id, get_task, get_task_by_id_global, graph_neighbors, graph_store_status,
    heartbeat_task, heartbeat_task_with_note, import_jsonl, label_atom_index_status,
    label_ontology_quality_report, list_board_columns, list_boards, list_comments,
    list_dependencies, list_entities, list_events, list_events_after, list_label_atoms,
    list_label_ontology_signals, list_label_proposals, list_label_semantics, list_labels,
    list_outbox, list_runs, list_signals, list_steps, list_task_labels, list_tasks,
    list_tasks_page, maintenance_apply_legacy_projection_cleanup,
    maintenance_continuous_capability_complete, maintenance_inventory_legacy_projections,
    maintenance_plan_rebuild_all, maintenance_plan_rebuild_store, maintenance_rebuild_all,
    maintenance_rebuild_store, maintenance_restore_legacy_projection_cleanup,
    maintenance_resume_rebuild_store, maintenance_run_once, maintenance_status,
    maintenance_verify_legacy_projection_cleanup, mark_execution_plan_not_required,
    naturalize_structured_metadata, normalize_legacy_priority, projection_status, promote_task,
    propose_task_label, queue_stats, rebuild_graph_store, rebuild_search_index,
    rebuild_vector_store, reclaim_expired, reclaim_task, reclaim_task_to,
    record_label_ontology_observation, record_signal, reject_label_proposal,
    release_projection_lease, remove_dependency, remove_step, remove_task_label,
    remove_task_label_by_id, renew_projection_lease, reopen_step, reopen_task,
    resolve_run_log_path, revert_label_ontology_mutation, review_label_ontology, review_signals,
    run_log_path_status, search_index_status, search_tasks, set_task_retry_policy_by_id, skip_step,
    specify_task, submit_review_task, submit_review_task_with_summary, suggest_task_labels,
    sync_graph_store, sync_search_index, sync_vector_store, task_neighborhood,
    task_ontology_summary, task_ontology_summary_by_id_global, unblock_task, update_signal_status,
    update_step, update_task, update_task_by_id, upsert_label_semantics,
    upsert_label_semantics_by_id, upsert_label_semantics_by_id_with_options,
    upsert_label_semantics_with_options, vacuum_database, validate_label_ontology_action,
    validate_priority, vector_store_status,
};
