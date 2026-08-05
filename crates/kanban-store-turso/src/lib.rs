mod db;
mod domain;
mod error;
mod migration;
mod operations;
mod schema;
mod shared;

#[cfg(feature = "legacy-sqlite-import")]
pub mod legacy_import;

pub use db::{CapabilityRecord, TursoStore, UpgradeBackupHook, UpgradeBackupRequest};
pub use domain::*;
pub use error::StoreError;

#[cfg(feature = "legacy-sqlite-import")]
pub use legacy_import::{
    LegacyImportOptions, LegacyImportResult, LegacyImportTableCount, LegacySqliteImportOptions,
    LegacySqliteImportResult, import_legacy_sqlite_v30,
};
pub use operations::{
    AddDependencyInput, AddDependencyRecord, ArchiveBoardInput, ArchiveTaskInput, BlockTaskInput,
    ClaimTaskInput, ClaimTaskRecord, CompleteStepInput, CompleteTaskInput, CreateBoardInput,
    CreateCommentInput, CreateStepInput, CreateTaskInput, HeartbeatTaskInput,
    MarkExecutionPlanNotRequiredInput, PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput,
    ReleaseTaskInput, RemoveDependencyInput, RemoveDependencyRecord, RemoveStepInput,
    ReopenStepInput, ReopenTaskInput, SkipStepInput, SpecifyTaskInput, SubmitReviewTaskInput,
    TaskListOptions, TaskListSort, TaskPlanFilter, UnblockTaskInput, UpdateStepInput,
    UpdateTaskInput,
};

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod db_tests;

#[cfg(test)]
mod db_constraints_tests;

#[cfg(test)]
mod capability_tests;
