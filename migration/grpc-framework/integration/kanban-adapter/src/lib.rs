//! 固定 Atlas 分支的 application adapter；没有新建数据库、HTTP 回调或独立状态机。
use kanban_live_core::*;
use kanban_service::{KanbanError, KanbanService, UpdateTaskCommand, realtime::RealtimeTaskCard};
#[derive(Clone)]
pub struct KanbanAdapter { application: KanbanService }
impl KanbanAdapter {
    pub fn new(application: KanbanService) -> Self { Self { application } }
}
fn map_error(error: KanbanError) -> LiveError {
    match error {
        KanbanError::NotFound(value) => LiveError::NotFound(value),
        KanbanError::Conflict(value) | KanbanError::IdempotencyConflict(value) => LiveError::Conflict(value),
        KanbanError::Storage(_) => LiveError::Unavailable,
        other => LiveError::Invalid(other.to_string()),
    }
}
fn card(value: RealtimeTaskCard) -> Result<Card> {
    let status = match value.status.as_str() {
        "triage" => Status::Triage, "todo" => Status::Todo, "scheduled" => Status::Scheduled,
        "ready" => Status::Ready, "running" => Status::Running, "blocked" => Status::Blocked,
        "review" => Status::Review, "done" => Status::Done, "archived" => Status::Archived,
        _ => return Err(LiveError::Invalid("未支持的任务状态".into())),
    };
    let result = Card { id: value.id, title: value.title, status,
        priority: value.priority.try_into().map_err(|_| LiveError::Invalid("priority".into()))?,
        position: value.position,
        seq: value.seq.try_into().map_err(|_| LiveError::Invalid("seq".into()))?,
        lock_version: value.lock_version.try_into().map_err(|_| LiveError::Invalid("lock_version".into()))? };
    result.validate()?;
    Ok(result)
}
impl BoardSource for KanbanAdapter {
    fn subscribe_changes(&self) -> tokio::sync::watch::Receiver<u64> { self.application.subscribe_realtime_changes() }
    fn load_board<'a>(&'a self, board_id: &'a str) -> BoxFuture<'a, Result<Vec<Card>>> {
        Box::pin(async move {
            let snapshot = self.application.load_realtime_board(board_id).await.map_err(map_error)?;
            if snapshot.board_id != board_id { return Err(LiveError::Invalid("RPC 需要 canonical board ID".into())); }
            snapshot.tasks.into_iter().map(card).collect()
        })
    }
}
impl TaskCommands for KanbanAdapter {
    fn update_title(&self, input: UpdateTitle) -> BoxFuture<'_, Result<Card>> {
        Box::pin(async move {
            let task = self.application.get_task(&input.task_id).await.map_err(map_error)?;
            if task.board_id != input.board_id { return Err(LiveError::NotFound(input.task_id)); }
            let expected: i64 = input.expected_version.try_into().map_err(|_| LiveError::Invalid("版本超出 i64".into()))?;
            let updated = self.application.update_task(UpdateTaskCommand {
                task_id: input.task_id, actor: input.actor, expected_lock_version: Some(expected),
                title: Some(input.title), description: None, assignee: None, priority: None,
                scheduled_at: None, due_at: None, max_retries: None, metadata: None,
            }).await.map_err(map_error)?;
            card(RealtimeTaskCard::from(updated))
        })
    }
}

impl RefreshSource for KanbanAdapter {
    fn subscribe_refreshes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.application.subscribe_realtime_changes()
    }
    fn check_board<'a>(&'a self, board_id: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move { self.application.check_realtime_board(board_id).await.map_err(map_error) })
    }
}
