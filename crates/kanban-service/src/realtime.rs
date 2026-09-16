//! 实时投影的 service-owned 一致读取，不暴露数据库句柄或 Protobuf 类型。
use crate::KanbanService;
use kanban_core::Clock;

impl<C: Clock> KanbanService<C> {
    /// 在 canonical 写入共用的 silent fence 内完成整个 typed 查询 future。
    /// 调用者必须先订阅 mutation hint；future 只调用普通 application 只读入口，
    /// 不得嵌套写入或再次获取 fence。取消 future 会立即释放 fence，且不发写提示。
    pub async fn with_realtime_read<T>(&self, query: impl std::future::Future<Output = T>) -> T {
        let _fence = self.mutation_gate.read().await;
        query.await
    }

    /// 必须先订阅、后读快照。值只表示写入临界区结束提示。
    pub fn subscribe_realtime_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.mutation_gate.subscribe()
    }
}

#[cfg(test)]
mod tests;
