//! 实时投影的 service-owned 一致读取，不暴露数据库句柄或 Protobuf 类型。
use crate::{KanbanError, KanbanService, Result, TaskListOptions, TaskRecord, TaskStatus};
use kanban_core::Clock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeTaskCard {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: i64,
    pub position: i64,
    pub seq: i64,
    pub lock_version: i64,
}
impl From<TaskRecord> for RealtimeTaskCard {
    fn from(task: TaskRecord) -> Self {
        Self {
            id: task.id,
            title: task.title,
            status: task.status,
            priority: task.priority,
            position: task.position,
            seq: task.seq,
            lock_version: task.lock_version,
        }
    }
}
#[derive(Debug, Clone)]
pub struct RealtimeBoardSnapshot {
    pub board_id: String,
    pub tasks: Vec<RealtimeTaskCard>,
}
impl<C: Clock> KanbanService<C> {
    /// 必须先订阅、后读快照。值只表示写入临界区结束提示。
    pub fn subscribe_realtime_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.mutation_gate.subscribe()
    }
    /// 当前 single-host gate 阻止整个分页过程与 canonical 写入交错。
    /// 只可从外部调用；持有 mutation gate 的命令不得嵌套调用此方法。
    pub async fn load_realtime_board(&self, selector: &str) -> Result<RealtimeBoardSnapshot> {
        let _fence = self.mutation_gate.read().await;
        let board = self.get_board(selector).await?;
        if board.archived_at.is_some() {
            return Err(KanbanError::NotFound(format!("看板 {}", board.id)));
        }
        let mut tasks = Vec::new();
        let mut offset = 0usize;
        let mut expected = None;
        let mut bytes = 0usize;
        loop {
            let page = self
                .list_tasks(
                    &board.slug,
                    TaskListOptions {
                        limit: 1_000,
                        offset,
                        include_archived: false,
                        ..TaskListOptions::default()
                    },
                )
                .await?;
            if page.total > 50_000 || expected.is_some_and(|total| total != page.total) {
                return Err(KanbanError::InvalidInput(
                    "看板快照过大或分页总量漂移".into(),
                ));
            }
            expected = Some(page.total);
            let size = page.tasks.len();
            if size == 0 && offset < page.total {
                return Err(KanbanError::Storage("快照分页没有前进".into()));
            }
            for task in page.tasks {
                let card = RealtimeTaskCard::from(task);
                bytes = bytes
                    .checked_add(card.id.len() + card.title.len() + 128)
                    .ok_or_else(|| KanbanError::InvalidInput("快照大小溢出".into()))?;
                if bytes > 16 * 1024 * 1024 {
                    return Err(KanbanError::InvalidInput("快照超过 16 MiB 数据预算".into()));
                }
                tasks.push(card);
            }
            offset += size;
            if offset >= page.total {
                break;
            }
        }
        if tasks.len() != expected.unwrap_or(0) {
            return Err(KanbanError::Storage("快照不完整".into()));
        }
        Ok(RealtimeBoardSnapshot {
            board_id: board.id,
            tasks,
        })
    }
}

impl<C: Clock> KanbanService<C> {
    /// Atlas 的刷新流只验证作用域，不读取整板任务或分页窗口。
    /// 该方法不发通知；调用者不得已持有 mutation gate。
    pub async fn check_realtime_board(&self, board_id: &str) -> Result<()> {
        let _fence = self.mutation_gate.read().await;
        let board = self.get_board(board_id).await?;
        if board.id != board_id || board.archived_at.is_some() {
            return Err(KanbanError::NotFound(format!("看板 {board_id}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
