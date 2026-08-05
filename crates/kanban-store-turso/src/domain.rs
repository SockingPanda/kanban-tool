#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: String,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdgeRecord {
    pub parent: TaskRecord,
    pub child: TaskRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySnapshotRecord {
    pub task: TaskRecord,
    pub parents: Vec<TaskRecord>,
    pub children: Vec<TaskRecord>,
    pub edges: Vec<DependencyEdgeRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub metadata_json: String,
    pub created_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepRecord {
    pub id: String,
    pub board_id: String,
    pub parent_task_id: String,
    pub title: String,
    pub body: Option<String>,
    pub linked_task: Option<TaskRecord>,
    pub position: i64,
    pub required: bool,
    pub status: String,
    pub resolution_note: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepsRecord {
    pub task_id: String,
    pub steps: Vec<TaskStepRecord>,
    pub execution_plan: TaskExecutionPlanRecord,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionPlanRecord {
    pub board_id: String,
    pub task_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub updated_by: String,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: String,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_token: String,
    pub claim_owner: String,
    pub claim_expires_at: i64,
    pub started_at: i64,
    pub last_heartbeat_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub metadata_json: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    pub task_ref: String,
    pub seq: i64,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub position: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub claim_token: Option<String>,
    pub claim_owner: Option<String>,
    pub claim_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    pub max_retries: Option<i64>,
    pub result_summary: Option<String>,
    pub result_json: Option<String>,
    pub metadata_json: String,
    pub lock_version: i64,
    pub dependency_blocked: bool,
    pub unfinished_parent_count: i64,
    pub execution_plan_state: String,
    pub required_step_count: i64,
    pub completed_required_step_count: i64,
    pub optional_step_count: i64,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}
