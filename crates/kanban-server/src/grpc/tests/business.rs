//! 通过真实 Host 验证完整业务契约，而非在同一进程绕过 wire 调用 handler。
mod invariants;

use super::{Host, frame};
use kanban_protocol::{
    self as dto,
    rpc::{self, v1 as pb},
};
use prost::Message;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tonic::{Request, metadata::MetadataValue, transport::Channel};

type Client = pb::kanban_service_client::KanbanServiceClient<Channel>;

async fn client(host: &Host) -> Client {
    Client::connect(host.url.clone())
        .await
        .unwrap()
        .max_decoding_message_size(rpc::MAX_MESSAGE_BYTES)
        .max_encoding_message_size(rpc::MAX_MESSAGE_BYTES)
}

fn input<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

#[test]
fn every_formal_unary_operation_has_an_application_adapter() {
    let manifest: Vec<Value> = serde_json::from_str(rpc::METHOD_MANIFEST).unwrap();
    let expected: std::collections::BTreeSet<_> = manifest
        .iter()
        .map(|entry| entry["request"].as_str().unwrap())
        .collect();
    let actual: std::collections::BTreeSet<_> = crate::grpc::business::REGISTERED_REQUESTS
        .iter()
        .copied()
        .collect();
    assert_eq!(expected.len(), manifest.len());
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn native_health_and_http_bootstrap_share_the_same_report() {
    let host = Host::start().await;
    let report: dto::HealthResponse = client(&host)
        .await
        .get_health(pb::GetHealthRequest::from_parts((), (), ()).unwrap())
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    let bootstrap = reqwest::get(format!("{}/health", host.url))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let bootstrap: dto::HealthResponse = serde_json::from_slice(&bootstrap).unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::to_value(bootstrap).unwrap()
    );
    host.finish().await;
}

async fn create(client: &mut Client, board: &str, title: &str, key: &str) -> dto::ApiTask {
    let response = client.create_task(pb::CreateTaskRequest::from_parts(
        dto::CreateTaskPath { board: board.to_owned() }, (),
        input(json!({"title":title,"description":"完整说明","status":"todo","idempotency_key":key,"actor":"正文作者","metadata":{"wide":9007199254740993_u64}})),
    ).unwrap()).await.unwrap().into_inner();
    dto::CreateTaskResponse::try_from(response).unwrap().data
}

#[tokio::test]
async fn typed_business_preserves_cas_idempotency_null_presence_and_error_codes() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "RPC 完整任务", "task-create").await;
    let replay = create(&mut client, &host.board, "RPC 完整任务", "task-create").await;
    assert_eq!(task.id, replay.id);
    assert_eq!(task.description.as_deref(), Some("完整说明"));
    assert_eq!(task.metadata["wide"], json!(9007199254740993_u64));

    let update = pb::UpdateTaskRequest::from_parts(
        dto::UpdateTaskPath { task_id: task.id.clone() }, (),
        input(json!({"expected_lock_version":task.lock_version,"description":null,"due_at":9007199254740993_i64})),
    ).unwrap();
    let updated: dto::UpdateTaskResponse = client
        .update_task(update.clone())
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(updated.data.description, None);
    assert_eq!(updated.data.title, task.title);
    assert_eq!(updated.data.due_at, Some(9007199254740993_i64));
    let conflict = client.update_task(update).await.unwrap_err();
    assert_eq!(
        rpc::decode_status(&conflict).unwrap().code,
        dto::ApiErrorCode::ClaimConflict
    );

    let invalid = client
        .create_task(pb::CreateTaskRequest::default())
        .await
        .unwrap_err();
    assert_eq!(
        rpc::decode_status(&invalid).unwrap().code,
        dto::ApiErrorCode::InvalidInput
    );
    let absent = client
        .get_task(
            pb::GetTaskRequest::from_parts(
                dto::GetTaskPath {
                    task_id: "t_missing".into(),
                },
                dto::GetTaskQuery::default(),
                (),
            )
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        rpc::decode_status(&absent).unwrap().code,
        dto::ApiErrorCode::NotFound
    );
    host.finish().await;
}

#[tokio::test]
async fn complete_details_include_non_card_writes_and_actor_metadata() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "详情任务", "details").await;
    let mut request = Request::new(pb::CreateCommentRequest::from_parts(
        dto::CreateCommentPath { task_id: task.id.clone() }, (),
        input(json!({"body":"非卡片评论","idempotency_key":"comment-key","metadata":{"wide":18446744073709551615_u64}})),
    ).unwrap());
    request.metadata_mut().insert_bin(
        "x-kb-actor-bin",
        MetadataValue::from_bytes("元数据作者".as_bytes()),
    );
    let comment: dto::CreateCommentResponse = client
        .create_comment(request)
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(comment.data.author, "元数据作者");
    client
        .create_step(
            pb::CreateStepRequest::from_parts(
                dto::CreateStepPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"title":"步骤","required":false,"body":"详细步骤内容"})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    client
        .add_task_label(
            pb::AddTaskLabelRequest::from_parts(
                dto::AddTaskLabelPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"name":"grpc","create_missing":true})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let details: dto::GetTaskDetailsResponse = client
        .get_task_details(
            pb::GetTaskDetailsRequest::from_parts(
                dto::GetTaskPath {
                    task_id: task.id.clone(),
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
    assert_eq!(details.data.comments.len(), 1);
    assert_eq!(details.data.comments[0].body, "非卡片评论");
    assert_eq!(details.data.steps[0].body.as_deref(), Some("详细步骤内容"));
    assert!(details.data.labels.iter().any(|label| label.name == "grpc"));
    assert!(!details.data.events.is_empty());
    let doctor: dto::DoctorResponse = client
        .doctor(pb::DoctorRequest::from_parts((), (), ()).unwrap())
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    let expected = host.state.application().doctor().await.unwrap();
    assert_eq!(doctor.data.ok, expected.ok);
    assert_eq!(doctor.data.integrity_check, expected.integrity_check);
    assert_eq!(doctor.data.consistency_errors, 0);
    host.finish().await;
}

#[tokio::test]
async fn native_claim_heartbeat_release_share_run_and_error_semantics() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "执行任务", "claim").await;
    client
        .mark_execution_plan_not_required(
            pb::MarkExecutionPlanNotRequiredRequest::from_parts(
                dto::MarkExecutionPlanNotRequiredPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"reason":"无必要步骤"})),
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
    let claim: dto::ClaimTaskResponse = client
        .claim_task(
            pb::ClaimTaskRequest::from_parts(
                dto::ClaimTaskPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"actor":"worker","ttl_ms":60000})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(claim.data.task.status, dto::ApiTaskStatus::Running);
    let wrong = client
        .heartbeat_task(
            pb::HeartbeatTaskRequest::from_parts(
                dto::HeartbeatTaskPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"actor":"worker","claim_token":"wrong"})),
            )
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        rpc::decode_status(&wrong).unwrap().code,
        dto::ApiErrorCode::ClaimTokenMismatch
    );
    client
        .heartbeat_task(
            pb::HeartbeatTaskRequest::from_parts(
                dto::HeartbeatTaskPath {
                    task_id: task.id.clone(),
                },
                (),
                input(
                    json!({"actor":"worker","claim_token":claim.data.claim_token,"ttl_ms":60000}),
                ),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let released: dto::ReleaseTaskResponse = client
        .release_task(
            pb::ReleaseTaskRequest::from_parts(
                dto::ReleaseTaskPath {
                    task_id: task.id.clone(),
                },
                (),
                input(json!({"actor":"worker","claim_token":claim.data.claim_token})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(released.data.status, dto::ApiTaskStatus::Ready);
    host.finish().await;
}

#[tokio::test]
async fn grpc_web_unary_uses_binary_protobuf_with_complete_fields() {
    let host = Host::start().await;
    let mut native = client(&host).await;
    let task = create(&mut native, &host.board, "浏览器二进制", "web").await;
    let request = pb::GetTaskDetailsRequest::from_parts(
        dto::GetTaskPath {
            task_id: task.id.clone(),
        },
        (),
        (),
    )
    .unwrap();
    let response = reqwest::Client::new()
        .post(format!(
            "{}/kanban.v1.KanbanService/GetTaskDetails",
            host.url
        ))
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("origin", &host.url)
        .body(frame(request))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes[0], 0);
    let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    let response = pb::GetTaskDetailsResponse::decode(&bytes[5..5 + len]).unwrap();
    let details: dto::GetTaskDetailsResponse = response.try_into().unwrap();
    assert_eq!(details.data.task.id, task.id);
    assert_eq!(details.data.task.description, task.description);
    assert_eq!(bytes[5 + len], 0x80);
    let trailers = std::str::from_utf8(&bytes[10 + len..]).unwrap();
    assert!(trailers.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, value)| key == "grpc-status" && value.trim() == "0")
    }));
    host.finish().await;
}

async fn attachment_roundtrip(size: usize) {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let task = create(&mut client, &host.board, "附件任务", "attachment").await;
    let request = dto::CreateAttachmentRequest {
        id: None,
        filename: "附件.bin".into(),
        content: vec![0xa5; size],
        content_type: Some("application/octet-stream".into()),
        rel_path: None,
        sha256: None,
        actor: Some("附件作者".into()),
    };
    let created: dto::CreateAttachmentResponse = client
        .create_attachment(
            pb::CreateAttachmentRequest::from_parts(
                dto::CreateAttachmentPath {
                    task_id: task.id.clone(),
                },
                (),
                request,
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(created.data.size_bytes, size as i64);
    let downloaded: rpc::dto::AttachmentDownload = client
        .download_attachment(
            pb::DownloadAttachmentRequest::from_parts(
                dto::GetAttachmentPath {
                    task_id: task.id.clone(),
                    attachment_id: created.data.id.clone(),
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
    assert_eq!(downloaded.attachment, created.data);
    assert_eq!(downloaded.content.len(), size);
    assert!(downloaded.content.iter().all(|byte| *byte == 0xa5));
    drop(downloaded);
    client
        .delete_attachment(
            pb::DeleteAttachmentRequest::from_parts(
                dto::DeleteAttachmentPath {
                    task_id: task.id,
                    attachment_id: created.data.id,
                },
                (),
                (),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    host.finish().await;
}

#[tokio::test]
async fn attachment_exceeds_default_grpc_message_limit_without_truncation() {
    attachment_roundtrip(5 * 1024 * 1024).await;
}

#[tokio::test]
#[ignore = "显式验证真实 256 MiB 边界，避免普通单元测试反复分配大消息"]
async fn attachment_exact_256_mib_boundary_roundtrip() {
    attachment_roundtrip(256 * 1024 * 1024).await;
}

#[tokio::test]
#[ignore = "显式运行真实生成 TS 客户端和 Node Fetch"]
async fn generated_business_client_fetches_current_host() {
    let host = Host::start().await;
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/web/build/rpc-fetch-smoke.mjs");
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(40),
        tokio::process::Command::new("node")
            .arg(script)
            .arg(&host.url)
            .arg(&host.board)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "生成客户端失败: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    host.finish().await;
}
