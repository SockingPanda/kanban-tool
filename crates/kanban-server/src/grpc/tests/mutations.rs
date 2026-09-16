//! 正式 mutation 与 QueryService 的完整投影、替换和 dispatcher 事务一致性。
mod lifecycle;

use super::queries::common::Projection;
use super::{Host, query_request};
use kanban_protocol::{self as dto, rpc::v1 as pb};
use pb::{query_definition::Query, query_frame::Body};
use prost::Message;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;
use tonic::{Code, transport::Channel};

type Client = pb::kanban_service_client::KanbanServiceClient<Channel>;
type Stream = tonic::Streaming<pb::QueryFrame>;

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

fn list_query(board: &str) -> Query {
    Query::ListTasks(pb::ListTasksRequest {
        board: Some(board.into()),
        ..Default::default()
    })
}
async fn watch_query(host: &Host, query: Query) -> Stream {
    host.client()
        .await
        .watch_queries(query_request(query))
        .await
        .unwrap()
        .into_inner()
}
async fn watch(host: &Host, board: &str) -> Stream {
    watch_query(host, list_query(board)).await
}
async fn next(stream: &mut Stream, projection: &mut Projection) -> pb::QueryBegin {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(begin) = projection.apply(stream.message().await.unwrap().unwrap()) {
                return begin;
            }
        }
    })
    .await
    .unwrap()
}
async fn missing(stream: &mut Stream) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match stream.message().await.unwrap().unwrap().body.unwrap() {
                Body::Failure(failure) => {
                    assert_eq!(
                        failure.error.unwrap().code,
                        pb::DtoApiErrorCode::NotFound as i32
                    );
                    break;
                }
                Body::Ready(_) | Body::Heartbeat(_) => {}
                other => panic!("缺失的查询应报告 Failure: {other:?}"),
            }
        }
    })
    .await
    .unwrap()
}
fn projected_details(projection: &Projection) -> dto::GetTaskDetailsResponse {
    let Some(pb::query_result::Result::GetTaskDetails(result)) =
        pb::QueryResult::decode(projection.bytes.as_slice())
            .unwrap()
            .result
    else {
        panic!("详情投影类型");
    };
    result.try_into().unwrap()
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
    let mut projection = Projection::default();
    next(&mut surviving, &mut projection).await;
    let initial = projection.cursor.clone().unwrap();
    super::ready(&mut disappearing).await;
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
    next(&mut surviving, &mut projection).await;
    assert_eq!(projection.cursor.as_ref().unwrap().epoch, initial.epoch);
    assert!(projection.cursor.as_ref().unwrap().revision > initial.revision);
    let authoritative = target_client
        .list_tasks(
            pb::ListTasksRequest::from_parts(
                dto::ListTasksPath {
                    board: target.board.clone(),
                },
                Default::default(),
                (),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        projection.bytes,
        pb::QueryResult {
            result: Some(pb::query_result::Result::ListTasks(authoritative))
        }
        .encode_to_vec()
    );
    missing(&mut disappearing).await;
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
    drop((surviving, disappearing));
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
    let mut stream = watch_query(
        &host,
        Query::GetTaskDetails(pb::GetTaskDetailsRequest {
            task_id: Some(task.id.clone()),
        }),
    )
    .await;
    let mut projection = Projection::default();
    next(&mut stream, &mut projection).await;
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
        next(&mut stream, &mut projection).await;
        let snapshot = projected_details(&projection);
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
        next(&mut stream, &mut projection).await;
        let snapshot = projected_details(&projection);
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

#[tokio::test]
async fn board_archive_fails_only_its_query_and_reconnect_keeps_authoritative_scope() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let board = create_board(&mut client, "archive-query").await;
    // GetBoard 的业务契约只接受活动看板；ListTasks 仍允许查询归档看板，不能用它期待 NotFound。
    let board_query = Query::GetBoard(pb::GetBoardRequest {
        board: Some(board.clone()),
    });
    let mut request = query_request(board_query.clone());
    let mut other = query_request(Query::ListBoards(pb::ListBoardsRequest::default()))
        .queries
        .remove(0);
    other.client_query_id = "boards".into();
    request.queries.push(other);
    let mut stream = host
        .client()
        .await
        .watch_queries(request)
        .await
        .unwrap()
        .into_inner();
    let mut ready = std::collections::BTreeSet::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while ready.len() < 2 {
            let frame = stream.message().await.unwrap().unwrap();
            if matches!(frame.body, Some(Body::Ready(_))) {
                ready.insert(frame.client_query_id);
            }
        }
    })
    .await
    .unwrap();
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
    let mut failure = false;
    let mut boards = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !(failure && boards) {
            let frame = stream.message().await.unwrap().unwrap();
            match frame.body.unwrap() {
                Body::Failure(error) => {
                    assert_eq!(frame.client_query_id, "query");
                    assert_eq!(
                        error.error.unwrap().code,
                        pb::DtoApiErrorCode::NotFound as i32
                    );
                    failure = true;
                }
                Body::End(_) => {
                    assert_eq!(frame.client_query_id, "boards");
                    boards = true;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let mut reconnect = watch_query(&host, board_query).await;
    missing(&mut reconnect).await;
    drop((stream, reconnect));
    host.finish().await;
}

#[tokio::test]
async fn non_card_mutation_updates_complete_query_and_failed_cas_keeps_committed_projection() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "完整详情事务").await;
    let mut stream = watch_query(
        &host,
        Query::GetTaskDetails(pb::GetTaskDetailsRequest {
            task_id: Some(task.id.clone()),
        }),
    )
    .await;
    let mut projection = Projection::default();
    next(&mut stream, &mut projection).await;
    comment(&mut client, &task.id, "不改变卡片版本的评论").await;
    next(&mut stream, &mut projection).await;
    let value = projected_details(&projection);
    assert_eq!(value.data.task.lock_version, task.lock_version);
    assert_eq!(value.data.comments[0].body, "不改变卡片版本的评论");
    let update = pb::UpdateTaskRequest::from_parts(
        dto::UpdateTaskPath {
            task_id: task.id.clone(),
        },
        (),
        input(json!({"title":"已提交","expected_lock_version":task.lock_version})),
    )
    .unwrap();
    client.update_task(update.clone()).await.unwrap();
    next(&mut stream, &mut projection).await;
    let before = projection.bytes.clone();
    let cursor = projection.cursor.clone();
    let error = client.update_task(update).await.unwrap_err();
    assert_eq!(
        kanban_protocol::rpc::decode_status(&error).unwrap().code,
        dto::ApiErrorCode::ClaimConflict
    );
    let current = details(&mut client, &task.id).await;
    assert_eq!(current.data.task.title, "已提交");
    assert_eq!(
        serde_json::to_value(current).unwrap(),
        serde_json::to_value(projected_details(&projection)).unwrap()
    );
    let until = tokio::time::sleep(Duration::from_millis(350));
    tokio::pin!(until);
    loop {
        tokio::select! {
            _=&mut until=>break,
            frame=stream.message()=>{
                let frame=frame.unwrap().unwrap();
                assert!(matches!(frame.body,Some(Body::Ready(_)|Body::Heartbeat(_))),"失败 CAS 不得发布新投影");
                projection.apply(frame);
            }
        }
    }
    assert_eq!(projection.bytes, before);
    assert_eq!(projection.cursor, cursor);
    drop(stream);
    host.finish().await;
}
