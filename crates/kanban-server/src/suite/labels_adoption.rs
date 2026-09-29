//! Server 只证明真实路由，不再重复承载静态 DTO 样本。
use kanban_protocol::{CreateBoardLabelResponse, DeleteBoardLabelQuery, DeleteBoardLabelResponse};

#[tokio::test]
async fn delete_board_label_router_returns_removal_report() {
    use crate::test_support::{decode_response, parts, rpc_request};
    use kanban_protocol::rpc::v1 as pb;
    use std::collections::BTreeMap;
    use tower::ServiceExt;
    let directory = tempfile::tempdir().expect("temporary directory");
    let state = crate::AppState::open(directory.path().join("kanban.db"), "adoption")
        .await
        .unwrap();
    let router = crate::build_router(state);
    let create = router
        .clone()
        .oneshot(rpc_request(
            "CreateBoardLabel",
            pb::CreateBoardLabelRequest::from_parts(
                parts(serde_json::json!({"board":"default"})),
                (),
                parts(serde_json::json!({"name":"fixture-delete","color":null})),
            )
            .unwrap(),
            &BTreeMap::new(),
        ))
        .await
        .unwrap();
    let created: CreateBoardLabelResponse =
        decode_response::<pb::CreateBoardLabelResponse, _>(create).await;
    let response = router
        .oneshot(rpc_request(
            "DeleteBoardLabel",
            pb::DeleteBoardLabelRequest::from_parts(
                parts(serde_json::json!({"board":"default","label_id":created.data.id})),
                DeleteBoardLabelQuery { force: false },
                (),
            )
            .unwrap(),
            &BTreeMap::from([("x-kb-actor".into(), "adoption".into())]),
        ))
        .await
        .unwrap();
    let deleted: DeleteBoardLabelResponse =
        decode_response::<pb::DeleteBoardLabelResponse, _>(response).await;
    assert_eq!(deleted.data.label.name, "fixture-delete");
    assert!(!deleted.data.forced);
    assert_eq!(deleted.data.removed_task_bindings, 0);
    assert!(!deleted.data.removed_semantics);
    assert_eq!(deleted.data.removed_atoms, 0);
}
