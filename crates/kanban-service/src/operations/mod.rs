//! 按领域分组的 application operation。
//!
//! 每个 operation 模块都向 [`ApplicationService`] 添加一个内聚的 command 或 query。
//! 共享 service 状态仍保留在 service core 中。
// Signal ledger operation 由所有 adapter 共享。

mod attachment;
mod board;
mod comment;
mod context;
mod dependency;
mod entities;
mod event;
mod graph;
mod labels;
mod maintenance;
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
    ContextBuild, ContextBuildOptions, ContextCandidate, ContextDiagnostic, ContextEvidence,
    ContextItem, ContextPack, ContextPolicy, ContextProviderStatus, ContextSources,
    MAX_CONTEXT_BUDGET, MAX_CONTEXT_DEPTH, MAX_CONTEXT_LIMIT,
};
pub use dependency::{DependencyCreate, DependencyList, DependencyRemove};
pub use entities::{EntityListOptions, EntityQuery, EntityUpsertCommand};
pub use event::*;
pub use graph::{
    BoardTaskMapOptions, GraphNeighborsOptions, GraphQuery, GraphQueryOptions,
    ProjectionStatusOptions, TaskNeighborhoodOptions,
};
pub use labels::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, AddTaskLabelsRecordInput, BoardLabelCreate,
    BoardLabelList, CreateBoardLabelCommand, CreateLabelRecord, RemoveTaskLabelCommand,
    RemoveTaskLabelRecord, TaskLabelAdd, TaskLabelList, TaskLabelRemove,
};
pub use maintenance::*;
pub use ontology::LabelOntologyOperations;
pub use relations::{
    RelationDeleteCommand, RelationListOptions, RelationPredicateCommand, RelationQuery,
    RelationUpsertCommand,
};
pub(crate) use run::application_run;
pub use run::*;
pub use search::{
    MAX_SEARCH_LIMIT, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults,
    SearchTasks,
};
pub use signal::*;
pub use stats::*;
pub use step::{StepComplete, StepCreate, StepList, StepRemove, StepReopen, StepSkip, StepUpdate};
pub(crate) use task::application_task;
pub use task::{
    TaskDetailOntologyRecord, TaskDetailRead, TaskDetailRecord, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord,
};
pub use vector::{
    VectorChunkQueryCommand, VectorChunkResult, VectorConfigureCommand,
    VectorLabelAtomQueryCommand, VectorLabelAtomResult, VectorStatus,
};

pub use dependency::{
    AddDependencyCommand, AddDependencyRecord, AddDependencyResult, RemoveDependencyCommand,
    RemoveDependencyResult,
};
pub use step::{
    CompleteStepCommand, CompleteStepRecord, CreateStepCommand, CreateStepRecord,
    RemoveStepCommand, RemoveStepRecord, ReopenStepCommand, ReopenStepRecord, SkipStepCommand,
    SkipStepRecord, UpdateStepCommand, UpdateStepRecord,
};
pub use task::{
    ArchiveTaskCommand, BlockTaskCommand, ClaimTaskCommand, CompleteTaskCommand, CreateTaskCommand,
    HeartbeatTaskCommand, MarkExecutionPlanNotRequiredCommand, PromoteTaskCommand,
    ReclaimTaskCommand, ReleaseTaskCommand, ReopenTaskCommand, SpecifyTaskCommand,
    SubmitReviewTaskCommand, TaskListOptions, TaskListPage, TaskListSort, TaskPlanFilter,
    UnblockTaskCommand, UpdateTaskCommand,
};

#[cfg(test)]
pub(crate) mod test_support;
