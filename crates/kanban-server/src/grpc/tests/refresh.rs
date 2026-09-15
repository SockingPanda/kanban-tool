//! 正式 WorkspaceService 的真实网络验证；REST 只覆盖尚未退出的迁移写入口。
mod lifecycle;
mod resources;

use super::Host;
use kanban_protocol::{self as dto, rpc::v1 as pb};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;
use tonic::{Code, transport::Channel};

type Client = pb::kanban_service_client::KanbanServiceClient<Channel>;
type Stream = tonic::Streaming<pb::WorkspaceChangeFrame>;

async fn client(host: &Host) -> Client {
    Client::connect(host.url.clone())
        .await
        .unwrap()
        .max_encoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
        .max_decoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
}

fn input<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

async fn create(client: &mut Client, board: &str, title: &str) -> dto::ApiTask {
    let response: dto::CreateTaskResponse = client
        .create_task(
            pb::CreateTaskRequest::from_parts(
                dto::CreateTaskPath {
                    board: board.into(),
                },
                (),
                input(json!({"title": title, "description":"G06 验证任务", "status": "todo"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    response.data
}

async fn create_board(client: &mut Client, slug: &str) -> String {
    let response: dto::CreateBoardResponse = client
        .create_board(
            pb::CreateBoardRequest::from_parts((), (), input(json!({"slug": slug, "name": slug})))
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    response.data.id
}

async fn watch(host: &Host, board: &str) -> Stream {
    pb::workspace_service_client::WorkspaceServiceClient::connect(host.url.clone())
        .await
        .unwrap()
        .watch_changes(pb::WatchChangesRequest {
            board_id: board.into(),
            protocol_version: 1,
        })
        .await
        .unwrap()
        .into_inner()
}

async fn next(stream: &mut Stream) -> pb::WorkspaceChangeFrame {
    tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

async fn details(client: &mut Client, id: &str) -> dto::GetTaskDetailsResponse {
    client
        .get_task_details(
            pb::GetTaskDetailsRequest::from_parts(dto::GetTaskPath { task_id: id.into() }, (), ())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap()
}

async fn comment(client: &mut Client, id: &str, body: &str) {
    client
        .create_comment(
            pb::CreateCommentRequest::from_parts(
                dto::CreateCommentPath { task_id: id.into() },
                (),
                input(json!({"body": body})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
}

fn assert_hint(frame: &pb::WorkspaceChangeFrame, epoch: &str, sequence: u64) {
    assert_eq!(frame.epoch, epoch);
    assert_eq!(frame.sequence, sequence);
    assert!(matches!(
        frame.body,
        Some(pb::workspace_change_frame::Body::Invalidated(
            pb::QueriesInvalidated { reason: 2 }
        ))
    ));
}

#[tokio::test]
async fn formal_watch_refreshes_rest_and_native_non_card_writes() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "混合入口").await;
    let mut stream = watch(&host, &host.board).await;
    let first = next(&mut stream).await;
    let http = reqwest::Client::new();

    let response = http
        .post(format!("{}/api/v1/tasks/{}/comments", host.url, task.id))
        .header("content-type", "application/json")
        .body(r#"{"author":"rest","author_type":"agent","kind":"note","body":"REST 评论"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    assert_hint(&next(&mut stream).await, &first.epoch, 2);
    assert_eq!(
        details(&mut client, &task.id).await.data.comments[0].body,
        "REST 评论"
    );

    client
        .add_task_label(
            pb::AddTaskLabelRequest::from_parts(
                dto::AddTaskLabelPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"name":"native-label", "create_missing":true})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_hint(&next(&mut stream).await, &first.epoch, 3);
    assert!(
        details(&mut client, &task.id)
            .await
            .data
            .labels
            .iter()
            .any(|value| value.name == "native-label")
    );

    let response = http
        .post(format!("{}/api/v1/tasks/{}/steps", host.url, task.id))
        .header("content-type", "application/json")
        .body(r#"{"title":"REST 步骤","body":"刷新步骤详情","required":false}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    assert_hint(&next(&mut stream).await, &first.epoch, 4);
    let after_step = details(&mut client, &task.id).await;
    assert_eq!(after_step.data.steps[0].title, "REST 步骤");

    let attachment: dto::CreateAttachmentResponse = client
        .create_attachment(
            pb::CreateAttachmentRequest::from_parts(
                dto::CreateAttachmentPath {
                    task_id: task.id.clone(),
                },
                (),
                dto::CreateAttachmentRequest {
                    id: None,
                    filename: "refresh.txt".into(),
                    content: b"native attachment".to_vec(),
                    content_type: Some("text/plain".into()),
                    rel_path: None,
                    sha256: None,
                    actor: None,
                },
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_hint(&next(&mut stream).await, &first.epoch, 5);
    let downloaded: kanban_protocol::rpc::dto::AttachmentDownload = client
        .download_attachment(
            pb::DownloadAttachmentRequest::from_parts(
                dto::GetAttachmentPath {
                    task_id: task.id.clone(),
                    attachment_id: attachment.data.id,
                },
                (),
                (),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(downloaded.content, b"native attachment");
    let final_task = details(&mut client, &task.id).await.data.task;
    assert_eq!(final_task.title, task.title);
    assert_eq!(final_task.status, after_step.data.task.status);
    assert!(
        host.state.grpc_probe.loads().is_empty(),
        "正式刷新不读取整板卡片"
    );
    drop(stream);
    host.finish().await;
}

#[tokio::test]
async fn failed_cas_may_refresh_but_preserves_business_facts() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "原始内容").await;
    let mut stream = watch(&host, &host.board).await;
    let attached = next(&mut stream).await;
    let update = pb::UpdateTaskRequest::from_parts(
        dto::UpdateTaskPath {
            task_id: task.id.clone(),
        },
        (),
        input(json!({"title":"已提交", "expected_lock_version":task.lock_version})),
    )
    .unwrap();
    client.update_task(update.clone()).await.unwrap();
    assert_hint(&next(&mut stream).await, &attached.epoch, 2);
    let before = details(&mut client, &task.id).await;
    let conflict = client.update_task(update).await.unwrap_err();
    assert_eq!(
        kanban_protocol::rpc::decode_status(&conflict).unwrap().code,
        dto::ApiErrorCode::ClaimConflict
    );
    // gate 退出提示允许冗余；帧没有 task 业务值，随后查询必须仍是已提交的版本。
    assert_hint(&next(&mut stream).await, &attached.epoch, 3);
    let after = details(&mut client, &task.id).await;
    assert_eq!(
        serde_json::to_value(after).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    drop(stream);
    host.finish().await;
}

#[tokio::test]
async fn formal_attach_keeps_a_write_during_initial_board_check() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "初始化竞争").await;
    let pause = host.state.grpc_probe.pause_next_check();
    let mut workspace =
        pb::workspace_service_client::WorkspaceServiceClient::connect(host.url.clone())
            .await
            .unwrap();
    let request = pb::WatchChangesRequest {
        board_id: host.board.clone(),
        protocol_version: 1,
    };
    let attaching =
        tokio::spawn(async move { workspace.watch_changes(request).await.unwrap().into_inner() });
    tokio::time::timeout(Duration::from_secs(5), pause.entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    comment(&mut client, &task.id, "验证开始后提交").await;
    pause.release.add_permits(1);
    let mut stream = attaching.await.unwrap();
    let first = next(&mut stream).await;
    assert_hint(&next(&mut stream).await, &first.epoch, 2);
    assert_eq!(
        details(&mut client, &task.id).await.data.comments[0].body,
        "验证开始后提交"
    );
    drop(stream);
    host.finish().await;
}

#[tokio::test]
async fn board_archive_closes_formal_stream_and_rejects_reconnect() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let board = create_board(&mut client, "archive-watch").await;
    let mut stream = watch(&host, &board).await;
    next(&mut stream).await;
    client
        .archive_board(
            pb::ArchiveBoardRequest::from_parts(
                dto::ArchiveBoardPath {
                    board: board.clone(),
                },
                (),
                dto::ArchiveBoardRequest { actor: None },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code(), Code::NotFound);
    let mut workspace =
        pb::workspace_service_client::WorkspaceServiceClient::connect(host.url.clone())
            .await
            .unwrap();
    assert_eq!(
        workspace
            .watch_changes(pb::WatchChangesRequest {
                board_id: board,
                protocol_version: 1
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    host.finish().await;
}

#[tokio::test]
async fn portable_replace_refreshes_surviving_board_and_ends_removed_scope() {
    let source = Host::start().await;
    let mut source_client = client(&source).await;
    let imported = create(&mut source_client, &source.board, "导入后任务").await;
    let portable = source._directory.path().join("portable.jsonl");
    source_client
        .maintenance_export(
            pb::MaintenanceExportRequest::from_parts(
                (),
                (),
                dto::MaintenancePathRequest {
                    path: portable.to_string_lossy().into_owned(),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();

    let target = Host::start().await;
    assert_eq!(target.board, source.board, "默认看板 ID 是稳定的替换作用域");
    let mut target_client = client(&target).await;
    let old = create(&mut target_client, &target.board, "将被替换").await;
    let removed = create_board(&mut target_client, "removed-by-import").await;
    let mut surviving = watch(&target, &target.board).await;
    let mut disappearing = watch(&target, &removed).await;
    let initial = next(&mut surviving).await;
    next(&mut disappearing).await;
    let report: dto::ImportResponse = target_client
        .maintenance_import(
            pb::MaintenanceImportRequest::from_parts(
                (),
                (),
                dto::MaintenanceImportRequest {
                    path: portable.to_string_lossy().into_owned(),
                    replace: true,
                },
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(report.data.phase, "completed");
    assert!(!report.data.restart_required);
    assert_hint(&next(&mut surviving).await, &initial.epoch, 2);
    let error = tokio::time::timeout(Duration::from_secs(5), disappearing.message())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code(), Code::NotFound);
    assert_eq!(
        details(&mut target_client, &imported.id)
            .await
            .data
            .task
            .title,
        imported.title
    );
    assert_eq!(
        target_client
            .get_task(
                pb::GetTaskRequest::from_parts(
                    dto::GetTaskPath { task_id: old.id },
                    dto::GetTaskQuery::default(),
                    (),
                )
                .unwrap()
            )
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    drop(surviving);
    target.finish().await;
    source.finish().await;
}

#[tokio::test]
async fn dispatcher_claim_heartbeat_and_release_refresh_the_formal_stream() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "dispatcher 写入口").await;
    client
        .mark_execution_plan_not_required(
            pb::MarkExecutionPlanNotRequiredRequest::from_parts(
                dto::MarkExecutionPlanNotRequiredPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"reason":"仅验证 dispatcher 生命周期"})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    client
        .promote_task(
            pb::PromoteTaskRequest::from_parts(
                dto::PromoteTaskPath {
                    task_id: task.id.clone(),
                },
                (),
                dto::PromoteTaskRequest::default(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut stream = watch(&host, &host.board).await;
    next(&mut stream).await;
    let profile = host._directory.path().join("dispatcher.toml");
    tokio::fs::write(
        &profile,
        r#"
board = "default"
command = "sleep 0.4; exit 1"
poll_interval_ms = 5
claim_ttl_ms = 5000
heartbeat_interval_ms = 25
on_failure = "ready"
log_dir = "runs"
"#,
    )
    .await
    .unwrap();
    let config = crate::DispatcherConfig::load(&profile).await.unwrap();
    let (stop, receiver) = tokio::sync::watch::channel(crate::ShutdownSignal::Running);
    let address = host.url.strip_prefix("http://").unwrap().parse().unwrap();
    let dispatcher = tokio::spawn(crate::dispatcher::run_dispatcher(
        host.state.clone(),
        config,
        address,
        receiver,
    ));
    loop {
        next(&mut stream).await;
        let snapshot = details(&mut client, &task.id).await;
        if snapshot.data.task.status == dto::ApiTaskStatus::Running {
            assert!(
                snapshot
                    .data
                    .events
                    .iter()
                    .any(|event| event.kind == "task.claimed")
            );
            stop.send_replace(crate::ShutdownSignal::Graceful);
            break;
        }
    }
    let released = loop {
        next(&mut stream).await;
        let snapshot = details(&mut client, &task.id).await;
        if snapshot.data.task.status == dto::ApiTaskStatus::Ready {
            break snapshot;
        }
    };
    for kind in ["task.claimed", "task.heartbeat", "task.released"] {
        assert!(
            released.data.events.iter().any(|event| event.kind == kind),
            "缺少 dispatcher {kind}"
        );
    }
    tokio::time::timeout(Duration::from_secs(5), dispatcher)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    drop(stream);
    host.finish().await;
}
