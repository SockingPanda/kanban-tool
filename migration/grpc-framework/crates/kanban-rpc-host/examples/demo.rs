//! 内存演示 fixture，不打开 Turso，也不代替正式 kanban serve。
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{Mutex, watch};
use kanban_live_core::*;
use kanban_rpc_host::{RpcApp, serve};

struct Demo { cards: Mutex<BTreeMap<String, Card>>, changed: watch::Sender<u64> }
impl BoardSource for Demo {
    fn subscribe_changes(&self) -> watch::Receiver<u64> { self.changed.subscribe() }
    fn load_board<'a>(&'a self, board: &'a str) -> BoxFuture<'a, Result<Vec<Card>>> {
        Box::pin(async move {
            if board != "b_default" { return Err(LiveError::NotFound(board.into())); }
            Ok(self.cards.lock().await.values().cloned().collect())
        })
    }
}
impl TaskCommands for Demo {
    fn update_title(&self, input: UpdateTitle) -> BoxFuture<'_, Result<Card>> {
        Box::pin(async move {
            if input.board_id != "b_default" { return Err(LiveError::NotFound(input.board_id)); }
            let mut cards = self.cards.lock().await;
            let current = cards.get(&input.task_id).ok_or_else(|| LiveError::NotFound(input.task_id.clone()))?;
            if current.lock_version != input.expected_version { return Err(LiveError::Conflict(input.task_id)); }
            let mut next = current.clone();
            next.title = input.title;
            next.lock_version = next.lock_version.checked_add(1).ok_or_else(|| LiveError::Budget("任务版本溢出".into()))?;
            next.validate()?;
            cards.insert(next.id.clone(), next.clone());
            drop(cards);
            self.changed.send_modify(|v| *v = v.wrapping_add(1));
            Ok(next)
        })
    }
}
#[tokio::main]
async fn main() -> kanban_rpc_host::HostResult<()> {
    let card = Card { id: "t_demo".into(), title: "gRPC 实时框架".into(), status: Status::Todo,
        priority: 1, position: 0, seq: 1, lock_version: 0 };
    let (changed, _) = watch::channel(0);
    let demo = Arc::new(Demo { cards: Mutex::new(BTreeMap::from([(card.id.clone(), card)])), changed });
    let hub = Arc::new(Hub::new("b_default", Limits::default())?);
    let (stop_tx, stop_rx) = watch::channel(false);
    // pump 在监听之前订阅 source，并负责唯一的投影发布路径。
    let pump = tokio::spawn(run_projection(demo.clone(), hub.clone(), stop_rx.clone()));
    while hub.snapshot().await.is_err() { tokio::time::sleep(std::time::Duration::from_millis(10)).await; }
    let app = RpcApp::new(vec![hub], demo, vec![
        "http://127.0.0.1:5173".parse()?, "http://localhost:5173".parse()?, "http://127.0.0.1:50051".parse()?,
    ])?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:50051").await?;
    eprintln!("内存 fixture: 127.0.0.1:50051；不连接正式数据库");
    let signal = stop_tx.clone();
    tokio::spawn(async move { let _ = tokio::signal::ctrl_c().await; signal.send_replace(true); });
    let result = serve(listener, app, stop_rx).await;
    stop_tx.send_replace(true);
    pump.await?;
    result
}
