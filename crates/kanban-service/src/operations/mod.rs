//! 按领域分组的 application operation。
//!
//! 每个 operation 模块都向共享的 application/service 入口添加一个内聚的 command 或 query。
//! 共享 service 状态仍保留在 service core 中。
// Signal ledger operation 由所有 host surface 共享。

mod attachment;
mod board;
mod comment;
mod context;
mod dependency;
mod entities;
mod event;
mod graph;
mod labels;
pub(crate) mod maintenance;
mod ontology;
mod relations;
mod run;
mod search;
mod signal;
mod stats;
mod step;
mod task;
mod vector;

pub use attachment::{CreateAttachmentCommand, CreateAttachmentRecord, DeleteAttachmentCommand};
pub use board::{ArchiveBoardCommand, ArchiveBoardRecord, CreateBoardCommand, CreateBoardRecord};
pub(crate) use comment::application_comment;
pub use comment::{CreateCommentCommand, CreateCommentRecord};
pub use context::{
    ContextBuildOptions, ContextCandidate, ContextDiagnostic, ContextEvidence, ContextItem,
    ContextPack, ContextPolicy, ContextProviderStatus, ContextSources, MAX_CONTEXT_BUDGET,
    MAX_CONTEXT_DEPTH, MAX_CONTEXT_LIMIT,
};
pub(crate) use dependency::application_dependency_snapshot;
pub use entities::{EntityListOptions, EntityUpsertCommand};
pub use event::*;
pub use graph::{
    BoardTaskMapOptions, GraphNeighborsOptions, GraphQueryOptions, ProjectionStatusOptions,
    TaskNeighborhoodOptions,
};
pub(crate) use labels::application_label;
pub use labels::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, CreateBoardLabelCommand, DeleteBoardLabelCommand,
    DeleteBoardLabelRecord, RemoveTaskLabelCommand,
};
pub use relations::{
    RelationDeleteCommand, RelationListOptions, RelationPredicateCommand, RelationUpsertCommand,
};
pub(crate) use run::application_run;
pub use run::*;
pub use search::{
    MAX_SEARCH_LIMIT, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults,
};
pub use signal::*;
pub use stats::*;
pub(crate) use task::application_task;
pub use task::{
    TaskDetailOntologyRecord, TaskDetailRecord, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord,
};
pub use vector::{
    VectorChunkQueryCommand, VectorChunkResult, VectorConfigureCommand,
    VectorLabelAtomQueryCommand, VectorLabelAtomResult, VectorStatus,
};

pub use dependency::{
    AddDependencyCommand, AddDependencyResult, RemoveDependencyCommand, RemoveDependencyResult,
};
pub use step::{
    CompleteStepCommand, CreateStepCommand, RemoveStepCommand, ReopenStepCommand, SkipStepCommand,
    UpdateStepCommand,
};
pub use task::{
    ArchiveTaskCommand, BlockTaskCommand, ClaimTaskCommand, CompleteTaskCommand, CreateTaskCommand,
    HeartbeatTaskCommand, MarkExecutionPlanNotRequiredCommand, PromoteTaskCommand,
    ReclaimTaskCommand, ReleaseTaskCommand, ReopenTaskCommand, SpecifyTaskCommand,
    SubmitReviewTaskCommand, TaskListOptions, TaskListPage, TaskListSort, TaskPlanFilter,
    UnblockTaskCommand, UpdateTaskCommand,
};
