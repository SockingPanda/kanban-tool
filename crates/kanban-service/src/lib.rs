//! Shared application-service and canonical Turso boundary for every kanban-tool adapter.
//!
//! The HTTP host constructs one [`ApplicationService`] over the canonical
//! store. CLI, MCP and Desktop never construct this service and never receive a
//! storage handle; they reach it through the localhost API.

mod adapter;
mod db;
mod domain;
mod error;
mod maintenance;
mod migration;
mod schema;
mod shared;
mod store_operations;

#[cfg(test)]
mod test_support;

pub mod dto;
pub mod operations;
pub mod ports;
pub mod service;
pub mod vector;

#[cfg(feature = "legacy-sqlite-import")]
pub mod legacy_import;

// Entity/relation/graph ports are re-exported below so every host adapter uses
// the same canonical application boundary, including host-admin maintenance.

pub use adapter::TursoApplicationStore;
pub use dto::*;
pub use kanban_core::{Board, BoardColumn, KanbanError, Result, TaskStatus, new_task_id};
pub use operations::LabelOntologyOperations;
pub use operations::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, AddTaskLabelsRecordInput, ArchiveBoardCommand,
    ArchiveBoardRecord, ArchiveTaskCommand, ArchiveTaskRecord, AttachmentCreate, AttachmentDelete,
    AttachmentList, AttachmentRead, BackupReportRecord, BoardArchive, BoardColumns, BoardCreate,
    BoardGet, BoardLabelCreate, BoardLabelList, BoardList, BoardTaskMapOptions,
    CheckpointReportRecord, CommentCreate, CommentList, ContextBuild, ContextBuildOptions,
    ContextCandidate, ContextDiagnostic, ContextEvidence, ContextItem, ContextPack, ContextPolicy,
    ContextProviderStatus, ContextSources, CreateAttachmentCommand, CreateAttachmentRecord,
    CreateBoardCommand, CreateBoardLabelCommand, CreateBoardRecord, CreateLabelRecord,
    DeleteAttachmentCommand, DependencyCreate, DependencyList, DependencyRemove,
    DoctorDerivedStoreRecord, DoctorIssueRecord, DoctorReportRecord, EntityListOptions,
    EntityQuery, EntityUpsertCommand, EventList, EventListOptions, EventListPage, EventRecord,
    ExportReportRecord, GraphNeighborsOptions, GraphQuery, GraphQueryOptions, ImportReportRecord,
    LegacyImportOptionsRecord, LegacyImportResultRecord, LegacyImportTableCountRecord,
    MAX_CONTEXT_BUDGET, MAX_CONTEXT_DEPTH, MAX_CONTEXT_LIMIT, MAX_SEARCH_LIMIT,
    MaintenanceOwnerRecord, MaintenanceQuery, MaintenanceRunRecord, MaintenanceStatusRecord,
    ProjectionStatusOptions, ProjectionStatusRecord, RUN_LOG_TAIL_BYTES, ReclaimTaskCommand,
    ReclaimTaskRecord, RelationDeleteCommand, RelationListOptions, RelationPredicateCommand,
    RelationQuery, RelationUpsertCommand, RemoveTaskLabelCommand, RemoveTaskLabelRecord,
    ReopenTaskCommand, ReopenTaskRecord, RunList, RunLog, RunLogRecord, RunShow, SearchHit,
    SearchIndexStatus, SearchMeta, SearchQuery, SearchResults, SearchTasks, SpecifyTaskCommand,
    SpecifyTaskRecord, StepComplete, StepCreate, StepList, StepRemove, StepReopen, StepSkip,
    StepUpdate, TaskArchive, TaskBlock, TaskClaim, TaskCreate, TaskDetailOntologyRecord,
    TaskDetailRead, TaskDetailRecord, TaskDone, TaskHeartbeat, TaskLabelAdd, TaskLabelList,
    TaskLabelRemove, TaskList, TaskNeighborhoodOptions, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord, TaskPlanNotRequired, TaskPromote, TaskReclaim, TaskReclaimExplicit,
    TaskRelease, TaskReopen, TaskReview, TaskShow, TaskSpecify, TaskUnblock, TaskUpdate,
    UnblockTaskCommand, UnblockTaskRecord, UpdateTaskCommand, UpdateTaskRecord, VacuumReportRecord,
};
pub use ports::ApplicationStore;
pub use service::ApplicationService;

/// Host-facing service store alias kept explicit so adapters do not depend on
/// persistence row models or the old standalone store crate.
pub type ServiceStore = TursoApplicationStore;

// Canonical persistence entry points.  Store row models stay private so the
// service DTO boundary cannot accidentally expose a second application model.
pub use db::{CapabilityRecord, TursoStore, UpgradeBackupHook, UpgradeBackupRequest};
pub use error::StoreError;

pub use maintenance::{
    StoreBackupReport, StoreCheckpointReport, StoreDoctorDerivedStore, StoreDoctorIssue,
    StoreDoctorReport, StoreExportReport, StoreImportReport, StoreMaintenanceOwner,
    StoreMaintenanceRun, StoreMaintenanceStatus, StoreProjectionStatus, StoreVacuumReport,
};

// Inputs are explicit aliases at the service boundary.  Records and options
// that collide with application DTOs remain internal to `store_operations`.
pub use store_operations::{
    AddDependencyInput, AddDependencyRecord as StoreAddDependencyRecord, AddTaskLabelsInput,
    AddTaskLabelsRecord as StoreAddTaskLabelsRecord, ArchiveBoardInput, ArchiveTaskInput,
    BlockTaskInput, ClaimTaskInput, ClaimTaskRecord as StoreClaimTaskRecord, CompleteStepInput,
    CompleteTaskInput, CreateAttachmentInput, CreateBoardInput, CreateCommentInput,
    CreateLabelInput, CreateSignalInput, CreateStepInput, CreateTaskInput, EntityUpsertInput,
    HeartbeatTaskInput, LabelProposalDecisionInput, LabelProposalInput, LabelSuggestionOptions,
    MarkExecutionPlanNotRequiredInput, OntologyActionInput, OntologyActorInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput, RelationDeleteInput,
    RelationPredicateInput, RelationUpsertInput, ReleaseTaskInput, RemoveDependencyInput,
    RemoveDependencyRecord as StoreRemoveDependencyRecord, RemoveStepInput, RemoveTaskLabelInput,
    ReopenStepInput, ReopenTaskInput, ReviewSignalsInput, SignalLifecycleInput, SkipStepInput,
    SpecifyTaskInput, StoreSignalListOptions, SubmitReviewTaskInput, UnblockTaskInput,
    UpdateStepInput, UpdateTaskInput, UpsertLabelSemanticsInput,
};

pub use store_operations::search::{
    StoreSearchHit, StoreSearchIndexStatus, StoreSearchMeta, StoreSearchQuery, StoreSearchResults,
};

pub use store_operations::{
    StoreBoardTaskMapOptions, StoreEntityListOptions, StoreGraphNeighborsOptions,
    StoreGraphQueryOptions, StoreProjectionStatusOptions, StoreRelationListOptions,
    StoreTaskListOptions, StoreTaskListSort, StoreTaskNeighborhoodOptions, StoreTaskPlanFilter,
};

pub use vector::{
    MAX_VECTOR_BATCH, MAX_VECTOR_CONTENT_BYTES, MAX_VECTOR_DIMENSIONS, ProjectionJobRecord,
    VECTOR_BACKEND, VECTOR_LABEL_ATOMS_PROJECTION, VECTOR_TASKS_PROJECTION, VectorChunkHitRecord,
    VectorConfig, VectorDocumentInput, VectorEmbeddingInput, VectorLabelAtomHitRecord,
    VectorStatusRecord, content_hash, stable_id,
};

#[cfg(feature = "legacy-sqlite-import")]
pub use legacy_import::{
    LegacyImportOptions, LegacyImportResult, LegacyImportTableCount, LegacySqliteImportOptions,
    LegacySqliteImportResult, import_legacy_sqlite_v30,
};
