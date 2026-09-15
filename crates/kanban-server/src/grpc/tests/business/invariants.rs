//! 退役 REST handler 后继续通过原生正式 RPC 验证事务和状态机断言。
use super::*;

async fn ready_task(client: &mut Client, board: &str, name: &str) -> dto::ApiTask {
    let task = create(client, board, name, name).await;
    client
        .mark_execution_plan_not_required(
            pb::MarkExecutionPlanNotRequiredRequest::from_parts(
                input(json!({"task_id":task.id})),
                (),
                input(json!({"reason":"单步执行","actor":"planner"})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let promoted: dto::PromoteTaskResponse = client
        .promote_task(
            pb::PromoteTaskRequest::from_parts(
                input(json!({"task_id":task.id})),
                (),
                input(json!({"actor":"planner"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    promoted.data
}

#[tokio::test]
async fn formal_running_and_review_completion_preserve_tokens_run_and_final_facts() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    for review in [false, true] {
        let task = ready_task(
            &mut client,
            &host.board,
            if review { "review" } else { "done" },
        )
        .await;
        let claim: dto::ClaimTaskResponse = client
            .claim_task(
                pb::ClaimTaskRequest::from_parts(
                    input(json!({"task_id":task.id})),
                    (),
                    input(json!({"actor":"worker","ttl_ms":300000})),
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        for body in [
            json!({"actor":"worker","claim_token":"wrong"}),
            json!({"actor":"other","claim_token":claim.data.claim_token}),
            json!({"actor":"worker","claim_token":format!(" {} ",claim.data.claim_token)}),
        ] {
            let status = if review {
                client
                    .submit_review_task(
                        pb::SubmitReviewTaskRequest::from_parts(
                            input(json!({"task_id":task.id})),
                            (),
                            input(body),
                        )
                        .unwrap(),
                    )
                    .await
                    .unwrap_err()
            } else {
                client
                    .complete_task(
                        pb::CompleteTaskRequest::from_parts(
                            input(json!({"task_id":task.id})),
                            (),
                            input(body),
                        )
                        .unwrap(),
                    )
                    .await
                    .unwrap_err()
            };
            assert_eq!(status.code(), tonic::Code::FailedPrecondition);
            assert_eq!(
                rpc::decode_status(&status).unwrap().code,
                dto::ApiErrorCode::ClaimTokenMismatch
            );
        }
        let expected_version = if review {
            let reviewed: dto::SubmitReviewTaskResponse=client.submit_review_task(pb::SubmitReviewTaskRequest::from_parts(
                input(json!({"task_id":task.id})),(),input(json!({"actor":"worker","claim_token":claim.data.claim_token,"summary":"ready for review"}))).unwrap()).await.unwrap().into_inner().try_into().unwrap();
            assert_eq!(reviewed.data.status, dto::ApiTaskStatus::Review);
            assert!(reviewed.data.claim_owner.is_none());
            assert!(reviewed.data.claim_expires_at.is_none());
            assert!(reviewed.data.last_heartbeat_at.is_none());
            assert!(reviewed.data.completed_at.is_none());
            assert_eq!(
                reviewed.data.current_run_id.as_deref(),
                Some(claim.data.run.id.as_str())
            );
            assert_eq!(reviewed.data.lock_version, claim.data.task.lock_version + 1);
            assert!(
                client
                    .claim_task(
                        pb::ClaimTaskRequest::from_parts(
                            input(json!({"task_id":task.id})),
                            (),
                            input(json!({"actor":"dispatcher","ttl_ms":300000}))
                        )
                        .unwrap()
                    )
                    .await
                    .is_err(),
                "dispatcher 不得 claim review"
            );
            reviewed.data.lock_version + 1
        } else {
            claim.data.task.lock_version + 1
        };
        let body = if review {
            json!({"actor":"reviewer","summary":"approved","result":{"approved":true}})
        } else {
            json!({"actor":"worker","claim_token":claim.data.claim_token,"summary":"approved","result":{"approved":true}})
        };
        let completed: dto::CompleteTaskResponse = client
            .complete_task(
                pb::CompleteTaskRequest::from_parts(
                    input(json!({"task_id":task.id})),
                    (),
                    input(body),
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        assert_eq!(completed.data.status, dto::ApiTaskStatus::Done);
        assert!(completed.data.claim_owner.is_none());
        assert!(completed.data.claim_expires_at.is_none());
        assert!(completed.data.last_heartbeat_at.is_none());
        assert_eq!(completed.data.completed_at, Some(completed.data.updated_at));
        assert_eq!(completed.data.result_summary.as_deref(), Some("approved"));
        assert_eq!(completed.data.result, Some(json!({"approved":true})));
        assert_eq!(
            completed.data.current_run_id.as_deref(),
            Some(claim.data.run.id.as_str())
        );
        assert_eq!(completed.data.lock_version, expected_version);
        let detail: dto::GetTaskDetailsResponse = client
            .get_task_details(
                pb::GetTaskDetailsRequest::from_parts(input(json!({"task_id":task.id})), (), ())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        assert_eq!(detail.data.task, completed.data);
        assert!(
            detail
                .data
                .events
                .iter()
                .any(|event| event.kind == "task.claimed")
        );
        assert!(
            detail
                .data
                .events
                .iter()
                .any(|event| event.kind == "task.completed")
        );
    }
    host.finish().await;
}

#[tokio::test]
async fn formal_dependency_cycle_rollback_and_board_isolation_preserve_complete_graph() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let parent = create(&mut client, &host.board, "parent", "parent").await;
    let child = create(&mut client, &host.board, "child", "child").await;
    let board: dto::CreateBoardResponse = client
        .create_board(
            pb::CreateBoardRequest::from_parts(
                (),
                (),
                input(json!({"slug":"isolated","name":"isolated"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    let isolated = create(&mut client, &board.data.id, "isolated", "isolated").await;
    let request = pb::AddDependencyRequest::from_parts(
        input(json!({"task_id":child.id})),
        (),
        input(json!({"parent_task_id":parent.id,"actor":"writer"})),
    )
    .unwrap();
    let added: dto::AddDependencyResponse = client
        .add_dependency(request.clone())
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(added.data.parents.len(), 1);
    assert_eq!(added.data.parents[0].id, parent.id);
    let replay: dto::AddDependencyResponse = client
        .add_dependency(request)
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(added.data, replay.data);
    let error = client
        .add_dependency(
            pb::AddDependencyRequest::from_parts(
                input(json!({"task_id":parent.id})),
                (),
                input(json!({"parent_task_id":child.id})),
            )
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        rpc::decode_status(&error).unwrap().code,
        dto::ApiErrorCode::DependencyCycle
    );
    let after: dto::ListDependenciesResponse = client
        .list_dependencies(
            pb::ListDependenciesRequest::from_parts(input(json!({"task_id":child.id})), (), ())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(added.data, after.data, "失败的依赖事务不能改变已有图");
    let listed: dto::ListTasksResponse = client
        .list_tasks(
            pb::ListTasksRequest::from_parts(
                input(json!({"board":board.data.id})),
                Default::default(),
                (),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(listed.meta.total, 1);
    assert_eq!(listed.data[0].id, isolated.id);
    assert_eq!(listed.data[0].board_id, board.data.id);
    for _ in 0..2 {
        let removed: dto::RemoveDependencyResponse = client
            .remove_dependency(
                pb::RemoveDependencyRequest::from_parts(
                    input(json!({"child_task_id":child.id,"parent_task_id":parent.id})),
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
        assert!(removed.data.parents.is_empty());
        assert!(removed.data.edges.is_empty());
    }
    host.finish().await;
}

#[tokio::test]
async fn formal_steps_and_comments_keep_entity_local_idempotency_and_plan_lifecycle() {
    let host = Host::start().await;
    let mut client = client(&host).await;
    let first = create(&mut client, &host.board, "first", "first").await;
    let second = create(&mut client, &host.board, "second", "second").await;
    let mut step_ids = Vec::new();
    let mut comment_ids = Vec::new();
    for task in [&first, &second] {
        let request=pb::CreateStepRequest::from_parts(input(json!({"task_id":task.id})),(),input(json!({"idempotency_key":"same-key","title":"step","required":true,"actor":"planner"}))).unwrap();
        let created: dto::CreateStepResponse = client
            .create_step(request.clone())
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        let replay: dto::CreateStepResponse = client
            .create_step(request)
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        assert_eq!(created.data, replay.data);
        assert_eq!(created.data.steps.len(), 1);
        step_ids.push(created.data.steps[0].id.clone());
        let comment = pb::CreateCommentRequest::from_parts(
            input(json!({"task_id":task.id})),
            (),
            input(json!({"idempotency_key":"same-key","body":"comment","author":"writer"})),
        )
        .unwrap();
        let created: dto::CreateCommentResponse = client
            .create_comment(comment.clone())
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        let replay: dto::CreateCommentResponse = client
            .create_comment(comment)
            .await
            .unwrap()
            .into_inner()
            .try_into()
            .unwrap();
        assert_eq!(created.data, replay.data);
        comment_ids.push(created.data.id);
    }
    assert_ne!(step_ids[0], step_ids[1]);
    assert_ne!(comment_ids[0], comment_ids[1]);
    let path = json!({"task_id":first.id,"step_id":step_ids[0]});
    let done: dto::CompleteStepResponse = client
        .complete_step(
            pb::CompleteStepRequest::from_parts(
                input(path.clone()),
                (),
                input(json!({"note":"finished","actor":"operator"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(done.data.steps[0].status, dto::ApiStepStatus::Done);
    assert_eq!(
        done.data.steps[0].resolution_note.as_deref(),
        Some("finished")
    );
    let skipped: dto::SkipStepResponse = client
        .skip_step(
            pb::SkipStepRequest::from_parts(
                input(path.clone()),
                (),
                input(json!({"reason":"not needed","actor":"operator"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(skipped.data.steps[0].status, dto::ApiStepStatus::Skipped);
    let reopened: dto::ReopenStepResponse = client
        .reopen_step(
            pb::ReopenStepRequest::from_parts(
                input(path.clone()),
                (),
                input(json!({"reason":"needs revision","actor":"operator"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert_eq!(reopened.data.steps[0].status, dto::ApiStepStatus::Todo);
    assert!(reopened.data.steps[0].resolution_note.is_none());
    let removed: dto::RemoveStepResponse = client
        .remove_step(pb::RemoveStepRequest::from_parts(input(path), (), ()).unwrap())
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    assert!(removed.data.steps.is_empty());
    assert_eq!(
        removed.data.execution_plan.state,
        dto::ApiExecutionPlanState::Unplanned
    );
    host.finish().await;
}
