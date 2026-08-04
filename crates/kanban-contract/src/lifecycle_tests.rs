use crate::{
    AddDependencyRequest, ArchiveBoardRequest, ArchiveTaskRequest, BlockTaskRequest,
    ClaimTaskRequest, CompleteTaskRequest, ContractBinding, ContractDirection, ContractGranularity,
    ContractStrictness, EndpointObligation, HeartbeatTaskRequest, MigrationState,
    PromoteTaskRequest, ReclaimTargetStatus, ReclaimTaskRequest, ReleaseTaskRequest,
    ReopenTaskRequest, SpecifyTaskRequest, SubmitReviewTaskRequest, UnblockTaskRequest,
    endpoint_descriptor, operation_inventory,
};
use serde_json::json;

macro_rules! rejects_unknown {
    ($ty:ty, $value:expr) => {
        serde_json::from_value::<$ty>($value)
            .expect_err(concat!(stringify!($ty), " 必须拒绝未知字段"));
    };
}

#[test]
fn lifecycle_requests_preserve_wire_defaults() {
    let claim: ClaimTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(claim.ttl_ms, 300_000);
    assert_eq!(claim.actor, None);
    assert_eq!(claim.worker_profile, None);
    assert_eq!(claim.metadata, None);

    let opaque_metadata = json!([1, {"nested": null}]);
    let claim: ClaimTaskRequest =
        serde_json::from_value(json!({"metadata": opaque_metadata.clone()})).unwrap();
    assert_eq!(claim.metadata, Some(opaque_metadata));

    let complete: CompleteTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert!(!complete.force);
    let review: SubmitReviewTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert!(!review.force);
    let block: BlockTaskRequest =
        serde_json::from_value(json!({"reason": "fixture block"})).unwrap();
    assert!(!block.force);

    let heartbeat: HeartbeatTaskRequest =
        serde_json::from_value(json!({"claim_token": "ct_fixture"})).unwrap();
    assert_eq!(heartbeat.ttl_ms, 300_000);
    let release: ReleaseTaskRequest =
        serde_json::from_value(json!({"claim_token": "ct_fixture"})).unwrap();
    assert_eq!(release.actor, None);

    assert_eq!(
        serde_json::from_value::<PromoteTaskRequest>(json!({})).unwrap(),
        PromoteTaskRequest::default()
    );
    assert_eq!(
        serde_json::from_value::<ReclaimTaskRequest>(json!({})).unwrap(),
        ReclaimTaskRequest::default()
    );
    assert_eq!(
        serde_json::from_value::<UnblockTaskRequest>(json!({})).unwrap(),
        UnblockTaskRequest::default()
    );
    assert_eq!(
        serde_json::from_value::<ArchiveTaskRequest>(json!({})).unwrap(),
        ArchiveTaskRequest::default()
    );
    assert_eq!(
        serde_json::from_value::<ArchiveBoardRequest>(json!({})).unwrap(),
        ArchiveBoardRequest::default()
    );
}

#[test]
fn lifecycle_requests_reject_unknown_outer_fields() {
    rejects_unknown!(SpecifyTaskRequest, json!({"unknown": true}));
    rejects_unknown!(PromoteTaskRequest, json!({"unknown": true}));
    rejects_unknown!(ClaimTaskRequest, json!({"unknown": true}));
    rejects_unknown!(ReclaimTaskRequest, json!({"unknown": true}));
    rejects_unknown!(
        HeartbeatTaskRequest,
        json!({"claim_token": "ct_fixture", "unknown": true})
    );
    rejects_unknown!(
        ReleaseTaskRequest,
        json!({"claim_token": "ct_fixture", "unknown": true})
    );
    rejects_unknown!(CompleteTaskRequest, json!({"unknown": true}));
    rejects_unknown!(SubmitReviewTaskRequest, json!({"unknown": true}));
    rejects_unknown!(
        BlockTaskRequest,
        json!({"reason": "blocked", "unknown": true})
    );
    rejects_unknown!(UnblockTaskRequest, json!({"unknown": true}));
    rejects_unknown!(
        ReopenTaskRequest,
        json!({"reason": "retry", "unknown": true})
    );
    rejects_unknown!(ArchiveTaskRequest, json!({"unknown": true}));
    rejects_unknown!(ArchiveBoardRequest, json!({"unknown": true}));
    rejects_unknown!(
        AddDependencyRequest,
        json!({"parent_task_id": "t_parent", "unknown": true})
    );
}

#[test]
fn required_lifecycle_fields_remain_required() {
    serde_json::from_value::<HeartbeatTaskRequest>(json!({}))
        .expect_err("heartbeat claim_token 必填");
    serde_json::from_value::<ReleaseTaskRequest>(json!({})).expect_err("release claim_token 必填");
    serde_json::from_value::<BlockTaskRequest>(json!({})).expect_err("block reason 必填");
    serde_json::from_value::<ReopenTaskRequest>(json!({})).expect_err("reopen reason 必填");
    serde_json::from_value::<AddDependencyRequest>(json!({})).expect_err("parent_task_id 必填");
}

#[test]
fn lifecycle_release_request_contract() {
    let request: ReleaseTaskRequest = serde_json::from_value(json!({
        "actor": "worker",
        "claim_token": "claim_exact"
    }))
    .unwrap();
    assert_eq!(request.actor.as_deref(), Some("worker"));
    assert_eq!(request.claim_token, "claim_exact");
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({"actor": "worker", "claim_token": "claim_exact"})
    );
}

#[test]
fn submit_review_task_request_contract() {
    let request: SubmitReviewTaskRequest = serde_json::from_value(json!({
        "actor": "worker",
        "claim_token": "claim_exact",
        "force": false,
        "summary": "ready for review"
    }))
    .unwrap();
    assert_eq!(request.actor.as_deref(), Some("worker"));
    assert_eq!(request.claim_token.as_deref(), Some("claim_exact"));
    assert!(!request.force);
    assert_eq!(request.summary.as_deref(), Some("ready for review"));
}

#[test]
fn complete_result_is_opaque_but_submit_review_is_closed() {
    let payload = json!({
        "claim_token": "ct_fixture",
        "result": {"nested": [1, true, null]}
    });
    let complete: CompleteTaskRequest = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(complete.result, Some(payload["result"].clone()));

    serde_json::from_value::<SubmitReviewTaskRequest>(payload)
        .expect_err("submit-review 不拥有 result 字段");
}

#[test]
fn reclaim_target_is_closed_to_service_supported_values() {
    assert_eq!(
        serde_json::from_value::<ReclaimTargetStatus>(json!("ready")).unwrap(),
        ReclaimTargetStatus::Ready
    );
    assert_eq!(
        serde_json::from_value::<ReclaimTargetStatus>(json!("blocked")).unwrap(),
        ReclaimTargetStatus::Blocked
    );
    for unsupported in [
        "triage",
        "todo",
        "scheduled",
        "running",
        "review",
        "done",
        "archived",
    ] {
        serde_json::from_value::<ReclaimTargetStatus>(json!(unsupported))
            .expect_err("reclaim target 不能扩大 core 状态权限");
    }
}

#[cfg(feature = "schema")]
#[test]
fn lifecycle_requests_all_derive_schema() {
    fn assert_schema<T: schemars::JsonSchema>() {}

    assert_schema::<SpecifyTaskRequest>();
    assert_schema::<PromoteTaskRequest>();
    assert_schema::<ClaimTaskRequest>();
    assert_schema::<ReclaimTaskRequest>();
    assert_schema::<HeartbeatTaskRequest>();
    assert_schema::<ReleaseTaskRequest>();
    assert_schema::<CompleteTaskRequest>();
    assert_schema::<SubmitReviewTaskRequest>();
    assert_schema::<BlockTaskRequest>();
    assert_schema::<UnblockTaskRequest>();
    assert_schema::<ReopenTaskRequest>();
    assert_schema::<ArchiveTaskRequest>();
    assert_schema::<ArchiveBoardRequest>();
    assert_schema::<AddDependencyRequest>();
}

const LIFECYCLE_CONTRACTS: &[(&str, &str)] = &[
    ("api.specify-task", "api.specify-task.request"),
    ("api.promote-task", "api.promote-task.request"),
    ("api.claim-task", "api.claim-task.request"),
    ("api.reclaim-task", "api.reclaim-task.request"),
    ("api.heartbeat-task", "api.heartbeat-task.request"),
    ("api.release-task", "api.release-task.request"),
    ("api.complete-task", "api.complete-task.request"),
    ("api.submit-review-task", "api.submit-review-task.request"),
    ("api.block-task", "api.block-task.request"),
    ("api.unblock-task", "api.unblock-task.request"),
    ("api.reopen-task", "api.reopen-task.request"),
    ("api.archive-task", "api.archive-task.request"),
    ("api.archive-board", "api.archive-board.request"),
    ("api.add-dependency", "api.add-dependency.request"),
];

#[test]
fn lifecycle_request_inventory_is_exact_and_adopted() {
    for (operation_id, contract_id) in LIFECYCLE_CONTRACTS {
        let endpoint = endpoint_descriptor(operation_id).expect("endpoint descriptor");
        assert_eq!(
            endpoint.migration,
            MigrationState::Adopted,
            "{operation_id}"
        );
        assert_eq!(
            endpoint.obligations.body,
            EndpointObligation::Contract(contract_id),
            "{operation_id}"
        );
        if matches!(
            *operation_id,
            "api.specify-task"
                | "api.promote-task"
                | "api.reopen-task"
                | "api.unblock-task"
                | "api.archive-task"
        ) {
            assert_eq!(
                endpoint.obligations.path,
                EndpointObligation::Contract(match *operation_id {
                    "api.specify-task" => "api.specify-task.path",
                    "api.promote-task" => "api.promote-task.path",
                    "api.reopen-task" => "api.reopen-task.path",
                    "api.unblock-task" => "api.unblock-task.path",
                    "api.archive-task" => "api.archive-task.path",
                    _ => unreachable!(),
                })
            );
            assert_eq!(
                endpoint.obligations.query,
                EndpointObligation::NotApplicable
            );
            assert_eq!(
                endpoint.obligations.success,
                EndpointObligation::Contract(match *operation_id {
                    "api.specify-task" => "api.specify-task.response",
                    "api.promote-task" => "api.promote-task.response",
                    "api.reopen-task" => "api.reopen-task.response",
                    "api.unblock-task" => "api.unblock-task.response",
                    "api.archive-task" => "api.archive-task.response",
                    _ => unreachable!(),
                })
            );
        } else if matches!(
            *operation_id,
            "api.claim-task"
                | "api.reclaim-task"
                | "api.heartbeat-task"
                | "api.release-task"
                | "api.complete-task"
                | "api.submit-review-task"
                | "api.block-task"
        ) {
            let prefix = operation_id.strip_prefix("api.").unwrap();
            let path = operation_inventory()
                .iter()
                .find(|contract| contract.id == format!("api.{prefix}.path"))
                .expect("B3-C2 path contract");
            let response = operation_inventory()
                .iter()
                .find(|contract| contract.id == format!("api.{prefix}.response"))
                .expect("B3-C2 response contract");
            assert_eq!(
                endpoint.obligations.path,
                EndpointObligation::Contract(path.id)
            );
            assert_eq!(
                endpoint.obligations.query,
                EndpointObligation::NotApplicable
            );
            assert_eq!(
                endpoint.obligations.success,
                EndpointObligation::Contract(response.id)
            );
        } else if *operation_id == "api.add-dependency" {
            assert_eq!(
                endpoint.obligations.path,
                EndpointObligation::Contract("api.add-dependency.path")
            );
            assert_eq!(
                endpoint.obligations.query,
                EndpointObligation::NotApplicable
            );
            assert_eq!(
                endpoint.obligations.success,
                EndpointObligation::Contract("api.add-dependency.response")
            );
        } else if *operation_id == "api.archive-board" {
            assert_eq!(
                endpoint.obligations.path,
                EndpointObligation::Contract("api.archive-board.path")
            );
            assert_eq!(
                endpoint.obligations.query,
                EndpointObligation::NotApplicable
            );
            assert_eq!(
                endpoint.obligations.success,
                EndpointObligation::Contract("api.archive-board.response")
            );
        } else {
            assert_eq!(endpoint.obligations.path, EndpointObligation::Todo);
            assert_eq!(endpoint.obligations.query, EndpointObligation::Todo);
            assert_eq!(endpoint.obligations.success, EndpointObligation::Todo);
        }
        assert_eq!(
            endpoint.obligations.headers,
            EndpointObligation::Contract(Box::leak(
                format!("{operation_id}.headers").into_boxed_str()
            ))
        );
        assert_eq!(endpoint.obligations.sse, EndpointObligation::NotApplicable);

        let contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == *contract_id)
            .expect("lifecycle request contract");
        assert_eq!(contract.surface, crate::ContractSurface::Api);
        assert_eq!(contract.direction, ContractDirection::Deserialize);
        assert_eq!(contract.granularity, ContractGranularity::Exact);
        assert_eq!(contract.strictness, ContractStrictness::DenyUnknownFields);
        assert_eq!(contract.binding, ContractBinding::ExactSurface);
        assert_eq!(contract.migration, MigrationState::Adopted);
        let adoption = contract.adoption.expect("adoption evidence");
        let operation = format!("{:?} {}", endpoint.method, endpoint.path);
        let operation = operation.replacen("Post", "POST", 1);
        assert_eq!(adoption.producer.operation, operation);
        assert_eq!(adoption.consumer.operation, operation);
    }
}

#[test]
fn b3_c2_lifecycle_paths_and_successes_are_exact() {
    let expected = [
        (
            "api.claim-task",
            "api.claim-task.path",
            "api.claim-task.response",
        ),
        (
            "api.reclaim-task",
            "api.reclaim-task.path",
            "api.reclaim-task.response",
        ),
        (
            "api.heartbeat-task",
            "api.heartbeat-task.path",
            "api.heartbeat-task.response",
        ),
        (
            "api.complete-task",
            "api.complete-task.path",
            "api.complete-task.response",
        ),
        (
            "api.submit-review-task",
            "api.submit-review-task.path",
            "api.submit-review-task.response",
        ),
        (
            "api.block-task",
            "api.block-task.path",
            "api.block-task.response",
        ),
    ];
    for (operation_id, path, success) in expected {
        let endpoint = endpoint_descriptor(operation_id).expect("endpoint descriptor");
        assert_eq!(
            endpoint.obligations.path,
            EndpointObligation::Contract(path)
        );
        assert_eq!(
            endpoint.obligations.query,
            EndpointObligation::NotApplicable
        );
        assert_eq!(
            endpoint.obligations.headers,
            EndpointObligation::Contract(Box::leak(
                format!("{operation_id}.headers").into_boxed_str()
            ))
        );
        assert_eq!(
            endpoint.obligations.success,
            EndpointObligation::Contract(success)
        );
    }
}

#[cfg(feature = "schema")]
#[test]
fn lifecycle_request_schema_roots_are_complete() {
    let actual = crate::schema_registry()
        .iter()
        .filter(|root| {
            LIFECYCLE_CONTRACTS
                .iter()
                .any(|(_, contract_id)| *contract_id == root.contract_id)
        })
        .map(|root| root.contract_id)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = LIFECYCLE_CONTRACTS
        .iter()
        .map(|(_, contract_id)| *contract_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
}
