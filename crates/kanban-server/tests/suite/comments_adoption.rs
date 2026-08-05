use crate::common::*;
use std::{fs, path::PathBuf};
fn fx(n: &str) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(n),
        )
        .unwrap(),
    )
    .unwrap()
}
fn norm(mut v: serde_json::Value) -> serde_json::Value {
    fn walk(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if m.contains_key("task_id") && m.contains_key("id") {
                    let note = m.get("body").and_then(|v| v.as_str()) == Some("note");
                    m.insert(
                        "id".into(),
                        json!(if note { "c_note" } else { "c_decision" }),
                    );
                    m.insert("task_id".into(), json!("t_fixture"));
                    m.insert("board_id".into(), json!("b_project"));
                    m.insert("created_at".into(), json!(if note { 1 } else { 2 }));
                }
                for x in m.values_mut() {
                    walk(x)
                }
                if let Some(a) = m.get_mut("data").and_then(|v| v.as_array_mut()) {
                    a.sort_by_key(|x| x.get("body").and_then(|v| v.as_str()) != Some("note"));
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    walk(x)
                }
            }
            _ => {}
        }
    }
    walk(&mut v);
    v
}
async fn produced() -> anyhow::Result<(serde_json::Value, serde_json::Value)> {
    let t = TestApp::new()?;
    let db = t.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    let task = create_ready_task_for_test(&db, "project", "seed", "fixtures")?;
    let app = t.router();
    assert_eq!(
        post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/comments", task.id),
            json!({"author":"alice","body":"note"})
        )
        .await?
        .0,
        StatusCode::CREATED
    );
    let (s, c) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        fx("create-comment-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(s, StatusCode::CREATED);
    let (s, l) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;
    assert_eq!(s, StatusCode::OK);
    Ok((norm(c), norm(l)))
}
#[test]
fn list_comments_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::ListCommentsPath {
            task_id: "t_fixture".into()
        })
        .unwrap(),
        fx("list-comments-path.v1.valid.json")
    )
}
#[tokio::test]
async fn list_comments_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let t = TestApp::new()?;
    let _task = kanban_sqlite::api::create_task(
        t.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("list path"),
    )?;
    let p: kanban_contract::ListCommentsPath =
        serde_json::from_value(fx("list-comments-path.v1.valid.json"))?;
    let (s, v) = get_json(t.router(), &format!("/api/v1/tasks/{}/comments", p.task_id)).await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "not_found");
    Ok(())
}
#[test]
fn create_comment_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::CreateCommentPath {
            task_id: "t_fixture".into()
        })
        .unwrap(),
        fx("create-comment-path.v1.valid.json")
    )
}
#[tokio::test]
async fn create_comment_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let t = TestApp::new()?;
    let _task = kanban_sqlite::api::create_task(
        t.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("create path"),
    )?;
    let p: kanban_contract::CreateCommentPath =
        serde_json::from_value(fx("create-comment-path.v1.valid.json"))?;
    let (s, _) = post_json(
        t.router(),
        &format!("/api/v1/tasks/{}/comments", p.task_id),
        json!({"author":"path","body":"path"}),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn create_comment_request_dto_serializes_to_committed_fixture() {
    let request = kanban_contract::CreateCommentRequest {
        idempotency_key: Some("comment.create:fixture".into()),
        author: Some("codex".into()),
        body: "Choose A".into(),
        kind: Some(kanban_contract::CommentKind::Decision),
        author_type: Some(kanban_contract::CommentAuthorType::Agent),
        agent_type: Some("executor".into()),
        metadata: Some(json!({
            "options": [
                {"slug": "a", "title": "A", "detail": "Choose A"},
                {"slug": "b", "title": "B", "detail": "Choose B"}
            ],
            "selected": "a",
            "reason": "Smaller boundary",
            "risk": "Migration drift",
            "verification": "Contract tests",
            "extension": {"source": "fixture"}
        })),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        fx("create-comment-request.v1.valid.json")
    )
}
#[tokio::test]
async fn create_comment_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let t = TestApp::new()?;
    let db = t.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    let task = create_ready_task_for_test(&db, "project", "seed", "request")?;
    let req = fx("create-comment-request.v1.valid.json");
    let (s, v) = post_json(
        t.router(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        req.clone(),
    )
    .await?;
    assert_eq!(s, StatusCode::CREATED);
    for k in ["author", "body", "kind", "author_type", "agent_type"] {
        assert_eq!(v["data"][k], req[k])
    }
    Ok(())
}
#[tokio::test]
async fn list_comments_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced().await?.1,
        fx("list-comments-response.v1.valid.json")
    );
    Ok(())
}
#[test]
fn list_comments_response_fixture_is_consumed_by_contract_root() {
    let v = fx("list-comments-response.v1.valid.json");
    let d: kanban_contract::ListCommentsResponse = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(d).unwrap(), v)
}
#[tokio::test]
async fn create_comment_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced().await?.0,
        fx("create-comment-response.v1.valid.json")
    );
    Ok(())
}
#[test]
fn create_comment_response_fixture_is_consumed_by_contract_root() {
    let v = fx("create-comment-response.v1.valid.json");
    let d: kanban_contract::CreateCommentResponse = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(d).unwrap(), v)
}
