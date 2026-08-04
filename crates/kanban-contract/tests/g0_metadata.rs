use kanban_contract::{
    ApiErrorCode, CreatedLabelsMeta, DataEnvelope, LabelOntologyReviewMeta, LimitMeta,
    MetadataEnvelope, NextAfterMeta, OffsetPaginationMeta, OptionalMetadataEnvelope,
    SignalFilterMeta, TaskOntologyDetails, TaskOntologyDetailsMeta, TotalPaginationMeta,
};
use serde_json::json;

#[test]
fn optional_none_omits_meta_and_some_roundtrips() {
    let none =
        serde_json::to_value(OptionalMetadataEnvelope::<_, LimitMeta>::new(1, None)).unwrap();
    assert_eq!(none, json!({"data": 1}));
    let missing =
        serde_json::from_value::<OptionalMetadataEnvelope<i32, LimitMeta>>(json!({"data": 1}))
            .unwrap();
    assert_eq!(missing.meta, None);

    let some = OptionalMetadataEnvelope::new(1, Some(LimitMeta { limit: 10 }));
    let value = serde_json::to_value(&some).unwrap();
    assert_eq!(value, json!({"data": 1, "meta": {"limit": 10}}));
    assert_eq!(
        serde_json::from_value::<OptionalMetadataEnvelope<i32, LimitMeta>>(value).unwrap(),
        some
    );
}

#[test]
fn unknown_fields_and_required_meta_are_rejected() {
    assert!(
        serde_json::from_value::<OptionalMetadataEnvelope<i32, LimitMeta>>(
            json!({"data": 1, "extra": true})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<OptionalMetadataEnvelope<i32, LimitMeta>>(
            json!({"data": 1, "meta": {"limit": 1, "extra": true}})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<MetadataEnvelope<i32, LimitMeta>>(json!({"data": 1})).is_err()
    );
    assert!(
        serde_json::from_value::<TaskOntologyDetailsMeta<String>>(
            json!({"details": {"ontology_summary": "ok", "extra": true}})
        )
        .is_err()
    );
}

#[test]
fn metadata_types_have_exact_wire_keys() {
    assert_eq!(
        serde_json::to_value(OffsetPaginationMeta {
            limit: 1,
            offset: 2
        })
        .unwrap(),
        json!({"limit": 1, "offset": 2})
    );
    assert_eq!(
        serde_json::to_value(TotalPaginationMeta {
            limit: 1,
            offset: 2,
            total: 3
        })
        .unwrap(),
        json!({"limit": 1, "offset": 2, "total": 3})
    );
    assert_eq!(
        serde_json::to_value(LimitMeta { limit: 9 }).unwrap(),
        json!({"limit": 9})
    );
    assert_eq!(
        serde_json::to_value(NextAfterMeta { next_after: 4 }).unwrap(),
        json!({"next_after": 4})
    );
    assert_eq!(
        serde_json::to_value(SignalFilterMeta {
            include_all: true,
            limit: 5
        })
        .unwrap(),
        json!({"include_all": true, "limit": 5})
    );
    assert_eq!(
        serde_json::to_value(LabelOntologyReviewMeta {
            group_by: "x".into(),
            include_all: false,
            limit: 6
        })
        .unwrap(),
        json!({"group_by": "x", "include_all": false, "limit": 6})
    );
    assert_eq!(
        serde_json::to_value(CreatedLabelsMeta {
            created_labels: vec!["x"]
        })
        .unwrap(),
        json!({"created_labels": ["x"]})
    );
    assert_eq!(
        serde_json::to_value(TaskOntologyDetailsMeta {
            details: TaskOntologyDetails {
                ontology_summary: "ok"
            }
        })
        .unwrap(),
        json!({"details": {"ontology_summary": "ok"}})
    );
    let _ = DataEnvelope::new(1);
}

#[cfg(feature = "schema")]
#[test]
fn metadata_types_derive_schema() {
    fn assert_schema<T: schemars::JsonSchema>() {}
    assert_schema::<OptionalMetadataEnvelope<i32, LimitMeta>>();
    assert_schema::<OffsetPaginationMeta>();
    assert_schema::<TotalPaginationMeta>();
    assert_schema::<LimitMeta>();
    assert_schema::<NextAfterMeta>();
    assert_schema::<CreatedLabelsMeta<String>>();
    assert_schema::<SignalFilterMeta>();
    assert_schema::<LabelOntologyReviewMeta>();
    assert_schema::<TaskOntologyDetails<String>>();
    assert_schema::<TaskOntologyDetailsMeta<String>>();

    let optional =
        serde_json::to_value(schemars::schema_for!(OptionalMetadataEnvelope<i32, LimitMeta>))
            .unwrap();
    let optional_root = &optional;
    let optional_required: std::collections::BTreeSet<_> = optional_root
        .get("required")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(optional_required, ["data"].into_iter().collect());
    assert_eq!(
        optional_root.get("additionalProperties").unwrap(),
        &serde_json::json!(false)
    );
    let optional_properties = optional_root
        .get("properties")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(optional_properties.contains_key("data"));
    assert!(optional_properties.contains_key("meta"));

    let required =
        serde_json::to_value(schemars::schema_for!(MetadataEnvelope<i32, LimitMeta>)).unwrap();
    let required_root = &required;
    let required_required: std::collections::BTreeSet<_> = required_root
        .get("required")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(required_required, ["data", "meta"].into_iter().collect());
    assert_eq!(
        required_root.get("additionalProperties").unwrap(),
        &serde_json::json!(false)
    );
    let required_properties = required_root
        .get("properties")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(required_properties.contains_key("data"));
    assert!(required_properties.contains_key("meta"));
}

#[test]
fn data_envelope_has_exact_data_only_wire_shape() {
    let envelope = DataEnvelope::new(json!({"id": 1}));
    let value = serde_json::to_value(&envelope).unwrap();
    assert_eq!(value, json!({"data": {"id": 1}}));
    let roundtrip: DataEnvelope<serde_json::Value> = serde_json::from_value(value).unwrap();
    assert_eq!(roundtrip, envelope);
    assert!(
        serde_json::from_value::<DataEnvelope<serde_json::Value>>(json!({"data": {}, "meta": {}}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<DataEnvelope<serde_json::Value>>(
            json!({"data": {}, "extra": true})
        )
        .is_err()
    );
}

#[test]
fn api_error_code_is_closed_and_uses_snake_case_wire_values() {
    assert_eq!(
        serde_json::to_value(ApiErrorCode::InvalidInput).unwrap(),
        json!("invalid_input")
    );
    assert_eq!(
        serde_json::from_value::<ApiErrorCode>(json!("claim_token_mismatch")).unwrap(),
        ApiErrorCode::ClaimTokenMismatch
    );
    assert!(serde_json::from_value::<ApiErrorCode>(json!("unknown_error")).is_err());
}

#[cfg(feature = "schema")]
#[test]
fn api_error_code_schema_lists_only_stable_wire_values() {
    let schema = serde_json::to_value(schemars::schema_for!(ApiErrorCode)).unwrap();
    assert_eq!(
        schema.get("enum").unwrap(),
        &json!([
            "not_found",
            "conflict",
            "idempotency_conflict",
            "dependency_cycle",
            "invalid_input",
            "feature_not_available",
            "server_unavailable",
            "execution_plan_required",
            "steps_incomplete",
            "claim_token_mismatch",
            "dependency_blocked",
            "claim_conflict",
            "invalid_transition",
            "internal"
        ])
    );
}
