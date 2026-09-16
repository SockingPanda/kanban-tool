//! 对象和文件 RPC 复用现有 Host、来源校验与关闭边界。
mod files;
mod objects;
pub(super) use objects::read_query;
mod status;
#[cfg(test)]
mod tests;

use crate::AppState;
use axum::Router;
use kanban_protocol::rpc::extensions as wire;
use std::sync::Arc;
use tokio::sync::{Semaphore, watch};
use tower::Layer;

#[derive(Clone)]
pub(super) struct ExtensionRuntime {
    stopped: watch::Sender<bool>,
    streams: Arc<Semaphore>,
    files: files::Uploads,
}
impl ExtensionRuntime {
    pub fn begin_shutdown(&self) {
        self.stopped.send_replace(true);
        self.files.stop();
    }
    pub async fn stop(&self) {
        self.begin_shutdown();
        // producer 独立观察关闭信号，不依赖下游继续读取。
        let _all = self.streams.acquire_many(32).await;
    }
    pub(super) fn check(&self) -> Result<(), tonic::Status> {
        if *self.stopped.borrow() {
            Err(status::unavailable("host.stopping"))
        } else {
            Ok(())
        }
    }
    fn permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, tonic::Status> {
        let permit = self
            .streams
            .clone()
            .try_acquire_owned()
            .map_err(|_| status::exhausted("host.stream_limit"))?;
        // 先占用生命周期再检查关闭状态，关闭屏障不会漏掉并发进入的 worker。
        self.check()?;
        Ok(permit)
    }
}

pub(super) fn mount(state: &AppState) -> (Router, ExtensionRuntime) {
    let runtime = ExtensionRuntime {
        stopped: watch::channel(false).0,
        streams: Arc::new(Semaphore::new(32)),
        files: files::Uploads::default(),
    };
    let objects = wire::object_service_server::ObjectServiceServer::new(objects::Objects {
        state: state.clone(),
        runtime: runtime.clone(),
    })
    .max_decoding_message_size(2 * 1024 * 1024)
    .max_encoding_message_size(8 * 1024 * 1024);
    let files = wire::file_service_server::FileServiceServer::new(files::Files {
        state: state.clone(),
        runtime: runtime.clone(),
        downloads: Arc::new(Semaphore::new(4)),
        chunk_workers: Arc::new(Semaphore::new(8)),
    })
    .max_decoding_message_size(128 * 1024)
    .max_encoding_message_size(4 * 1024 * 1024);
    let router = Router::new()
        .route_service(
            "/kanban.extensions.v1.ObjectService/*method",
            tonic_web::GrpcWebLayer::new().layer(super::deadline::Deadline::new(objects)),
        )
        .route_service(
            "/kanban.extensions.v1.FileService/*method",
            tonic_web::GrpcWebLayer::new().layer(super::deadline::Deadline::new(files)),
        );
    (router, runtime)
}

pub(super) type ResponseStream<T> =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<T, tonic::Status>> + Send + 'static>>;
pub(super) fn response_stream<T: Send + 'static>(
    receiver: tokio::sync::mpsc::Receiver<Result<T, tonic::Status>>,
) -> ResponseStream<T> {
    Box::pin(futures_util::stream::unfold(
        receiver,
        |mut receiver| async move { receiver.recv().await.map(|value| (value, receiver)) },
    ))
}
