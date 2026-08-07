use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kanban_core::{Clock, SystemClock};
use tokio::sync::Mutex;

use crate::{KanbanError, Result, db::TursoStore};

/// HTTP handler 与进程内 dispatcher 共享的规范 service 入口。
///
/// `TursoStore` 是唯一的 canonical persistence handle；运行日志、附件根目录、时钟和
/// mutation gate 都属于同一个 service 生命周期，避免 host 再创建第二层兼容包装。
#[derive(Clone)]
pub struct KanbanService<C = SystemClock> {
    pub(crate) store: TursoStore,
    pub(crate) run_log_root: Option<Arc<PathBuf>>,
    pub(crate) attachment_root: Option<Arc<PathBuf>>,
    pub(crate) clock: C,
    pub(crate) mutation_gate: Arc<Mutex<()>>,
}

impl KanbanService<SystemClock> {
    /// 在 service boundary 内打开并初始化 host 唯一拥有的 Turso 数据库。
    pub async fn open_with_roots(
        db_path: impl AsRef<Path>,
        run_log_root: Option<Arc<PathBuf>>,
        attachment_root: Arc<PathBuf>,
    ) -> Result<Self> {
        let store = TursoStore::open(db_path)
            .await
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        store
            .initialize()
            .await
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        let mutation_gate = store.mutation_gate();
        Ok(Self {
            store,
            run_log_root,
            attachment_root: Some(attachment_root),
            clock: SystemClock,
            mutation_gate,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(store: TursoStore) -> Self {
        let mutation_gate = store.mutation_gate();
        Self {
            store,
            run_log_root: None,
            attachment_root: None,
            clock: SystemClock,
            mutation_gate,
        }
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    /// 为 service 保留可测试的 Clock seam。
    #[cfg(test)]
    pub(crate) fn with_clock(store: TursoStore, clock: C) -> Self {
        let mutation_gate = store.mutation_gate();
        Self {
            store,
            run_log_root: None,
            attachment_root: None,
            clock,
            mutation_gate,
        }
    }
}
