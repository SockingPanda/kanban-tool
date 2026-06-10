use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(super) struct Envelope<T> {
    pub(super) data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) meta: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct ErrorEnvelope {
    pub(super) error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub(super) struct ErrorBody {
    pub(super) code: &'static str,
    pub(super) message: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TaskDto {
    pub(super) id: String,
    pub(super) board_id: String,
    pub(super) board_slug: String,
    #[serde(rename = "ref")]
    pub(super) task_ref: String,
    pub(super) seq: i64,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) status: TaskStatus,
    pub(super) status_reason: Option<String>,
    pub(super) assignee: Option<String>,
    pub(super) priority: i64,
    pub(super) position: i64,
    pub(super) scheduled_at: Option<i64>,
    pub(super) due_at: Option<i64>,
    pub(super) created_by: String,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
    pub(super) started_at: Option<i64>,
    pub(super) completed_at: Option<i64>,
    pub(super) archived_at: Option<i64>,
    pub(super) claim_owner: Option<String>,
    pub(super) claim_expires_at: Option<i64>,
    pub(super) last_heartbeat_at: Option<i64>,
    pub(super) current_run_id: Option<String>,
    pub(super) retry_count: i64,
    pub(super) max_retries: Option<i64>,
    pub(super) result_summary: Option<String>,
    pub(super) result_json: Option<String>,
    pub(super) metadata_json: String,
    pub(super) lock_version: i64,
}

impl From<kanban_sqlite::TaskRecord> for TaskDto {
    fn from(task: kanban_sqlite::TaskRecord) -> Self {
        Self {
            id: task.id,
            board_id: task.board_id,
            board_slug: task.board_slug,
            task_ref: task.task_ref,
            seq: task.seq,
            title: task.title,
            description: task.description,
            status: task.status,
            status_reason: task.status_reason,
            assignee: task.assignee,
            priority: task.priority,
            position: task.position,
            scheduled_at: task.scheduled_at,
            due_at: task.due_at,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
            archived_at: task.archived_at,
            claim_owner: task.claim_owner,
            claim_expires_at: task.claim_expires_at,
            last_heartbeat_at: task.last_heartbeat_at,
            current_run_id: task.current_run_id,
            retry_count: task.retry_count,
            max_retries: task.max_retries,
            result_summary: task.result_summary,
            result_json: task.result_json,
            metadata_json: task.metadata_json,
            lock_version: task.lock_version,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct RunDto {
    pub(super) id: String,
    pub(super) task_id: String,
    pub(super) status: String,
    pub(super) worker_profile: Option<String>,
    pub(super) worker_pid: Option<i64>,
    pub(super) claim_owner: String,
    pub(super) started_at: i64,
    pub(super) finished_at: Option<i64>,
    pub(super) exit_code: Option<i64>,
    pub(super) summary: Option<String>,
    pub(super) error: Option<String>,
    pub(super) log_path: Option<String>,
    pub(super) metadata_json: String,
}

impl From<kanban_sqlite::RunRecord> for RunDto {
    fn from(run: kanban_sqlite::RunRecord) -> Self {
        Self {
            id: run.id,
            task_id: run.task_id,
            status: run.status,
            worker_profile: run.worker_profile,
            worker_pid: run.worker_pid,
            claim_owner: run.claim_owner,
            started_at: run.started_at,
            finished_at: run.finished_at,
            exit_code: run.exit_code,
            summary: run.summary,
            error: run.error,
            log_path: run.log_path,
            metadata_json: run.metadata_json,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct EventDto {
    pub(super) id: i64,
    pub(super) event_id: String,
    pub(super) board_id: String,
    pub(super) task_id: Option<String>,
    pub(super) run_id: Option<String>,
    pub(super) kind: String,
    pub(super) actor: Option<String>,
    pub(super) payload: serde_json::Value,
    pub(super) created_at: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct CommentDto {
    pub(super) id: String,
    pub(super) board_id: String,
    pub(super) task_id: String,
    pub(super) author: String,
    pub(super) author_type: String,
    pub(super) agent_type: Option<String>,
    pub(super) body: String,
    pub(super) kind: String,
    pub(super) created_at: i64,
}

impl From<kanban_sqlite::CommentRecord> for CommentDto {
    fn from(comment: kanban_sqlite::CommentRecord) -> Self {
        Self {
            id: comment.id,
            board_id: comment.board_id,
            task_id: comment.task_id,
            author: comment.author,
            author_type: comment.author_type,
            agent_type: comment.agent_type,
            body: comment.body,
            kind: comment.kind,
            created_at: comment.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct RunLogDto {
    pub(super) run_id: String,
    pub(super) content: String,
    pub(super) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct ClaimDto {
    pub(super) task: TaskDto,
    pub(super) run: RunDto,
    pub(super) claim_token: String,
    pub(super) claim_expires_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(super) struct DependenciesDto {
    pub(super) parents: Vec<TaskDto>,
    pub(super) children: Vec<TaskDto>,
}

#[derive(Debug, Serialize)]
pub(super) struct SearchTaskHitDto {
    pub(super) task_id: String,
    pub(super) seq: i64,
    pub(super) score: f64,
    pub(super) snippet: Option<String>,
    pub(super) task: TaskDto,
}

#[derive(Debug, Serialize)]
pub(super) struct SearchTasksDto {
    pub(super) hits: Vec<SearchTaskHitDto>,
    pub(super) meta: kanban_search::SearchMeta,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContextBuildQuery {
    #[serde(default = "default_board")]
    pub(super) board: String,
    #[serde(default = "default_context_lexical_limit")]
    pub(super) lexical_limit: usize,
    #[serde(default = "default_context_graph_limit")]
    pub(super) graph_limit: usize,
    #[serde(default = "default_context_vector_limit")]
    pub(super) vector_limit: usize,
    #[serde(default = "default_context_max_items")]
    pub(super) max_items: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct BoardQuery {
    #[serde(default = "default_board")]
    pub(super) board: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphNeighborsQuery {
    pub(super) entity_uri: String,
    pub(super) predicate: Option<String>,
    #[serde(default = "default_graph_limit")]
    pub(super) limit: usize,
}

fn default_board() -> String {
    "default".to_owned()
}

fn default_context_lexical_limit() -> usize {
    5
}

fn default_context_graph_limit() -> usize {
    10
}

fn default_context_vector_limit() -> usize {
    5
}

fn default_context_max_items() -> usize {
    20
}

fn default_graph_limit() -> usize {
    50
}
