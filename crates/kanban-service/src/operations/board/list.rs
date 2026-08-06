use kanban_core::{Clock, Result};

use crate::{ApplicationHealth, BoardRecord, KanbanService};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn health(&self) -> Result<ApplicationHealth> {
        // 真实的 store query 可以证明已初始化的规范数据库仍可访问，同时不会向 handler
        // 暴露原始连接。
        self.list_boards(true).await?;
        Ok(ApplicationHealth { ok: true })
    }

    pub async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store
            .list_boards(include_archived)
            .await
            .map(|boards| boards.into_iter().map(super::application_board).collect())
            .map_err(crate::error::store_error)
    }
}
