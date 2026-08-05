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
    ArchiveBoardCommand, ArchiveBoardRecord, BoardArchive, BoardColumns, BoardCreate, BoardGet,
    ArchiveTaskCommand, ArchiveTaskRecord, BoardList, CommentCreate, CommentList,
    CreateBoardCommand, CreateBoardRecord, DependencyCreate, DependencyList, DependencyRemove,
    EventList, EventListOptions, EventListPage, EventRecord, RUN_LOG_TAIL_BYTES,
    ReclaimTaskCommand, ReclaimTaskRecord, ReopenTaskCommand, ReopenTaskRecord, RunList, RunLog,
    RunLogRecord, RunShow, SpecifyTaskCommand, SpecifyTaskRecord, StepComplete, StepCreate,
    StepList, StepRemove, StepReopen, StepSkip, StepUpdate, TaskArchive, TaskBlock, TaskClaim,
    TaskCreate, TaskDone, TaskHeartbeat, TaskList, TaskPlanNotRequired, TaskPromote, TaskReclaim,
    TaskReclaimExplicit, TaskRelease, TaskReopen, TaskReview, TaskShow, TaskSpecify, TaskUnblock,
    TaskUpdate, UnblockTaskCommand, UnblockTaskRecord, UpdateTaskCommand, UpdateTaskRecord,
};
pub use ports::ApplicationStore;
pub use service::ApplicationService;
