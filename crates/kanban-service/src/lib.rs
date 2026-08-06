#![doc = include_str!("../README.md")]

//! 所有 kanban-tool adapter 共享的 application service 与规范 Turso 边界。
//!
//! HTTP host 在规范 store 之上构造一个 [`ApplicationService`]。CLI、MCP 和 Desktop
//! 从不构造此 service，也不会接收 storage handle；它们通过 localhost API 访问。

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

// host 的 adoption 测试通过这个显式 feature 使用 service-owned 数据库 fixture；默认
// 产品构建不会编译该模块，也不会因此获得第二个 SQLite runtime backend。
#[cfg(feature = "test-support")]
#[path = "test_support/adoption.rs"]
pub mod adoption_test_support;

pub mod dto;
pub mod operations;
pub mod ports;
pub mod service;
pub mod vector;

#[cfg(feature = "legacy-sqlite-import")]
pub mod legacy_import;

// 下面重新导出 entity/relation/graph DTO，使每个 host adapter（包括 host-admin
// maintenance）都使用同一个规范 service boundary。

pub use adapter::TursoApplicationStore;
pub use dto::*;
pub use kanban_core::{Board, BoardColumn, KanbanError, Result, TaskStatus, new_task_id};
pub use operations::LabelOntologyOperations;
pub use operations::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, ArchiveBoardCommand, ArchiveBoardRecord,
    ArchiveTaskCommand, BackupReportRecord, BoardTaskMapOptions, CheckpointReportRecord,
    ContextBuildOptions, ContextCandidate, ContextDiagnostic, ContextEvidence, ContextItem,
    ContextPack, ContextPolicy, ContextProviderStatus, ContextSources, CreateAttachmentCommand,
    CreateAttachmentRecord, CreateBoardCommand, CreateBoardLabelCommand, CreateBoardRecord,
    DeleteAttachmentCommand, DoctorDerivedStoreRecord, DoctorIssueRecord, DoctorReportRecord,
    EntityListOptions, EntityUpsertCommand, EventListOptions, EventListPage, EventRecord,
    ExportReportRecord, GraphNeighborsOptions, GraphQueryOptions, ImportReportRecord,
    LegacyImportOptionsRecord, LegacyImportResultRecord, LegacyImportTableCountRecord,
    MAX_CONTEXT_BUDGET, MAX_CONTEXT_DEPTH, MAX_CONTEXT_LIMIT, MAX_SEARCH_LIMIT,
    MaintenanceOwnerRecord, MaintenanceQuery, MaintenanceRunRecord, MaintenanceStatusRecord,
    ProjectionStatusOptions, ProjectionStatusRecord, RUN_LOG_TAIL_BYTES, ReclaimTaskCommand,
    RelationDeleteCommand, RelationListOptions, RelationPredicateCommand, RelationUpsertCommand,
    RemoveTaskLabelCommand, ReopenTaskCommand, RunLogRecord, SearchHit, SearchIndexStatus,
    SearchMeta, SearchQuery, SearchResults, SpecifyTaskCommand, TaskDetailOntologyRecord,
    TaskDetailRead, TaskDetailRecord, TaskNeighborhoodOptions, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord, UnblockTaskCommand, UpdateTaskCommand, VacuumReportRecord,
    VectorChunkQueryCommand, VectorChunkResult, VectorConfigureCommand,
    VectorLabelAtomQueryCommand, VectorLabelAtomResult, VectorStatus,
};
pub use ports::ApplicationStore;
pub use service::{ApplicationService, KanbanService};

/// 明确保留的 host-facing service store 别名，使 adapter 不依赖持久化 row model
/// 或旧的独立 store crate。
pub type ServiceStore = TursoApplicationStore;

// 规范持久化入口。Store row model 保持私有，避免 service DTO 边界意外暴露第二套
// application model。
pub use db::{CapabilityRecord, TursoStore, UpgradeBackupHook, UpgradeBackupRequest};
pub use error::StoreError;

pub use maintenance::{
    StoreBackupReport, StoreCheckpointReport, StoreDoctorDerivedStore, StoreDoctorIssue,
    StoreDoctorReport, StoreExportReport, StoreImportReport, StoreMaintenanceOwner,
    StoreMaintenanceRun, StoreMaintenanceStatus, StoreProjectionStatus, StoreVacuumReport,
};

// 输入在 service 边界显式使用别名。与 application DTO 重名的 record 和 option
// 保持在 `store_operations` 内部。
pub use store_operations::{
    AddTaskLabelsInput, ArchiveBoardInput, ArchiveTaskInput, BlockTaskInput, ClaimTaskInput,
    ClaimTaskRecord as StoreClaimTaskRecord, CompleteTaskInput, CreateAttachmentInput,
    CreateBoardInput, CreateCommentInput, CreateLabelInput, CreateSignalInput, EntityUpsertInput,
    HeartbeatTaskInput, LabelProposalDecisionInput, LabelProposalInput, LabelSuggestionOptions,
    MarkExecutionPlanNotRequiredInput, OntologyActionInput, OntologyActorInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput, RelationDeleteInput,
    RelationPredicateInput, RelationUpsertInput, ReleaseTaskInput, RemoveTaskLabelInput,
    ReopenTaskInput, ReviewSignalsInput, SignalLifecycleInput, SpecifyTaskInput,
    StoreSignalListOptions, SubmitReviewTaskInput, UnblockTaskInput, UpsertLabelSemanticsInput,
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
