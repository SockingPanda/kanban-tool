//! 对象模型在真实 Turso 上的行为验收。
use super::*;
use serde_json::{Value as Json, json};
use std::sync::Arc;

async fn fixture() -> (tempfile::TempDir, KanbanService) {
    let dir = tempfile::tempdir().expect("temp directory");
    let service = KanbanService::open_with_roots(
        dir.path().join("state.db"),
        None,
        Arc::new(dir.path().join("attachments")),
    )
    .await
    .expect("service initialization");
    (dir, service)
}
async fn send(
    service: &KanbanService,
    request_id: &str,
    mutation: Json,
) -> ObjectResult<ObjectReceipt> {
    let request: ObjectCommand = serde_json::from_value(json!({
        "board_id":"b_default", "actor":"object-model-test", "request_id":request_id, "mutation":mutation
    })).expect("valid test command");
    service.object_execute(request).await
}
async fn create(service: &KanbanService, key: &str, title: &str, properties: Json) -> ObjectRecord {
    let receipt = send(
        service,
        title,
        json!({"operation":"create","object":{
            "type_key":key,"title":title,"body":null,"properties":properties
        }}),
    )
    .await
    .expect("create object");
    service
        .object_get("b_default", &receipt.ids[0])
        .await
        .expect("read object")
}
async fn task(service: &KanbanService) -> ObjectRecord {
    let c = service.store.connection().await.expect("connection");
    c.execute("INSERT INTO tasks(id,board_id,seq,title,status,created_by,created_at,updated_at) VALUES ('t_fixture','b_default',1,'fixture','todo','test',1,1)", ())
        .await.expect("task fixture insertion");
    service
        .object_get("b_default", "t_fixture")
        .await
        .expect("task identity")
}
fn target(o: &ObjectRecord) -> Json {
    json!({"id":o.id,"expected":o.version})
}
async fn define_field(
    service: &KanbanService,
    kind: &str,
    cardinality: &str,
    required: bool,
    type_key: &str,
) {
    let version = service.object_catalog().await.expect("catalog").version;
    send(service,"define-field",json!({"operation":"define_property","expected_catalog_version":version,
        "definition":{"key":"custom.value","name":"测试值","kind":kind,"cardinality":cardinality,"reference_type":null,"options":[]}
    })).await.expect("define field");
    send(service,"bind-field",json!({"operation":"bind_property","expected_catalog_version":version+1,
        "binding":{"type_key":type_key,"property_key":"custom.value","required":required,"position":1,"defaults":[]}
    })).await.expect("bind field");
}

#[test]
fn rejects_wrong_scalar_kind_and_duplicate_references() {
    let property = PropertyDefinition {
        key: "x".into(),
        name: "x".into(),
        kind: PropertyKind::Object,
        cardinality: Cardinality::Many,
        reference_type: None,
        options: vec![],
    };
    assert!(
        validation::scalar_values(&property, &[PropertyValue::Text("x".into())], false).is_err()
    );
    assert!(
        validation::scalar_values(
            &property,
            &[
                PropertyValue::Object("x".into()),
                PropertyValue::Object("x".into())
            ],
            false
        )
        .is_err()
    );
}
#[test]
fn sql_manifest_has_expected_owners_and_no_duplicate_names() {
    let entries = migration::ddl_objects();
    let names = entries
        .iter()
        .map(|(_, name, _)| *name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), entries.len());
    assert_eq!(
        entries
            .iter()
            .filter(|(kind, _, _)| *kind == "table")
            .count(),
        migration::TABLES.len()
    );
    assert!(names.contains("object_task_identity_insert"));
}
#[test]
fn portable_hex_text_is_not_a_blob_tag() {
    let text = json!({"body":"hex:0102"}).as_object().unwrap().clone();
    assert!(portable::validate_wire_record(&text).is_ok());
    let invalid = json!({"body":{"$kanban_blob_hex":"xyz"}})
        .as_object()
        .unwrap()
        .clone();
    assert!(portable::validate_wire_record(&invalid).is_err());
}
#[test]
fn digest_does_not_depend_on_object_key_order() {
    let a: Json = serde_json::from_str(r#"{"b":{"x":1,"a":2},"a":3}"#).unwrap();
    let b: Json = serde_json::from_str(r#"{"a":3,"b":{"a":2,"x":1}}"#).unwrap();
    assert_eq!(queries::digest(&a).unwrap(), queries::digest(&b).unwrap());
}
#[tokio::test]
async fn native_type_creation_needs_no_new_table() {
    let (_dir, s) = fixture().await;
    let version = s.object_catalog().await.unwrap().version;
    send(
        &s,
        "define-type",
        json!({"operation":"define_type","expected_catalog_version":version,
        "definition":{"key":"custom.risk","name":"风险","layout":{}}}),
    )
    .await
    .unwrap();
    define_field(&s, "text", "one", true, "custom.risk").await;
    let missing = send(
        &s,
        "missing",
        json!({"operation":"create","object":{
        "type_key":"custom.risk","title":"缺值","properties":{}}}),
    )
    .await
    .unwrap_err();
    assert_eq!(missing.code, ObjectErrorCode::InvalidArgument);
    let o = create(
        &s,
        "custom.risk",
        "风险一",
        json!({"custom.value":[{"kind":"text","value":"证据"}]}),
    )
    .await;
    assert_eq!(o.type_key, "custom.risk");
}
#[tokio::test]
async fn absent_and_explicit_empty_slots_are_distinct() {
    let (_dir, s) = fixture().await;
    define_field(&s, "text", "many", false, "note").await;
    let o = create(&s, "note", "记录", json!({})).await;
    assert!(!o.properties.contains_key("custom.value"));
    send(
        &s,
        "empty",
        json!({"operation":"patch","patch":{"target":target(&o),
        "edits":[{"action":"set","property_key":"custom.value","values":[]}]}}),
    )
    .await
    .unwrap();
    let changed = s.object_get("b_default", &o.id).await.unwrap();
    assert_eq!(
        changed.properties["custom.value"],
        Vec::<PropertyValue>::new()
    );
}
#[tokio::test]
async fn retry_is_idempotent_and_new_stale_edit_conflicts() {
    let (_dir, s) = fixture().await;
    let mutation =
        json!({"operation":"create","object":{"type_key":"note","title":"幂等","properties":{}}});
    let a = send(&s, "same", mutation.clone()).await.unwrap();
    let b = send(&s, "same", mutation).await.unwrap();
    assert_eq!(a.ids, b.ids);
    assert_eq!(a.event_sequence, b.event_sequence);
    assert!(b.replayed);
    let o = s.object_get("b_default", &a.ids[0]).await.unwrap();
    let edit = json!({"operation":"patch","patch":{"target":target(&o),"title":"改名","edits":[]}});
    send(&s, "edit", edit.clone()).await.unwrap();
    assert_eq!(
        send(&s, "stale", edit).await.unwrap_err().code,
        ObjectErrorCode::Conflict
    );
}
#[tokio::test]
async fn task_source_fields_cannot_be_changed_through_properties() {
    let (_dir, s) = fixture().await;
    let t = task(&s).await;
    let failure=send(&s,"bypass",json!({"operation":"patch","patch":{"target":target(&t),
        "edits":[{"action":"set","property_key":"task.status","values":[{"kind":"select","value":"running"}]}]}})).await.unwrap_err();
    assert_eq!(failure.code, ObjectErrorCode::FailedPrecondition);
    assert_eq!(
        s.object_get("b_default", &t.id).await.unwrap().properties["task.status"],
        vec![PropertyValue::Select("todo".into())]
    );
}

async fn refs(
    s: &KanbanService,
    name: &str,
    root: &ObjectRecord,
    key: &str,
    values: &[String],
    peers: &[ObjectRecord],
) -> ObjectResult<ObjectReceipt> {
    let version = s.object_catalog().await.unwrap().version;
    send(s,name,json!({"operation":"set_references","target":target(root),"property_key":key,"references":values,
        "expected":peers.iter().map(target).collect::<Vec<_>>(),"expected_catalog_version":version})).await
}
async fn fresh(s: &KanbanService, o: &ObjectRecord) -> ObjectRecord {
    s.object_get("b_default", &o.id).await.unwrap()
}
fn dates() -> Json {
    json!({"cycle.starts_at":[{"kind":"date","value":1}],"cycle.ends_at":[{"kind":"date","value":2}]})
}

#[tokio::test]
async fn cycle_relation_is_many_to_one_and_inverse_is_same_edge() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "cycle", "A", dates()).await;
    let b = create(&s, "cycle", "B", dates()).await;
    let t = task(&s).await;
    refs(
        &s,
        "link-a",
        &a,
        "cycle.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let t = fresh(&s, &t).await;
    let a = fresh(&s, &a).await;
    assert_eq!(
        t.properties["task.cycle"],
        vec![PropertyValue::Object(a.id.clone())]
    );
    assert_eq!(
        a.properties["cycle.tasks"],
        vec![PropertyValue::Object(t.id.clone())]
    );
    let failure = refs(
        &s,
        "link-b",
        &b,
        "cycle.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap_err();
    assert_eq!(failure.code, ObjectErrorCode::Conflict);
    assert!(fresh(&s, &b).await.properties["cycle.tasks"].is_empty());
    assert_eq!(fresh(&s, &t).await.version, t.version);
}
#[tokio::test]
async fn module_membership_is_many_to_many() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "module", "A", json!({})).await;
    let b = create(&s, "module", "B", json!({})).await;
    let t = task(&s).await;
    refs(
        &s,
        "a",
        &a,
        "module.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let t = fresh(&s, &t).await;
    refs(
        &s,
        "b",
        &b,
        "module.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let t = fresh(&s, &t).await;
    assert_eq!(t.properties["task.modules"].len(), 2);
    let c = s.store.connection().await.unwrap();
    assert_eq!(
        store::count(
            &c,
            "SELECT COUNT(*) FROM object_relation_edges WHERE relation_key='module_members'",
            vec![]
        )
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        store::count(
            &c,
            "SELECT COUNT(*) FROM object_property_values WHERE kind='object'",
            vec![]
        )
        .await
        .unwrap(),
        0
    );
}
#[tokio::test]
async fn transfer_requires_old_and_new_container_versions_and_is_atomic() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "cycle", "A", dates()).await;
    let b = create(&s, "cycle", "B", dates()).await;
    let t = task(&s).await;
    refs(
        &s,
        "attach",
        &a,
        "cycle.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let a = fresh(&s, &a).await;
    let t = fresh(&s, &t).await;
    let cv = s.object_catalog().await.unwrap().version;
    let change = json!({"operation":"change_relations","change":{"expected_catalog_version":cv,
        "expected":[target(&a),target(&b),target(&t)],
        "remove":[{"relation_key":"cycle_members","source_id":a.id,"target_id":t.id}],
        "add":[{"relation_key":"cycle_members","source_id":b.id,"target_id":t.id}]}});
    let one = send(&s, "transfer", change.clone()).await.unwrap();
    let two = send(&s, "transfer", change).await.unwrap();
    assert!(two.replayed);
    assert_eq!(one.event_sequence, two.event_sequence);
    assert_eq!(
        fresh(&s, &t).await.properties["task.cycle"],
        vec![PropertyValue::Object(b.id.clone())]
    );
    assert!(fresh(&s, &a).await.properties["cycle.tasks"].is_empty());
}
#[tokio::test]
async fn inverse_write_uses_same_engine() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "module", "A", json!({})).await;
    let t = task(&s).await;
    refs(
        &s,
        "inverse",
        &t,
        "task.modules",
        std::slice::from_ref(&a.id),
        std::slice::from_ref(&a),
    )
    .await
    .unwrap();
    assert_eq!(
        fresh(&s, &a).await.properties["module.tasks"],
        vec![PropertyValue::Object(t.id.clone())]
    );
}
#[tokio::test]
async fn acyclic_relation_rejects_cycle_and_rolls_back() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "module", "A", json!({})).await;
    let b = create(&s, "module", "B", json!({})).await;
    refs(
        &s,
        "parent",
        &a,
        "module.children",
        std::slice::from_ref(&b.id),
        std::slice::from_ref(&b),
    )
    .await
    .unwrap();
    let a = fresh(&s, &a).await;
    let b = fresh(&s, &b).await;
    assert!(
        refs(
            &s,
            "backedge",
            &b,
            "module.children",
            std::slice::from_ref(&a.id),
            std::slice::from_ref(&a)
        )
        .await
        .is_err()
    );
    assert!(fresh(&s, &b).await.properties["module.children"].is_empty());
}
#[tokio::test]
async fn generic_scalar_patch_cannot_bypass_relationship_uniqueness() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "cycle", "A", dates()).await;
    let t = task(&s).await;
    let error=send(&s,"illegal",json!({"operation":"patch","patch":{"target":target(&t),"edits":[{"action":"set","property_key":"task.cycle","values":[{"kind":"object","value":a.id}]}]}})).await.unwrap_err();
    assert_eq!(error.code, ObjectErrorCode::FailedPrecondition);
}
#[tokio::test]
async fn custom_type_and_custom_relation_need_no_rust_type_variant() {
    let (_dir, s) = fixture().await;
    let version = s.object_catalog().await.unwrap().version;
    send(&s,"ty",json!({"operation":"define_type","expected_catalog_version":version,"definition":{"key":"custom.release","name":"发布批次","layout":{}}})).await.unwrap();
    send(&s,"rel",json!({"operation":"define_relation","expected_catalog_version":version+1,"definition":{
        "key":"custom.release_items","name":"批次任务","source_type":"custom.release","target_type":"task",
        "source_property":"custom.release.tasks","target_property":"custom.task.release","source_cardinality":"many","target_cardinality":"one","acyclic":false}})).await.unwrap();
    let release = create(&s, "custom.release", "R", json!({})).await;
    let t = task(&s).await;
    refs(
        &s,
        "attach",
        &release,
        "custom.release.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    assert_eq!(
        fresh(&s, &t).await.properties["custom.task.release"],
        vec![PropertyValue::Object(release.id)]
    );
}
#[tokio::test]
async fn workflow_close_freezes_members_and_does_not_mutate_task_state() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "cycle", "A", dates()).await;
    let b = create(&s, "cycle", "B", dates()).await;
    let t = task(&s).await;
    refs(
        &s,
        "attach",
        &a,
        "cycle.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let a = fresh(&s, &a).await;
    send(
        &s,
        "start",
        json!({"operation":"start_workflow","target":target(&a)}),
    )
    .await
    .unwrap();
    let a = fresh(&s, &a).await;
    send(
        &s,
        "close",
        json!({"operation":"close_workflow","target":target(&a),"carry_to":target(&b)}),
    )
    .await
    .unwrap();
    let closure = s
        .object_workflow_closure("b_default", &a.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(closure.members.len(), 1);
    assert_eq!(closure.members[0].carried_to, Some(b.id.clone()));
    let t = fresh(&s, &t).await;
    assert_eq!(
        t.properties["task.cycle"],
        vec![PropertyValue::Object(b.id)]
    );
    assert_eq!(
        t.properties["task.status"],
        vec![PropertyValue::Select("todo".into())]
    );
    assert_eq!(t.version.source, Some(0));
    assert!(s.object_diagnostics("b_default").await.unwrap().is_empty());
}
#[tokio::test]
async fn reference_filter_and_pagination_work_from_both_sides() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "module", "A", json!({})).await;
    let t = task(&s).await;
    refs(
        &s,
        "attach",
        &a,
        "module.tasks",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let a = fresh(&s, &a).await;
    let page = s
        .object_references(ReferenceQuery {
            board_id: "b_default".into(),
            object_id: a.id.clone(),
            property_key: "module.tasks".into(),
            limit: 1,
            after_id: None,
            expected_version: a.version,
            expected_catalog_version: s.object_catalog().await.unwrap().version,
        })
        .await
        .unwrap();
    assert_eq!(page.items[0].id, t.id);
    let result = s
        .object_list(ObjectQuery {
            board_id: "b_default".into(),
            type_key: Some("task".into()),
            text: None,
            filters: vec![PropertyFilter::Contains {
                property_key: "task.modules".into(),
                value: PropertyValue::Object(a.id),
            }],
            include_archived: false,
            limit: 10,
            after: None,
        })
        .await
        .unwrap();
    assert_eq!(result.items.len(), 1);
}
#[tokio::test]
async fn copied_cardinality_flags_cannot_be_forged() {
    let (_dir, s) = fixture().await;
    let a = create(&s, "cycle", "A", dates()).await;
    let t = task(&s).await;
    let c = s.store.connection().await.unwrap();
    let result=c.execute("INSERT INTO object_relation_edges(board_id,relation_key,source_id,target_id,source_type,source_cardinality,target_cardinality,created_at) VALUES ('b_default','cycle_members',?1,?2,'cycle','many','many',1)",(a.id.as_str(),t.id.as_str())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn custom_lifecycle_and_rollup_use_only_catalog_definitions() {
    let (_dir, s) = fixture().await;
    let version = s.object_catalog().await.unwrap().version;
    send(
        &s,
        "delivery-type",
        json!({"operation":"define_type","expected_catalog_version":version,
        "definition":{"key":"custom.delivery","name":"交付窗口","layout":{}}}),
    )
    .await
    .unwrap();
    for (suffix, kind, required) in [
        ("phase", "select", true),
        ("from", "date", true),
        ("until", "date", true),
        ("opened", "date", false),
        ("closed", "date", false),
    ] {
        let key = format!("custom.delivery.{suffix}");
        let version = s.object_catalog().await.unwrap().version;
        let options = if suffix == "phase" {
            json!([
                {"key":"draft","name":"草拟"},{"key":"work","name":"执行"},
                {"key":"finished","name":"结束"},{"key":"aborted","name":"取消"}
            ])
        } else {
            json!([])
        };
        send(&s, &format!("field-{suffix}"), json!({"operation":"define_property","expected_catalog_version":version,
            "definition":{"key":key,"name":suffix,"kind":kind,"cardinality":"one","reference_type":null,"options":options}})).await.unwrap();
        let defaults = if suffix == "phase" {
            json!([{"kind":"select","value":"draft"}])
        } else {
            json!([])
        };
        send(&s, &format!("bind-{suffix}"), json!({"operation":"bind_property","expected_catalog_version":version+1,
            "binding":{"type_key":"custom.delivery","property_key":key,"required":required,"position":10,"defaults":defaults}})).await.unwrap();
    }
    let version = s.object_catalog().await.unwrap().version;
    send(&s,"delivery-relation",json!({"operation":"define_relation","expected_catalog_version":version,"definition":{
        "key":"custom.delivery_items","name":"交付事项","source_type":"custom.delivery","target_type":"task",
        "source_property":"custom.delivery.items","target_property":"custom.task.delivery",
        "source_cardinality":"many","target_cardinality":"one","acyclic":false}})).await.unwrap();
    send(&s,"delivery-workflow",json!({"operation":"define_workflow","expected_catalog_version":version+1,"definition":{
        "key":"custom.delivery_lifecycle","type_key":"custom.delivery","status_property":"custom.delivery.phase",
        "starts_property":"custom.delivery.from","ends_property":"custom.delivery.until",
        "started_property":"custom.delivery.opened","closed_property":"custom.delivery.closed",
        "members_property":"custom.delivery.items","member_status_property":"task.status",
        "planned_value":"draft","active_value":"work","completed_value":"finished","cancelled_value":"aborted",
        "done_values":["done"],"excluded_values":["archived"]}})).await.unwrap();
    send(&s,"delivery-rollup",json!({"operation":"define_rollup","expected_catalog_version":version+2,"definition":{
        "key":"custom.delivery_progress","type_key":"custom.delivery","members_property":"custom.delivery.items",
        "status_property":"task.status","done_values":["done"],"excluded_values":["archived"],"blocked_values":["blocked"]}})).await.unwrap();
    let container = create(&s,"custom.delivery","交付一",json!({
        "custom.delivery.from":[{"kind":"date","value":1}],"custom.delivery.until":[{"kind":"date","value":2}]
    })).await;
    let t = task(&s).await;
    refs(
        &s,
        "delivery-link",
        &container,
        "custom.delivery.items",
        std::slice::from_ref(&t.id),
        std::slice::from_ref(&t),
    )
    .await
    .unwrap();
    let container = fresh(&s, &container).await;
    send(
        &s,
        "delivery-start",
        json!({"operation":"start_workflow","target":target(&container)}),
    )
    .await
    .unwrap();
    let container = fresh(&s, &container).await;
    send(
        &s,
        "delivery-close",
        json!({"operation":"close_workflow","target":target(&container),"carry_to":null}),
    )
    .await
    .unwrap();
    let overview = s.object_overview("b_default", &container.id).await.unwrap();
    assert!(overview.frozen);
    assert_eq!(overview.progress.total, 1);
    assert_eq!(
        overview.object.properties["custom.delivery.phase"],
        vec![PropertyValue::Select("finished".into())]
    );
    assert!(fresh(&s, &t).await.properties["custom.task.delivery"].is_empty());
    assert_eq!(fresh(&s, &t).await.version.source, Some(0));
}
