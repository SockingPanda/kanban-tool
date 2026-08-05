//! Shared application-service boundary for every kanban-tool adapter.
//!
//! The HTTP host constructs one [`ApplicationService`] over the canonical
//! store. CLI, MCP and Desktop never construct this service and never receive a
//! storage handle; they reach it through the localhost API.

pub mod dto;
pub mod operations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use operations::{
    BoardColumns, BoardList, CommentCreate, CommentList, DependencyCreate, DependencyList,
    DependencyRemove, EventList, EventListOptions, EventListPage, EventRecord, RUN_LOG_TAIL_BYTES,
    RunList, RunLog, RunLogRecord, RunShow, StepComplete, StepCreate, StepList, StepRemove,
    StepReopen, StepSkip, StepUpdate, TaskBlock, TaskClaim, TaskCreate, TaskDone, TaskHeartbeat,
    TaskList, TaskPlanNotRequired, TaskPromote, TaskReclaim, TaskRelease, TaskReview, TaskShow,
};
pub use ports::ApplicationStore;
pub use service::ApplicationService;
