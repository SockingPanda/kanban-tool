#![doc = include_str!("../README.md")]

//! kanban-tool 的 application service 与规范 Turso 边界。
//!
//! `kanban-server` 通过 [`KanbanService`] 持有唯一的 canonical store；CLI、MCP 和 Desktop
//! 通过 localhost API 访问，不直接接触数据库。

mod attachment_stream;
mod db;
mod domain;
mod error;
mod maintenance;
mod migration;
mod mutation_gate;
pub mod realtime;
mod schema;
mod shared;
mod store_operations;

#[cfg(test)]
mod test_support;

// service 的持久化、迁移、能力和图约束测试必须显式挂载；仅保留文件不会进入测试二进制。
#[cfg(test)]
mod db_tests;

#[cfg(test)]
mod db_constraints_tests;

#[cfg(test)]
mod capability_tests;

#[cfg(test)]
mod graph_tests;

// host 的 adoption 测试通过这个显式 feature 使用 service-owned 数据库 fixture；默认
// 产品构建不会编译该模块，也不会因此获得第二个 SQLite runtime backend。
#[cfg(feature = "test-support")]
#[path = "test_support/adoption.rs"]
pub mod adoption_test_support;

pub mod dto;
pub mod object_model;
pub mod operations;
mod service;
mod suggestion_engine;
mod vector;

#[cfg(feature = "legacy-sqlite-import")]
mod legacy_import;

pub use attachment_stream::{ATTACHMENT_IO_CHUNK_BYTES, AttachmentUpload, MAX_ATTACHMENT_BYTES};
pub use dto::*;
pub use kanban_core::{Board, BoardColumn, KanbanError, Result, TaskStatus, new_task_id};
pub use operations::AttachmentStreamRecord;
pub use operations::maintenance::{
    BackupReportRecord, CheckpointReportRecord, DoctorDerivedStoreRecord, DoctorIssueRecord,
    DoctorReportRecord, ExportReportRecord, ImportReportRecord, MaintenanceOwnerRecord,
    MaintenanceRunRecord, MaintenanceStatusRecord, ProjectionStatusRecord, VacuumReportRecord,
};
pub use operations::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, ArchiveBoardCommand, ArchiveBoardRecord,
    ArchiveTaskCommand, BoardTaskMapOptions, BootstrapTaskLabelCommand, BootstrapTaskLabelRecord,
    ContextBuildOptions, ContextCandidate, ContextDiagnostic, ContextEvidence, ContextItem,
    ContextPack, ContextPolicy, ContextProviderStatus, ContextSources, CreateAttachmentCommand,
    CreateAttachmentRecord, CreateBoardCommand, CreateBoardLabelCommand, CreateBoardRecord,
    DeleteAttachmentCommand, DeleteBoardLabelCommand, DeleteBoardLabelRecord, EntityListOptions,
    EntityUpsertCommand, EventListOptions, EventListPage, EventRecord, GraphNeighborsOptions,
    GraphQueryOptions, MAX_CONTEXT_BUDGET, MAX_CONTEXT_DEPTH, MAX_CONTEXT_LIMIT, MAX_SEARCH_LIMIT,
    ProjectionStatusOptions, RUN_LOG_TAIL_BYTES, ReclaimTaskCommand, RelationDeleteCommand,
    RelationListOptions, RelationPredicateCommand, RelationUpsertCommand, RemoveTaskLabelCommand,
    ReopenTaskCommand, RunLogRecord, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery,
    SearchResults, SpecifyTaskCommand, TaskDetailOntologyRecord, TaskDetailRecord,
    TaskNeighborhoodOptions, TaskOntologySignalSummaryRecord, TaskOntologySummaryRecord,
    UnblockTaskCommand, UpdateTaskCommand, VectorChunkQueryCommand, VectorChunkResult,
    VectorConfigureCommand, VectorLabelAtomQueryCommand, VectorLabelAtomResult, VectorStatus,
};
pub use operations::{
    DeleteLabelSemanticsCommand, LabelAtomExplainActionRecord, LabelAtomExplainRecord,
    LabelAtomExplainSignalRecord, LabelAtomExplainValidationRecord, LabelAtomIndexHitRecord,
    LabelAtomIndexQuery, LabelAtomIndexQueryRecord, LabelAtomIndexStatusRecord,
    LabelOntologyActionRecord, LabelOntologyObservationRecord, LabelOntologyPrecisionRecall,
    LabelOntologyQualityDenominator, LabelOntologyQualityDisagreement, LabelOntologyQualityRates,
    LabelOntologyQualityRecord, LabelOntologyReviewAtomVariantRecord,
    LabelOntologyReviewGroupRecord, LabelOntologyReviewLabelRefRecord, LabelOntologyReviewQuery,
    LabelOntologySignalDetailRecord, LabelOntologySignalQuery, LabelOntologySignalRecord,
    LabelProposalAttemptRecord, LabelProposalCommand, LabelProposalDecisionCommand,
    LabelSemanticProposalRecord, LabelSuggestionCandidateRecord, LabelSuggestionEvidenceRecord,
    LabelSuggestionOptions, LabelSuggestionResultRecord, OntologyActionCommand,
    OntologyActorCommand, OntologyApplyAtomCommand, OntologyObservationCommand,
    OntologyRevertCommand, OntologySignalCommand, OntologyValidateCommand,
    UpsertLabelSemanticsCommand,
};
pub use service::KanbanService;

// 规范持久化入口只在 service crate 内可见；host 只能使用上面的 KanbanService。
// Store row model 和 StoreError 不进入跨 crate 的 application API。
pub(crate) use db::TursoStore;
#[cfg(test)]
pub(crate) use db::{UpgradeBackupHook, UpgradeBackupRequest};
pub(crate) use error::StoreError;

// 输入在 service 边界显式使用别名。与 application DTO 重名的 record 和 option
// 保持在 `store_operations` 内部。
pub(crate) use store_operations::{
    AddTaskLabelsInput, ArchiveBoardInput, BootstrapTaskLabelInput, CreateAttachmentInput,
    CreateBoardInput, CreateCommentInput, CreateLabelInput, DeleteBoardLabelInput,
    EntityUpsertInput, LabelProposalDecisionInput, LabelProposalInput, OntologyActionInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    RelationDeleteInput, RelationPredicateInput, RelationUpsertInput, RemoveTaskLabelInput,
    UpsertLabelSemanticsInput,
};

#[cfg(feature = "legacy-sqlite-import")]
pub use legacy_import::{
    LegacyImportOptions, LegacyImportResult, LegacyImportTableCount, LegacySqliteImportOptions,
    LegacySqliteImportResult,
};
