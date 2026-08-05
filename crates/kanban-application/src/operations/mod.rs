//! Application operations grouped by domain.
//!
//! Each operation module extends [`ApplicationService`] with one cohesive
//! command or query. Shared service state remains in the service core.
// Signal ledger operations are shared by all adapters.

mod attachment;
mod board;
mod comment;
mod dependency;
mod event;
mod labels;
mod maintenance;
mod ontology;
mod run;
mod search;
mod signal;
mod stats;
mod step;
mod task;

pub use attachment::{
    AttachmentCreate, AttachmentDelete, AttachmentList, AttachmentRead, CreateAttachmentCommand,
    CreateAttachmentRecord, DeleteAttachmentCommand,
};
pub use board::{
    ArchiveBoardCommand, ArchiveBoardRecord, BoardArchive, BoardColumns, BoardCreate, BoardGet,
    BoardList, CreateBoardCommand, CreateBoardRecord,
};
pub use comment::{CommentCreate, CommentList};
pub use dependency::{DependencyCreate, DependencyList, DependencyRemove};
pub use event::*;
pub use labels::{
    AddTaskLabelsCommand, AddTaskLabelsRecord, AddTaskLabelsRecordInput, BoardLabelCreate,
    BoardLabelList, CreateBoardLabelCommand, CreateLabelRecord, RemoveTaskLabelCommand,
    RemoveTaskLabelRecord, TaskLabelAdd, TaskLabelList, TaskLabelRemove,
};
pub use maintenance::*;
pub use ontology::LabelOntologyOperations;
pub use run::*;
pub use search::{
    MAX_SEARCH_LIMIT, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults,
    SearchTasks,
};
pub use signal::*;
pub use stats::*;
pub use step::{StepComplete, StepCreate, StepList, StepRemove, StepReopen, StepSkip, StepUpdate};
pub use task::{
    TaskArchive, TaskBlock, TaskClaim, TaskCreate, TaskDone, TaskHeartbeat, TaskList,
    TaskPlanNotRequired, TaskPromote, TaskReclaim, TaskReclaimExplicit, TaskRelease, TaskReopen,
    TaskReview, TaskShow, TaskSpecify, TaskUnblock, TaskUpdate,
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
