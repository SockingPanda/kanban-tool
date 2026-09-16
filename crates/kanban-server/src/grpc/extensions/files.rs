use super::{ExtensionRuntime, ResponseStream, response_stream, status};
use kanban_protocol::rpc::extensions as w;
use kanban_service::{
    AttachmentUpload,
    object_model::files::{FileInfo, FileUploadSpec},
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncReadExt,
    sync::{Semaphore, mpsc},
};
use tonic::{Request, Response, Status};

const CHUNK: usize = 64 * 1024;
const IDLE: Duration = Duration::from_secs(20 * 60);
#[derive(Default, Clone)]
pub(super) struct Uploads(Arc<Mutex<UploadMap>>);
#[derive(Default)]
struct UploadMap {
    stopped: bool,
    items: HashMap<String, Arc<Session>>,
}
struct Session {
    spec: FileUploadSpec,
    state: Mutex<Staging>,
}
struct Staging {
    upload: Option<AttachmentUpload>,
    touched: Instant,
}
impl Uploads {
    pub fn stop(&self) {
        if let Ok(mut map) = self.0.lock() {
            map.stopped = true;
            map.items.clear();
        }
    }
    fn find(&self, id: &str) -> Result<Arc<Session>, Status> {
        let mut map = self.0.lock().map_err(status::io)?;
        if map.stopped {
            return Err(status::unavailable("host.stopping"));
        }
        // 上传准入有界。过期会话须使用同一文件标识重新开始。
        Self::prune(&mut map);
        map.items
            .get(id)
            .cloned()
            .ok_or_else(|| status::missing("file.upload_expired"))
    }
    fn prune(map: &mut UploadMap) {
        map.items
            .retain(|_, session| match session.state.try_lock() {
                Ok(state) => state.upload.is_some() && state.touched.elapsed() <= IDLE,
                Err(std::sync::TryLockError::WouldBlock) => true,
                Err(std::sync::TryLockError::Poisoned(_)) => false,
            });
    }
    fn remove(&self, id: &str) -> Result<Option<Arc<Session>>, Status> {
        Ok(self.0.lock().map_err(status::io)?.items.remove(id))
    }
}
#[derive(Clone)]
pub(super) struct Files {
    pub state: crate::AppState,
    pub runtime: ExtensionRuntime,
    pub downloads: Arc<Semaphore>,
    pub chunk_workers: Arc<Semaphore>,
}
fn info(value: FileInfo) -> Result<w::FileInfo, Status> {
    Ok(w::FileInfo {
        id: value.id,
        board_id: value.board_id,
        owner_id: value.owner_id,
        filename: value.filename,
        content_type: value.content_type.unwrap_or_default(),
        size_bytes: u64::try_from(value.size_bytes).map_err(status::io)?,
        sha256: value.sha256.unwrap_or_default(),
        created_by: value.created_by,
        created_at: value.created_at,
        object_version: value.object_version,
    })
}
fn active_output(id: &str, session: &Session) -> Result<w::BeginFileUploadOutput, Status> {
    let mut stage = session
        .state
        .try_lock()
        .map_err(|_| status::unavailable("file.upload_busy"))?;
    let offset = stage
        .upload
        .as_ref()
        .ok_or_else(|| status::missing("file.upload_expired"))?
        .size_bytes();
    stage.touched = Instant::now();
    Ok(w::BeginFileUploadOutput {
        upload_id: id.to_owned(),
        offset,
        chunk_limit: CHUNK as u32,
        completed: None,
    })
}

#[tonic::async_trait]
impl w::file_service_server::FileService for Files {
    async fn begin_file_upload(
        &self,
        request: Request<w::BeginFileUploadInput>,
    ) -> Result<Response<w::BeginFileUploadOutput>, Status> {
        self.runtime.check()?;
        let actor = status::actor(&self.state, request.metadata(), &request.get_ref().actor)?;
        let i = request.into_inner();
        let spec = FileUploadSpec {
            board_id: i.board_id,
            owner_id: i.owner_id,
            file_id: i.file_id,
            filename: i.filename,
            content_type: (!i.content_type.is_empty()).then_some(i.content_type),
            size_bytes: i.size_bytes,
            sha256: i.sha256,
            actor,
        };
        // 校验器在客户端取消后仍持有许可，直到 blocking I/O 结束。
        let io_permit = self
            .downloads
            .clone()
            .try_acquire_owned()
            .map_err(|_| status::exhausted("file.verification_limit"))?;
        let lifetime_permit = self.runtime.permit()?;
        let service = self.state.application().clone();
        let replay_spec = spec.clone();
        let completed = tokio::spawn(async move {
            let _io_permit = io_permit;
            let _lifetime = lifetime_permit;
            service.file_upload_replay(&replay_spec).await
        })
        .await
        .map_err(status::io)?
        .map_err(status::object)?;
        if let Some(completed) = completed {
            return Ok(Response::new(w::BeginFileUploadOutput {
                upload_id: String::new(),
                offset: spec.size_bytes,
                chunk_limit: CHUNK as u32,
                completed: Some(info(completed)?),
            }));
        }
        {
            let mut map = self.runtime.files.0.lock().map_err(status::io)?;
            if map.stopped {
                return Err(status::unavailable("host.stopping"));
            }
            Uploads::prune(&mut map);
            for (id, session) in &map.items {
                if session.spec.file_id == spec.file_id {
                    if session.spec != spec {
                        return Err(status::conflict("file.idempotency_conflict"));
                    }
                    return active_output(id, session).map(Response::new);
                }
            }
            if map.items.len() >= 4 {
                return Err(status::exhausted("file.upload_limit"));
            }
        }
        let upload = self
            .state
            .application()
            .file_begin_upload(&spec)
            .await
            .map_err(status::object)?;
        let mut map = self.runtime.files.0.lock().map_err(status::io)?;
        if map.stopped {
            return Err(status::unavailable("host.stopping"));
        }
        for (id, session) in &map.items {
            if session.spec.file_id == spec.file_id {
                if session.spec != spec {
                    return Err(status::conflict("file.idempotency_conflict"));
                }
                return active_output(id, session).map(Response::new);
            }
        }
        if map.items.len() >= 4 {
            return Err(status::exhausted("file.upload_limit"));
        }
        let id = format!("u_{}", kanban_service::new_task_id());
        let session = Arc::new(Session {
            spec,
            state: Mutex::new(Staging {
                upload: Some(upload),
                touched: Instant::now(),
            }),
        });
        let output = active_output(&id, &session)?;
        map.items.insert(id, session);
        Ok(Response::new(output))
    }
    async fn write_file_chunk(
        &self,
        request: Request<w::WriteFileChunkInput>,
    ) -> Result<Response<w::FileOffsetOutput>, Status> {
        self.runtime.check()?;
        let input = request.into_inner();
        if input.data.is_empty() || input.data.len() > CHUNK {
            return Err(status::invalid("file.chunk_size_invalid"));
        }
        let session = self.runtime.files.find(&input.upload_id)?;
        let lifetime = self.runtime.permit()?;
        let worker = self
            .chunk_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| status::exhausted("file.chunk_workers_busy"))?;
        let offset = tokio::task::spawn_blocking(move || {
            let _lifetime = lifetime;
            let _worker = worker;
            let mut state = session.state.lock().map_err(status::io)?;
            let upload = state
                .upload
                .as_mut()
                .ok_or_else(|| status::missing("file.upload_expired"))?;
            let current = upload.size_bytes();
            if input.offset < current {
                if !upload
                    .matches_chunk(input.offset, &input.data)
                    .map_err(status::io)?
                {
                    return Err(status::conflict("file.chunk_retry_mismatch"));
                }
                state.touched = Instant::now();
                return Ok(current);
            }
            if input.offset != current
                || input
                    .offset
                    .checked_add(input.data.len() as u64)
                    .is_none_or(|end| end > session.spec.size_bytes)
            {
                return Err(status::invalid("file.chunk_offset_invalid"));
            }
            let upload = state
                .upload
                .take()
                .ok_or_else(|| status::missing("file.upload_expired"))?;
            let upload = upload.write_chunk(&input.data).map_err(status::io)?;
            let offset = upload.size_bytes();
            state.upload = Some(upload);
            state.touched = Instant::now();
            Ok::<_, Status>(offset)
        })
        .await
        .map_err(status::io)??;
        Ok(Response::new(w::FileOffsetOutput { offset }))
    }
    async fn finish_file_upload(
        &self,
        request: Request<w::FileUploadIdentity>,
    ) -> Result<Response<w::FileInfo>, Status> {
        self.runtime.check()?;
        let lifetime = self.runtime.permit()?;
        let worker = self
            .chunk_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| status::exhausted("file.chunk_workers_busy"))?;
        let session = self
            .runtime
            .files
            .remove(&request.into_inner().upload_id)?
            .ok_or_else(|| status::missing("file.upload_expired"))?;
        let spec = session.spec.clone();
        let service = self.state.application().clone();
        // 接受提交后，即使响应 future 被丢弃，也持有生命周期直到提交完成。
        // 断线调用方以原文件标识重新 Begin，核对不确定的提交结果。
        let value = tokio::spawn(async move {
            let _lifetime = lifetime;
            let upload = tokio::task::spawn_blocking(move || {
                let _worker = worker;
                let mut state = session.state.lock().map_err(status::io)?;
                let upload = state
                    .upload
                    .take()
                    .ok_or_else(|| status::missing("file.upload_expired"))?;
                if upload.size_bytes() != session.spec.size_bytes {
                    return Err(status::precondition("file.upload_incomplete"));
                }
                upload.seal().map_err(status::io)
            })
            .await
            .map_err(status::io)??;
            let value = service
                .file_commit_upload(&spec, upload)
                .await
                .map_err(status::object)?;
            info(value)
        })
        .await
        .map_err(status::io)??;
        Ok(Response::new(value))
    }

    async fn cancel_file_upload(
        &self,
        request: Request<w::FileUploadIdentity>,
    ) -> Result<Response<w::CancelFileUploadOutput>, Status> {
        let removed = self
            .runtime
            .files
            .remove(&request.into_inner().upload_id)?
            .is_some();
        Ok(Response::new(w::CancelFileUploadOutput { removed }))
    }
    async fn list_object_files(
        &self,
        request: Request<w::FileOwnerInput>,
    ) -> Result<Response<w::FileListOutput>, Status> {
        self.runtime.check()?;
        list(&self.state, request.into_inner())
            .await
            .map(Response::new)
    }

    type DownloadFileStream = ResponseStream<w::FileDownloadFrame>;
    async fn download_file(
        &self,
        request: Request<w::FileIdentityInput>,
    ) -> Result<Response<Self::DownloadFileStream>, Status> {
        self.runtime.check()?;
        let deadline = super::super::deadline::parse(&request.metadata().clone().into_headers())?;
        let i = request.into_inner();
        let permit = self
            .downloads
            .clone()
            .try_acquire_owned()
            .map_err(|_| status::exhausted("file.download_limit"))?;
        let stream_permit = self.runtime.permit()?;
        let service = self.state.application().clone();
        let mut stop = self.runtime.stopped.subscribe();
        let (sender, receiver) = mpsc::channel(2);
        tokio::spawn(async move {
            let _permit = permit;
            let _stream_permit = stream_permit;
            // 校验中的 blocking worker 结束前保留许可，避免取消后重复启动无界磁盘校验。
            let download = service
                .file_download(&i.board_id, &i.owner_id, &i.file_id)
                .await
                .map_err(status::object);
            let work = async {
                let download = download?;
                let expected_size = download.info.size_bytes as u64;
                let expected_hash = download.info.sha256.clone();
                send(
                    &sender,
                    &mut stop,
                    w::file_download_frame::Frame::Header(info(download.info)?),
                )
                .await?;
                let mut file = tokio::fs::File::from_std(download.file);
                let mut offset = 0u64;
                let mut hash = Sha256::new();
                let mut buffer = vec![0; CHUNK];
                loop {
                    if *stop.borrow() || sender.is_closed() {
                        return Ok(());
                    }
                    let n = file.read(&mut buffer).await.map_err(status::io)?;
                    if n == 0 {
                        break;
                    }
                    if offset
                        .checked_add(n as u64)
                        .is_none_or(|end| end > expected_size)
                    {
                        return Err(status::data_loss("file.size_changed"));
                    }
                    hash.update(&buffer[..n]);
                    send(
                        &sender,
                        &mut stop,
                        w::file_download_frame::Frame::Chunk(w::FileDownloadChunk {
                            offset,
                            data: buffer[..n].to_vec(),
                        }),
                    )
                    .await?;
                    offset += n as u64;
                }
                let hash = format!("{:x}", hash.finalize());
                if offset != expected_size || expected_hash.is_some_and(|v| v != hash) {
                    return Err(status::data_loss("file.integrity_failed"));
                }
                send(
                    &sender,
                    &mut stop,
                    w::file_download_frame::Frame::Complete(w::FileDownloadComplete {
                        size_bytes: offset,
                        sha256: hash,
                    }),
                )
                .await
            };
            let result = if let Some(deadline) = deadline {
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep_until(deadline) => {
                        // 下游停止 poll 时也释放 producer 与许可；外层 Deadline 负责 terminal 状态。
                        let _ = sender.try_send(Err(Status::deadline_exceeded("RPC deadline 已到期")));
                        return;
                    }
                    result = work => result,
                }
            } else {
                work.await
            };
            if let Err(error) = result {
                if *stop.borrow() || sender.is_closed() {
                    return;
                }
                tokio::select! { _ = stop.changed() => {}, _ = sender.closed() => {}, _ = sender.send(Err(error)) => {} }
            }
        });
        Ok(Response::new(response_stream(receiver)))
    }
}
async fn send(
    sender: &mpsc::Sender<Result<w::FileDownloadFrame, Status>>,
    stop: &mut tokio::sync::watch::Receiver<bool>,
    frame: w::file_download_frame::Frame,
) -> Result<(), Status> {
    if *stop.borrow() {
        return Err(status::unavailable("host.stopping"));
    }
    tokio::select! {
        _ = stop.changed() => Err(status::unavailable("host.stopping")),
        result = sender.send(Ok(w::FileDownloadFrame { frame: Some(frame) })) => result.map_err(|_| Status::cancelled("file.receiver_closed")),
    }
}

pub(super) async fn list(
    state: &crate::AppState,
    i: w::FileOwnerInput,
) -> Result<w::FileListOutput, Status> {
    let page = state
        .application()
        .file_page(
            &i.board_id,
            &i.owner_id,
            if i.limit == 0 { 100 } else { i.limit },
            &i.after_id,
        )
        .await
        .map_err(status::object)?;
    Ok(w::FileListOutput {
        items: page.items.into_iter().map(info).collect::<Result<_, _>>()?,
        next_id: page.next_id.unwrap_or_default(),
    })
}
