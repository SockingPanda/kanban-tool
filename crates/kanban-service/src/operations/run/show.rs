use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, RunRecord};

impl<C> KanbanService<C>
where
    C: Clock,
{
    /// 按规范全局 id 返回一个 run。
    ///
    /// 这里会有意拒绝 `default#1` 等 selector；该 operation 只接受规范的 `r_...` id。
    pub async fn get_run(&self, run_id: &str) -> Result<RunRecord> {
        let run_id = run_id.trim();
        if !run_id.starts_with("r_") || run_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "run_id must be a global r_... id".to_owned(),
            ));
        }
        self.application
            .store
            .store
            .get_run(run_id)
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_run)
    }
}
