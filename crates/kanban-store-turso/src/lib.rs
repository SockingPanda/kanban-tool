mod db;
mod domain;
mod error;
mod maintenance;
mod migration;
mod operations;
mod schema;
mod shared;

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
    ArchiveBoardInput, ArchiveTaskInput, BlockTaskInput, ClaimTaskInput, ClaimTaskRecord,
    CompleteStepInput, CompleteTaskInput, CreateBoardInput, CreateCommentInput, CreateLabelInput,
    CreateSignalInput, CreateStepInput, CreateTaskInput, HeartbeatTaskInput,
    LabelProposalDecisionInput, LabelProposalInput, LabelSuggestionOptions,
    MarkExecutionPlanNotRequiredInput, OntologyActionInput, OntologyActorInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput, ReleaseTaskInput,
    RemoveDependencyInput, RemoveDependencyRecord, RemoveStepInput, RemoveTaskLabelInput,
    ReopenStepInput, ReopenTaskInput, ReviewSignalsInput, SignalLifecycleInput, SignalListOptions,
    SkipStepInput, SpecifyTaskInput, SubmitReviewTaskInput, TaskListOptions, TaskListSort,
    TaskPlanFilter, UnblockTaskInput, UpdateStepInput, UpdateTaskInput, UpsertLabelSemanticsInput,
};

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod db_tests;

#[cfg(test)]
mod db_constraints_tests;

#[cfg(test)]
mod capability_tests;
