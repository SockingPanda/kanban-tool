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

pub use attachment::{
    AttachmentCreate, AttachmentDelete, AttachmentList, AttachmentRead, CreateAttachmentCommand,
    CreateAttachmentRecord, DeleteAttachmentCommand,
};
pub use board::{
    ArchiveBoardCommand, ArchiveBoardRecord, BoardArchive, BoardColumns, BoardCreate, BoardGet,
    BoardList, CreateBoardCommand, CreateBoardRecord,
};
pub use comment::{CommentCreate, CommentList};
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
pub use run::*;
pub use search::{
    MAX_SEARCH_LIMIT, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults,
    SearchTasks,
};
pub use signal::*;
pub use stats::*;
pub use step::{StepComplete, StepCreate, StepList, StepRemove, StepReopen, StepSkip, StepUpdate};
pub use task::{
    TaskArchive, TaskBlock, TaskClaim, TaskCreate, TaskDetailOntologyRecord, TaskDetailRead,
    TaskDetailRecord, TaskDone, TaskHeartbeat, TaskList, TaskOntologySignalSummaryRecord,
    TaskOntologySummaryRecord, TaskPlanNotRequired, TaskPromote, TaskReclaim, TaskReclaimExplicit,
    TaskRelease, TaskReopen, TaskReview, TaskShow, TaskSpecify, TaskUnblock, TaskUpdate,
};
pub use vector::{
    VectorChunkQueryCommand, VectorChunkResult, VectorConfigureCommand,
    VectorLabelAtomQueryCommand, VectorLabelAtomResult, VectorStatus,
};

pub use comment::{CreateCommentCommand, CreateCommentRecord};
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
    ArchiveTaskCommand, ArchiveTaskRecord, BlockTaskCommand, BlockTaskRecord, ClaimTaskCommand,
    ClaimTaskRecord, CompleteTaskCommand, CompleteTaskRecord, CreateTaskCommand, CreateTaskRecord,
    HeartbeatTaskCommand, HeartbeatTaskRecord, MarkExecutionPlanNotRequiredCommand,
    MarkExecutionPlanNotRequiredRecord, PromoteTaskCommand, PromoteTaskRecord,
    ReclaimExpiredTaskRecord, ReclaimTaskCommand, ReclaimTaskRecord, ReleaseTaskCommand,
    ReleaseTaskRecord, ReopenTaskCommand, ReopenTaskRecord, SpecifyTaskCommand, SpecifyTaskRecord,
    SubmitReviewTaskCommand, SubmitReviewTaskRecord, TaskListOptions, TaskListPage, TaskListSort,
    TaskPlanFilter, UnblockTaskCommand, UnblockTaskRecord, UpdateTaskCommand, UpdateTaskRecord,
};

#[cfg(test)]
pub(crate) mod test_support;
