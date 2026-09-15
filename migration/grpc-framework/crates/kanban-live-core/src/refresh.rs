use tokio::sync::watch;
use crate::{BoxFuture, Result};

/// 查询刷新提示源。check_board 必须在所有写入共用的 fence 内验证看板。
/// 通知允许误报与合并；它不包含审计事件，不承诺发生了成功 mutation。
pub trait RefreshSource: Send + Sync + 'static {
    fn subscribe_refreshes(&self) -> watch::Receiver<u64>;
    fn check_board<'a>(&'a self, board_id: &'a str) -> BoxFuture<'a, Result<()>>;
}
