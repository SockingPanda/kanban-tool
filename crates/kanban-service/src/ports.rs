/// 所有应用能力共享的标记约束。
///
/// 持久化方法位于各个精窄 operation capability trait，而不是这个公共约束中。
/// 具体的 Turso 实现在 `kanban-server` 内适配，使 storage crate 不会泄漏到其他
/// 产品 adapter。
pub trait ApplicationStore: Clone + Send + Sync + 'static {}
