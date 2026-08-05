//! Application operations grouped by domain.
//!
//! Each operation module extends [`ApplicationService`] with one cohesive
//! command or query. Shared service state remains in the service core.

mod board;
mod comment;
mod dependency;
mod event;
mod run;
mod step;
mod task;

pub use board::{BoardColumns, BoardList};
pub use comment::{CommentCreate, CommentList};
pub use dependency::{DependencyCreate, DependencyList, DependencyRemove};
pub use event::*;
pub use run::*;
pub use step::{StepCreate, StepList, StepUpdate};
pub use task::{
    TaskBlock, TaskClaim, TaskCreate, TaskDone, TaskHeartbeat, TaskList, TaskPlanNotRequired,
    TaskPromote, TaskReclaim, TaskRelease, TaskReview, TaskShow,
};

pub use comment::{CreateCommentCommand, CreateCommentRecord};
pub use dependency::{
    AddDependencyCommand, AddDependencyRecord, AddDependencyResult, RemoveDependencyCommand,
    RemoveDependencyResult,
};
pub use step::{CreateStepCommand, CreateStepRecord, UpdateStepCommand, UpdateStepRecord};
pub use task::{
    BlockTaskCommand, BlockTaskRecord, ClaimTaskCommand, ClaimTaskRecord, CompleteTaskCommand,
    CompleteTaskRecord, CreateTaskCommand, CreateTaskRecord, HeartbeatTaskCommand,
    HeartbeatTaskRecord, MarkExecutionPlanNotRequiredCommand, MarkExecutionPlanNotRequiredRecord,
    PromoteTaskCommand, PromoteTaskRecord, ReclaimExpiredTaskRecord, ReleaseTaskCommand,
    ReleaseTaskRecord, SubmitReviewTaskCommand, SubmitReviewTaskRecord, TaskListOptions,
    TaskListPage, TaskListSort, TaskPlanFilter,
};

#[cfg(test)]
pub(crate) mod test_support;
