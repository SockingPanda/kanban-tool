use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use kanban_core::{Clock, SystemClock};
use tokio::sync::Mutex;

use crate::{ApplicationStore, KanbanError, Result, TursoApplicationStore};

/// HTTP handler 与进程内 dispatcher 共享的规范 command/query 入口。
#[derive(Debug, Clone)]
pub struct ApplicationService<S, C = SystemClock> {
    pub(crate) store: S,
    pub(crate) clock: C,
    pub(crate) mutation_gate: Arc<Mutex<()>>,
}

/// host-facing 的规范 service。
///
/// 当前仍以 [`ApplicationService`] 承载尚未完成扁平化的 operation。这个兼容核心只在
/// service crate 内装配；后续领域迁移完成后会逐步删除 generic store abstraction。
#[derive(Clone)]
pub struct KanbanService<C = SystemClock> {
    pub(crate) application: ApplicationService<TursoApplicationStore, C>,
}

impl KanbanService<SystemClock> {
    /// 在 service boundary 内打开并初始化 host 唯一拥有的 Turso 数据库。
    pub async fn open_with_roots(
        db_path: impl AsRef<Path>,
        run_log_root: Option<Arc<PathBuf>>,
        attachment_root: Arc<PathBuf>,
    ) -> Result<Self> {
        let store = TursoApplicationStore::open_with_roots(db_path, run_log_root, attachment_root)
            .await
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(Self::new(store))
    }

    pub fn new(store: TursoApplicationStore) -> Self {
        Self {
            application: ApplicationService::new(store),
        }
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    /// 为 application service 保留可测试的 Clock seam。
    pub fn with_clock(store: TursoApplicationStore, clock: C) -> Self {
        Self {
            application: ApplicationService::with_clock(store, clock),
        }
    }
}

impl<C> Deref for KanbanService<C> {
    type Target = ApplicationService<TursoApplicationStore, C>;

    fn deref(&self) -> &Self::Target {
        &self.application
    }
}

impl<S> ApplicationService<S, SystemClock>
where
    S: ApplicationStore,
{
    pub fn new(store: S) -> Self {
        Self {
            store,
            clock: SystemClock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub fn with_clock(store: S, clock: C) -> Self {
        Self {
            store,
            clock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
    }
}
