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
pub use operations::LabelOntologyOperations;
pub use operations::{
    ArchiveBoardCommand, ArchiveBoardRecord, ArchiveTaskCommand, ArchiveTaskRecord,
    AttachmentCreate, AttachmentDelete, AttachmentList, AttachmentRead, BoardArchive,
    BoardColumns, BoardCreate, BoardGet, BoardList, CommentCreate, CommentList,
    CreateAttachmentCommand, CreateAttachmentRecord, CreateBoardCommand, CreateBoardRecord,
    DeleteAttachmentCommand, DependencyCreate, DependencyList, DependencyRemove, EventList,
    EventListOptions, EventListPage, EventRecord, MAX_SEARCH_LIMIT, RUN_LOG_TAIL_BYTES,
    ReclaimTaskCommand, ReclaimTaskRecord, ReopenTaskCommand, ReopenTaskRecord, RunList, RunLog,
    RunLogRecord, RunShow, SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults,
    SearchTasks, SpecifyTaskCommand, SpecifyTaskRecord, StepComplete, StepCreate, StepList,
    StepRemove, StepReopen, StepSkip, StepUpdate, TaskArchive, TaskBlock, TaskClaim, TaskCreate,
    TaskDone, TaskHeartbeat, TaskList, TaskPlanNotRequired, TaskPromote, TaskReclaim,
    TaskReclaimExplicit, TaskRelease, TaskReopen, TaskReview, TaskShow, TaskSpecify, TaskUnblock,
    TaskUpdate, UnblockTaskCommand, UnblockTaskRecord, UpdateTaskCommand, UpdateTaskRecord,
};
pub use ports::ApplicationStore;
pub use service::ApplicationService;
