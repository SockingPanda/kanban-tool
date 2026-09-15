//! 唯一 Host 持有全部 HTTP/1、HTTP/2 连接任务；取消 serve 会中断连接及响应 body。
use axum::Router;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use std::{future::Future, io, time::Duration};
use tokio::{net::TcpListener, sync::watch, task::JoinSet};

pub(crate) async fn serve<S>(listener: TcpListener, router: Router, shutdown: S) -> io::Result<()>
where
    S: Future<Output = ()> + Send,
{
    let (drain, _) = watch::channel(false);
    // JoinSet 的 Drop 会 abort 全部成员，不把连接生命周期交给 detached spawn。
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result { tracing::warn!(%error, "HTTP 连接任务结束"); }
            }
            accepted = listener.accept() => {
                let (socket, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) if matches!(error.kind(), io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused) => continue,
                    Err(error) => {
                        tracing::warn!(%error, "HTTP accept 失败，等待重试");
                        tokio::select! {
                            _ = &mut shutdown => break,
                            _ = tokio::time::sleep(Duration::from_secs(1)) => continue,
                        }
                    }
                };
                let router = router.clone();
                let mut stopping = drain.subscribe();
                connections.spawn(async move {
                    let builder = Builder::new(TokioExecutor::new());
                    let connection = builder.serve_connection_with_upgrades(
                        TokioIo::new(socket), TowerToHyperService::new(router),
                    );
                    tokio::pin!(connection);
                    let result = tokio::select! {
                        result = &mut connection => result,
                        _ = kanban_rpc_host::wait_stop(&mut stopping) => {
                            connection.as_mut().graceful_shutdown();
                            connection.await
                        }
                    };
                    if let Err(error) = result { tracing::debug!(%error, "HTTP 连接关闭"); }
                });
            }
        }
    }
    drop(listener);
    drain.send_replace(true);
    let drained = tokio::time::timeout(Duration::from_secs(5), async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "HTTP graceful shutdown 超时，已停止全部连接",
        ));
    }
    Ok(())
}
