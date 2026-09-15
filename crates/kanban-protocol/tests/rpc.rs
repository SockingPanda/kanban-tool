use std::collections::BTreeSet;

use kanban_protocol::{
    self as dto,
    rpc::{self, v1},
};
use prost::Message;
use serde_json::json;

#[path = "rpc/fixtures.rs"]
mod fixtures;

#[test]
fn every_business_operation_has_a_named_rpc() {
    let manifest: Vec<serde_json::Value> = serde_json::from_str(rpc::METHOD_MANIFEST).unwrap();
    let ids = manifest
        .iter()
        .map(|m| m["operation_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let expected = dto::operation_catalog()
        .iter()
        .filter(|op| op.surface == dto::ContractSurface::Api)
        .map(|op| op.operation_id)
        .chain(["rpc.get-task-details", "rpc.get-label-ontology-quality"])
        .collect::<BTreeSet<_>>();
    assert_eq!(ids, expected);
    assert_eq!(manifest.len(), expected.len());
    let proto = include_str!("../proto/kanban/v1/kanban.proto");
    for item in manifest {
        assert!(proto.contains(&format!("rpc {}(", item["method"].as_str().unwrap())));
    }
}

#[test]
fn patch_missing_clear_and_value_are_distinct() {
    for (field, value) in [
        ("description", json!("new")),
        ("assignee", json!("worker")),
        ("scheduled_at", json!(i64::MAX)),
        ("due_at", json!(i64::MIN)),
        ("max_retries", json!(0)),
        ("metadata", json!({"large":u64::MAX})),
    ] {
        let mut encoded = Vec::new();
        for input in [json!({}), json!({field:null}), json!({field:value})] {
            let original: dto::UpdateTaskRequest = serde_json::from_value(input).unwrap();
            let expected = serde_json::to_value(&original).unwrap();
            let wire = v1::UpdateTaskRequest::from_parts(
                dto::UpdateTaskPath {
                    task_id: "t_one".into(),
                },
                (),
                original,
            )
            .unwrap();
            let bytes = wire.encode_to_vec();
            let (_, (), back) = v1::UpdateTaskRequest::decode(bytes.as_slice())
                .unwrap()
                .decode_parts()
                .unwrap();
            assert_eq!(serde_json::to_value(back).unwrap(), expected);
            encoded.push(bytes);
        }
        assert_ne!(encoded[0], encoded[1], "{field}: missing/clear");
        assert_ne!(encoded[1], encoded[2], "{field}: clear/value");
    }
    let malformed = v1::UpdateTaskRequest {
        task_id: Some("t_one".into()),
        description: Some(v1::PatchString { change: None }),
        ..Default::default()
    };
    assert!(malformed.decode_parts().is_err());
}

#[test]
fn optional_collection_preserves_explicit_empty() {
    let original: dto::UpsertLabelSemanticsRequest =
        serde_json::from_value(json!({"applies_when":[],"actor":"test"})).unwrap();
    let wire = v1::UpsertLabelSemanticsRequest::from_parts(
        dto::LabelSemanticsPath {
            board: "default".into(),
            label_id: "l_one".into(),
        },
        (),
        original,
    )
    .unwrap();
    assert_eq!(wire.applies_when.as_ref().unwrap().items.len(), 0);
    assert!(wire.excludes_when.is_none());
    let (_, (), back) = wire.decode_parts().unwrap();
    assert_eq!(back.applies_when, Some(vec![]));
    assert_eq!(back.excludes_when, None);
}

#[test]
fn explicit_zero_and_false_do_not_trigger_defaults() {
    let base = v1::ClaimTaskRequest {
        task_id: Some("t_one".into()),
        actor: Some("worker".into()),
        ..Default::default()
    };
    assert_eq!(base.clone().decode_parts().unwrap().2.ttl_ms, 300_000);
    assert_eq!(
        v1::ClaimTaskRequest {
            ttl_ms: Some(0),
            ..base
        }
        .decode_parts()
        .unwrap()
        .2
        .ttl_ms,
        0
    );
    let query = v1::ListTasksRequest {
        board: Some("default".into()),
        ..Default::default()
    }
    .decode_parts()
    .unwrap()
    .1;
    assert_eq!(query.limit, dto::DEFAULT_TASK_READ_LIMIT);
    let query = v1::ListTasksRequest {
        board: Some("default".into()),
        limit: Some(0),
        include_archived: Some(false),
        ..Default::default()
    }
    .decode_parts()
    .unwrap()
    .1;
    assert_eq!(query.limit, 0);
    assert!(!query.include_archived);
}

#[test]
fn json_numbers_null_and_containers_roundtrip_exactly() {
    let value = json!({"signed":i64::MIN,"signed_max":i64::MAX,"unsigned":u64::MAX,"float":1.0,"negative_zero":-0.0,"nested":[null,false,{"empty":[]} ]});
    let wire = rpc::encode_json(value.clone()).unwrap();
    let wire = v1::JsonValue::decode(wire.encode_to_vec().as_slice()).unwrap();
    assert_eq!(rpc::decode_json(wire).unwrap(), value);
    assert!(rpc::decode_json(v1::JsonValue { kind: None }).is_err());
    assert!(
        rpc::decode_json(v1::JsonValue {
            kind: Some(v1::json_value::Kind::FloatValue(f64::NAN))
        })
        .is_err()
    );
}

#[test]
fn body_json_missing_and_explicit_null_survive() {
    for value in [
        dto::JsonBodyFieldWire::Missing,
        dto::JsonBodyFieldWire::Present(serde_json::Value::Null),
    ] {
        let expected = value.clone();
        let input: dto::RecordLabelOntologyObservationRequest =
            serde_json::from_value(json!({"actor":{"name":"test","type":"user"},"signals":[]}))
                .unwrap();
        let input = dto::RecordLabelOntologyObservationRequest {
            agent_candidates: value,
            ..input
        };
        let wire = v1::RecordLabelOntologyObservationRequest::from_parts(
            dto::TaskLabelSurfacePath {
                task_id: "t_one".into(),
            },
            rpc::dto::TaskLabelBoardQuery::default(),
            input,
        )
        .unwrap();
        let (_, _, back) = wire.decode_parts().unwrap();
        assert_eq!(back.agent_candidates, expected);
    }
}

#[test]
fn invalid_enum_priority_and_required_presence_are_rejected() {
    let missing = v1::GetBoardRequest::default().decode_parts().unwrap_err();
    let status: tonic::Status = missing.into();
    assert_eq!(
        rpc::decode_status(&status).unwrap().code,
        dto::ApiErrorCode::InvalidInput
    );
    assert!(
        v1::ListTasksRequest {
            board: Some("default".into()),
            status: vec![0],
            ..Default::default()
        }
        .decode_parts()
        .is_err()
    );
    assert!(
        v1::ListTasksRequest {
            board: Some("default".into()),
            status: vec![999],
            ..Default::default()
        }
        .decode_parts()
        .is_err()
    );
    assert!(dto::ApiTaskPriority::try_from(v1::DtoApiTaskPriority { value: Some(4) }).is_err());
    assert!(dto::GetBoardResponse::try_from(v1::GetBoardResponse::default()).is_err());
}

#[test]
fn attachments_move_raw_bytes_and_complete_metadata() {
    let content = vec![0u8, 255, 0, 128, 1];
    let original_ptr = content.as_ptr();
    let input: dto::CreateAttachmentRequest =
        serde_json::from_value(json!({"filename":"资料.bin"})).unwrap();
    let wire = v1::CreateAttachmentRequest::from_parts(
        dto::CreateAttachmentPath {
            task_id: "t_one".into(),
        },
        (),
        dto::CreateAttachmentRequest { content, ..input },
    )
    .unwrap();
    assert_eq!(wire.content.as_ptr(), original_ptr);
    let (_, (), input) = wire.decode_parts().unwrap();
    assert_eq!(input.content.as_ptr(), original_ptr);
    let attachment = dto::ApiAttachment {
        id: "a_one".into(),
        board_id: "b_one".into(),
        task_id: "t_one".into(),
        filename: input.filename,
        rel_path: "attachments/a_one".into(),
        content_type: Some("application/octet-stream".into()),
        size_bytes: 5,
        sha256: Some("abc".into()),
        created_by: "user".into(),
        created_at: i64::MAX,
    };
    let output = rpc::dto::AttachmentDownload::from((attachment.clone(), input.content));
    let wire = v1::DownloadAttachmentResponse::try_from(output).unwrap();
    let back: rpc::dto::AttachmentDownload =
        v1::DownloadAttachmentResponse::decode(wire.encode_to_vec().as_slice())
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(back.attachment, attachment);
    assert_eq!(back.content, [0, 255, 0, 128, 1]);
    assert_eq!(rpc::MAX_MESSAGE_BYTES, 257 * 1024 * 1024);
}

#[test]
fn known_and_future_event_payloads_have_distinct_typed_variants() {
    for (kind, payload) in [
        ("task.created", json!({"status":"triage"})),
        ("task.blocked", json!({"reason":"waiting"})),
        ("task.future", json!({"unsigned":u64::MAX})),
    ] {
        let event = dto::StreamEventData {
            id: i64::MAX,
            event_id: "e_one".into(),
            board_id: "b_one".into(),
            task_id: Some("t_one".into()),
            run_id: None,
            kind: kind.into(),
            actor: None,
            payload: dto::event_payload::EventPayload::from_kind_and_value(kind, payload).unwrap(),
            created_at: i64::MIN,
        };
        let wire = v1::DtoStreamEventData::try_from(event.clone()).unwrap();
        let back: dto::StreamEventData =
            v1::DtoStreamEventData::decode(wire.encode_to_vec().as_slice())
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(back, event);
    }
    let event = dto::StreamEventData {
        id: 1,
        event_id: "e_one".into(),
        board_id: "b_one".into(),
        task_id: Some("t_one".into()),
        run_id: None,
        kind: "task.created".into(),
        actor: None,
        payload: dto::event_payload::EventPayload::Unknown(json!({})),
        created_at: 0,
    };
    let wire = v1::DtoStreamEventData::try_from(event).unwrap();
    assert!(dto::StreamEventData::try_from(wire).is_err());
}

#[derive(Clone, PartialEq, Message)]
struct GoogleRpcStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(message, repeated, tag = "3")]
    details: Vec<GoogleAny>,
}
#[derive(Clone, PartialEq, Message)]
struct GoogleAny {
    #[prost(string, tag = "1")]
    type_url: String,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}

#[test]
fn all_business_errors_use_standard_google_status_any_layout() {
    use dto::ApiErrorCode::*;
    for code in [
        NotFound,
        Conflict,
        IdempotencyConflict,
        DependencyCycle,
        InvalidInput,
        FeatureNotAvailable,
        ServerUnavailable,
        ExecutionPlanRequired,
        StepsIncomplete,
        ClaimTokenMismatch,
        DependencyBlocked,
        ClaimConflict,
        InvalidTransition,
        Internal,
    ] {
        let expected = dto::ErrorBody {
            code,
            message: "明确业务错误".into(),
        };
        let status = rpc::encode_status(expected.clone());
        let standard = GoogleRpcStatus::decode(status.details()).unwrap();
        assert_eq!(standard.code, status.code() as i32);
        assert_eq!(standard.message, status.message());
        assert_eq!(standard.details.len(), 1);
        assert_eq!(
            standard.details[0].type_url,
            "type.googleapis.com/kanban.v1.ErrorDetail"
        );
        assert_eq!(rpc::decode_status(&status), Some(expected));
    }
    let raw = v1::ErrorDetail {
        code: v1::DtoApiErrorCode::Conflict.into(),
        message: "raw detail".into(),
        retryable: false,
    };
    assert_eq!(
        rpc::decode_status(&tonic::Status::with_details(
            tonic::Code::Aborted,
            "raw detail",
            raw.encode_to_vec().into()
        )),
        None
    );
    let wrong = GoogleRpcStatus {
        code: tonic::Code::NotFound as i32,
        message: "mismatch".into(),
        details: vec![],
    };
    assert_eq!(
        rpc::decode_status(&tonic::Status::with_details(
            tonic::Code::Aborted,
            "mismatch",
            wrong.encode_to_vec().into()
        )),
        None
    );
}

#[test]
fn task_scoped_ontology_queries_keep_board_and_status() {
    let query = v1::SuggestTaskLabelsRequest {
        task_id: Some("t_one".into()),
        board: Some("other".into()),
        min_score: Some(0.0),
        ..Default::default()
    }
    .decode_parts()
    .unwrap()
    .1;
    assert_eq!(query.board.as_deref(), Some("other"));
    assert_eq!(query.limit, 5);
    assert_eq!(query.min_score, 0.0);
    let query = v1::ListTaskLabelProposalsRequest {
        task_id: Some("t_one".into()),
        board: Some("b_one".into()),
        status: Some("proposed".into()),
    }
    .decode_parts()
    .unwrap()
    .1;
    assert_eq!(query.status.as_deref(), Some("proposed"));
    assert_eq!(query.board.as_deref(), Some("b_one"));
}

#[test]
fn ontology_quality_and_atom_index_have_typed_results() {
    let quality: dto::cli_labels::CliLabelOntologyQualityOutput = serde_json::from_str(
        include_str!("../../../schemas/fixtures/cli/label-ontology-quality-output.v1.valid.json"),
    )
    .unwrap();
    let expected = quality.clone();
    let wire = v1::GetLabelOntologyQualityResponse::try_from(quality).unwrap();
    let actual: dto::cli_labels::CliLabelOntologyQualityOutput = wire.try_into().unwrap();
    assert_eq!(actual, expected);
    let query = v1::GetLabelOntologyQualityRequest {
        board: Some("default".into()),
        ..Default::default()
    }
    .decode_parts()
    .unwrap()
    .1;
    assert_eq!(query.sample_limit, 20);
    let original = rpc::dto::LabelAtomIndexQueryResponse::new(rpc::dto::LabelAtomIndexQueryData {
        data: vec![],
        degraded: true,
        diagnostics: vec!["vector_provider_unavailable".into()],
    });
    let wire = v1::QueryLabelAtomIndexResponse::try_from(original.clone()).unwrap();
    let back: rpc::dto::LabelAtomIndexQueryResponse = wire.try_into().unwrap();
    assert_eq!(back, original);
}

#[test]
fn task_details_preserve_every_aggregate_section() {
    let task: dto::GetTaskResponse = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/api/get-task-response.v1.valid.json"
    ))
    .unwrap();
    let dependencies: dto::ListDependenciesResponse = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/api/list-dependencies-response.v1.valid.json"
    ))
    .unwrap();
    let steps: dto::ListStepsResponse = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/api/list-steps-response.v1.valid.json"
    ))
    .unwrap();
    let comments: dto::ListCommentsResponse = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/api/list-comments-response.v1.valid.json"
    ))
    .unwrap();
    let runs: dto::ListRunsResponse = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/api/list-runs-response.v1.valid.json"
    ))
    .unwrap();
    let original = dto::GetTaskDetailsResponse {
        data: dto::TaskDetailAggregate {
            labels: task.data.labels.clone(),
            task: task.data,
            dependencies: dependencies.data,
            execution_plan: steps.data.execution_plan,
            steps: steps.data.steps,
            comments: comments.data,
            runs: runs.data,
            events: vec![],
            ontology: dto::TaskDetailOntology {
                summary: None,
                degraded: true,
                diagnostics: vec!["unavailable".into()],
            },
        },
    };
    let wire = v1::GetTaskDetailsResponse::try_from(original.clone()).unwrap();
    let back: dto::GetTaskDetailsResponse =
        v1::GetTaskDetailsResponse::decode(wire.encode_to_vec().as_slice())
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(back, original);
}
