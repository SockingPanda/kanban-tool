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
    AddTaskLabelsCommand, AddTaskLabelsRecord, AddTaskLabelsRecordInput, ArchiveBoardCommand,
    ArchiveBoardRecord, ArchiveTaskCommand, ArchiveTaskRecord,
    AttachmentCreate, AttachmentDelete, AttachmentList, AttachmentRead, BackupReportRecord,
    BoardArchive, BoardColumns, BoardCreate, BoardGet, BoardLabelCreate, BoardLabelList, BoardList,
    CheckpointReportRecord, CommentCreate, CommentList, CreateAttachmentCommand,
    CreateAttachmentRecord, CreateBoardCommand, CreateBoardLabelCommand, CreateBoardRecord,
    CreateLabelRecord, DeleteAttachmentCommand, DependencyCreate, DependencyList,
    DependencyRemove, DoctorDerivedStoreRecord, DoctorIssueRecord, DoctorReportRecord, EventList,
    EventListOptions, EventListPage, EventRecord, ExportReportRecord, ImportReportRecord,
    LegacyImportOptionsRecord, LegacyImportResultRecord, LegacyImportTableCountRecord,
    MAX_SEARCH_LIMIT, MaintenanceOwnerRecord, MaintenanceQuery, MaintenanceRunRecord,
    MaintenanceStatusRecord, ProjectionStatusRecord, RUN_LOG_TAIL_BYTES, ReclaimTaskCommand,
    ReclaimTaskRecord, RemoveTaskLabelCommand, RemoveTaskLabelRecord, ReopenTaskCommand,
    ReopenTaskRecord, RunList, RunLog, RunLogRecord, RunShow, SearchHit, SearchIndexStatus,
    SearchMeta, SearchQuery, SearchResults, SearchTasks, SpecifyTaskCommand, SpecifyTaskRecord,
    StepComplete, StepCreate, StepList, StepRemove, StepReopen, StepSkip, StepUpdate, TaskArchive,
    TaskBlock, TaskClaim, TaskCreate, TaskDone, TaskHeartbeat, TaskLabelAdd, TaskLabelList,
    TaskLabelRemove, TaskList, TaskPlanNotRequired, TaskPromote, TaskReclaim, TaskReclaimExplicit,
    TaskRelease, TaskReopen, TaskReview, TaskShow, TaskSpecify, TaskUnblock, TaskUpdate,
    UnblockTaskCommand, UnblockTaskRecord, UpdateTaskCommand, UpdateTaskRecord, VacuumReportRecord,
};
pub use ports::ApplicationStore;
pub use service::ApplicationService;
