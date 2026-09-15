use crate::common::{Host, input, task_request};
use kanban_client::ClientError;
use kanban_protocol::{self as dto, ApiErrorCode};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn native_client_preserves_presence_cas_idempotency_actor_and_selectors() {
    let host = Host::start().await;
    let client = &host.client;
    assert_eq!(
        client.health().await.unwrap().db_path,
        host.directory.path().join("canonical.db").to_string_lossy()
    );
    let request = task_request("t_client_business");
    let task = client
        .create_task("default", request.clone())
        .await
        .unwrap();
    assert_eq!(
        client
            .create_task("default", request.clone())
            .await
            .unwrap()
            .id,
        task.id
    );
    let changed = dto::CreateTaskRequest {
        title: "不同请求".to_owned(),
        ..request
    };
    assert_eq!(
        client
            .create_task("default", changed)
            .await
            .unwrap_err()
            .code(),
        "idempotency_conflict"
    );
    assert_eq!(task.metadata["wide"], json!(9007199254740993_i64));
    assert_eq!(task.metadata["maximum"], json!(u64::MAX));
    assert_eq!(
        client
            .resolve_task_id("default", &format!("#{}", task.seq))
            .await
            .unwrap(),
        task.id
    );
    let comment = client
        .create_comment(
            &task.id,
            &input(json!({"body": "原生评论", "idempotency_key": "comment-native"})),
        )
        .await
        .unwrap();
    assert_eq!(comment.author, "原生作者");
    client
        .create_step(
            &task.id,
            &input(json!({"title": "必要信息", "required": false, "body": "完整步骤"})),
        )
        .await
        .unwrap();
    let details = client.get_task_details(&task.id).await.unwrap();
    assert_eq!(details.comments[0].body, "原生评论");
    assert_eq!(details.steps.len(), 1);
    let update: dto::UpdateTaskRequest = input(
        json!({"expected_lock_version": details.task.lock_version, "description": null, "due_at": 9007199254740993_i64}),
    );
    let updated = client.update_task(&task.id, &update).await.unwrap();
    assert_eq!(updated.description, None);
    assert_eq!(updated.title, task.title);
    assert_eq!(updated.due_at, Some(9007199254740993_i64));
    assert!(matches!(
        client.update_task(&task.id, &update).await.unwrap_err(),
        ClientError::Api {
            status: 409,
            code: ApiErrorCode::ClaimConflict,
            ..
        }
    ));
    assert!(matches!(
        client.get_task("t_missing").await.unwrap_err(),
        ClientError::Api {
            status: 404,
            code: ApiErrorCode::NotFound,
            ..
        }
    ));
    let result = client
        .list_tasks(
            "default",
            &dto::ListTasksQuery {
                status: vec![updated.status, dto::ApiTaskStatus::Blocked],
                q: Some(task.title.clone()),
                limit: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(result.data[0].id, task.id);
    assert!(
        client
            .list_board_label_proposals("default", Some("proposed"))
            .await
            .unwrap()
            .data
            .is_empty()
    );
    assert!(
        client
            .list_task_label_proposals("default", &task.id, Some("proposed"))
            .await
            .unwrap()
            .data
            .is_empty()
    );
    assert_eq!(
        client
            .list_board_label_proposals("missing", None)
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    host.finish().await;
}

#[tokio::test]
async fn native_claim_returns_run_and_preserves_forbidden_wrong_token() {
    let host = Host::start().await;
    let client = &host.client;
    let task = client
        .create_task("default", task_request("t_native_claim"))
        .await
        .unwrap();
    client
        .mark_execution_plan_not_required(&task.id, &input(json!({"reason": "无步骤"})))
        .await
        .unwrap();
    client
        .promote_task(&task.id, &Default::default())
        .await
        .unwrap();
    let claimed = client
        .claim_task(&task.id, &input(json!({"ttl_ms":60000})))
        .await
        .unwrap();
    assert_eq!(claimed.task.status, dto::ApiTaskStatus::Running);
    assert_eq!(
        client.get_run(&claimed.run.id).await.unwrap().id,
        claimed.run.id
    );
    let wrong = client
        .heartbeat_task(&task.id, &input(json!({"claim_token":"wrong"})))
        .await
        .unwrap_err();
    assert!(matches!(
        wrong,
        ClientError::Api {
            status: 403,
            code: ApiErrorCode::ClaimTokenMismatch,
            ..
        }
    ));
    client
        .heartbeat_task(
            &task.id,
            &input(json!({"claim_token":claimed.claim_token,"ttl_ms":60000})),
        )
        .await
        .unwrap();
    let released = client
        .release_task(&task.id, &input(json!({"claim_token":claimed.claim_token})))
        .await
        .unwrap();
    assert_eq!(released.status, dto::ApiTaskStatus::Ready);
    host.finish().await;
}

async fn attachment_roundtrip(bytes: usize) {
    let host = Host::start().await;
    let task = host
        .client
        .create_task("default", task_request("t_native_attachment"))
        .await
        .unwrap();
    let request = dto::CreateAttachmentRequest {
        id: Some("a_native_bytes".to_owned()),
        filename: "资料.bin".to_owned(),
        content: vec![0xa7; bytes],
        content_type: Some("application/octet-stream".to_owned()),
        rel_path: None,
        sha256: None,
        actor: None,
    };
    let attachment = host
        .client
        .create_attachment(&task.id, &request)
        .await
        .unwrap();
    assert_eq!(attachment.size_bytes, i64::try_from(bytes).unwrap());
    assert_eq!(attachment.created_by, "原生作者");
    let downloaded = host
        .client
        .download_attachment(&task.id, &attachment.id)
        .await
        .unwrap();
    assert_eq!(downloaded.content, request.content);
    assert_eq!(
        downloaded.attachment_id.as_deref(),
        Some(attachment.id.as_str())
    );
    assert_eq!(downloaded.sha256, attachment.sha256);
    assert_eq!(
        downloaded.content_type.as_deref(),
        Some("application/octet-stream")
    );
    assert!(
        host.client
            .delete_attachment(&task.id, &attachment.id)
            .await
            .unwrap()
    );
    assert!(
        host.client
            .list_attachments(&task.id)
            .await
            .unwrap()
            .is_empty()
    );
    host.finish().await;
}

#[tokio::test]
async fn attachments_cross_the_default_tonic_four_mib_limit() {
    attachment_roundtrip(5 * 1024 * 1024).await;
}

#[tokio::test]
#[ignore = "完整附件上限验证，单独运行以控制内存和磁盘占用"]
async fn attachment_exact_256_mib_boundary_roundtrip() {
    tokio::time::timeout(
        Duration::from_secs(90),
        attachment_roundtrip(256 * 1024 * 1024),
    )
    .await
    .unwrap();
}
