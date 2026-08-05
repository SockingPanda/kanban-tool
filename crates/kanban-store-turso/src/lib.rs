mod db;
mod domain;
mod error;
mod maintenance;
mod migration;
mod operations;
mod schema;
mod shared;
mod vector;

#[cfg(feature = "legacy-sqlite-import")]
pub mod legacy_import;

pub use db::{CapabilityRecord, TursoStore, UpgradeBackupHook, UpgradeBackupRequest};
pub use domain::*;
pub use error::StoreError;
pub use maintenance::{
    StoreBackupReport, StoreCheckpointReport, StoreDoctorDerivedStore, StoreDoctorIssue,
    StoreDoctorReport, StoreExportReport, StoreImportReport, StoreMaintenanceOwner,
    StoreMaintenanceRun, StoreMaintenanceStatus, StoreProjectionStatus, StoreVacuumReport,
};

#[cfg(feature = "legacy-sqlite-import")]
pub use legacy_import::{
    LegacyImportOptions, LegacyImportResult, LegacyImportTableCount, LegacySqliteImportOptions,
    LegacySqliteImportResult, import_legacy_sqlite_v30,
};
pub use operations::CreateAttachmentInput;
pub use operations::search::{
    StoreSearchHit, StoreSearchIndexStatus, StoreSearchMeta, StoreSearchQuery, StoreSearchResults,
};
pub use operations::{
    AddDependencyInput, AddDependencyRecord, AddTaskLabelsInput, AddTaskLabelsRecord,
    ArchiveBoardInput, ArchiveTaskInput, BlockTaskInput, BoardTaskMapOptions, ClaimTaskInput,
    ClaimTaskRecord, CompleteStepInput, CompleteTaskInput, CreateBoardInput, CreateCommentInput,
    CreateLabelInput, CreateSignalInput, CreateStepInput, CreateTaskInput, EntityListOptions,
    EntityUpsertInput, GraphNeighborsOptions, GraphQueryOptions, HeartbeatTaskInput,
    LabelProposalDecisionInput, LabelProposalInput, LabelSuggestionOptions,
    MarkExecutionPlanNotRequiredInput, OntologyActionInput, OntologyActorInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    ProjectionStatusOptions, PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput,
    RelationDeleteInput, RelationListOptions, RelationPredicateInput, RelationUpsertInput,
    ReleaseTaskInput, RemoveDependencyInput, RemoveDependencyRecord, RemoveStepInput,
    RemoveTaskLabelInput, ReopenStepInput, ReopenTaskInput, ReviewSignalsInput,
    SignalLifecycleInput, SignalListOptions, SkipStepInput, SpecifyTaskInput,
    SubmitReviewTaskInput, TaskListOptions, TaskListSort, TaskNeighborhoodOptions, TaskPlanFilter,
    UnblockTaskInput, UpdateStepInput, UpdateTaskInput, UpsertLabelSemanticsInput,
};
pub use vector::{
    MAX_VECTOR_BATCH, MAX_VECTOR_CONTENT_BYTES, MAX_VECTOR_DIMENSIONS, ProjectionJobRecord,
    VECTOR_BACKEND, VECTOR_LABEL_ATOMS_PROJECTION, VECTOR_TASKS_PROJECTION, VectorChunkHitRecord,
    VectorConfig, VectorDocumentInput, VectorEmbeddingInput, VectorLabelAtomHitRecord,
    VectorStatusRecord, content_hash, stable_id,
};

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod db_tests;

#[cfg(test)]
mod db_constraints_tests;

#[cfg(test)]
mod capability_tests;

#[cfg(test)]
mod graph_tests;
