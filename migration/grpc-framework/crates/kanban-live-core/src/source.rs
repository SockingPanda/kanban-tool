use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use crate::{BoxFuture, Card, Hub, Result};

/// 实现者负责在同一一致性边界内读取完整卡片集合。
/// watch 只是提示，可以合并、重复；不能用它的数值充当 cursor。
pub trait BoardSource: Send + Sync + 'static {
    fn subscribe_changes(&self) -> watch::Receiver<u64>;
    fn load_board<'a>(&'a self, board_id: &'a str) -> BoxFuture<'a, Result<Vec<Card>>>;
}
async fn stopping(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow() { return; }
        if stop.changed().await.is_err() { return; }
    }
}
/// 一个看板只运行一个 pump，所有客户端共享结果。
/// 无独立数据库轮询；读取失败才定时重试，心跳不会触发数据库查询。
pub async fn run_projection(source: Arc<dyn BoardSource>, hub: Arc<Hub>, mut stop: watch::Receiver<bool>) {
    let mut changes = source.subscribe_changes(); // 必须先订阅再读取。
    loop {
        if *stop.borrow() { break; }
        // 先标记看到的通知，再开始读取。读取期间的新通知留给下一轮。
        changes.borrow_and_update();
        let loaded = tokio::select! {
            biased;
            _ = stopping(&mut stop) => break,
            value = tokio::time::timeout(Duration::from_secs(10), source.load_board(hub.board_id())) => value,
        };
        let ok = match loaded {
            Ok(Ok(cards)) => hub.publish(cards).await.is_ok(),
            _ => false,
        };
        if !ok {
            hub.unavailable().await;
            tokio::select! {
                _ = stopping(&mut stop) => break,
                _ = tokio::time::sleep(Duration::from_secs(2)) => continue,
            }
        }
        tokio::select! {
            _ = stopping(&mut stop) => break,
            result = changes.changed() => if result.is_err() { break; },
        }
        // 合并连续写入，且不让持续写入无限延后刷新。
        tokio::select! {
            _ = stopping(&mut stop) => break,
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
    }
    hub.stop().await;
}
