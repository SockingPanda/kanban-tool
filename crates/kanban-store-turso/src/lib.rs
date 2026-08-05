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
    LegacyImportOptions, LegacyImportResult, LegacySqliteImportOptions, LegacySqliteImportResult,
    import_legacy_sqlite_v30,
};
pub use operations::{
    AddDependencyInput, AddDependencyRecord, ArchiveBoardInput, BlockTaskInput, ClaimTaskInput,
    ClaimTaskRecord, CompleteStepInput, CompleteTaskInput, CreateBoardInput, CreateCommentInput,
    CreateStepInput, CreateTaskInput, HeartbeatTaskInput, MarkExecutionPlanNotRequiredInput,
    PromoteTaskInput, ReclaimExpiredTaskInput, ReleaseTaskInput, RemoveDependencyInput,
    RemoveDependencyRecord, RemoveStepInput, ReopenStepInput, SkipStepInput, SubmitReviewTaskInput,
    TaskListOptions, TaskListSort, TaskPlanFilter, UpdateStepInput,
};

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod db_tests;

#[cfg(test)]
mod db_constraints_tests;

#[cfg(test)]
mod capability_tests;
