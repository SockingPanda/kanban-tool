use crate::common::*;
use std::{collections::BTreeMap, fs, path::PathBuf};

fn fixture(name: &str) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn normalized_response(mut value: serde_json::Value) -> serde_json::Value {
    let data = value["data"].as_object_mut().unwrap();
    data.insert("id".into(), json!("t_fixture"));
    data.insert("board_id".into(), json!("b_project"));
    data.insert("ref".into(), json!("project#2"));
    data.insert("created_at".into(), json!(1));
    data.insert("updated_at".into(), json!(1));
    for label in data["labels"].as_array_mut().unwrap() {
        let label = label.as_object_mut().unwrap();
        label.insert("id".into(), json!("l_core"));
        label.insert("board_id".into(), json!("b_project"));
        label.insert("created_at".into(), json!(1));
        label.insert("updated_at".into(), json!(1));
    }
    value
}

fn request_dto() -> kanban_contract::CreateTaskRequest {
    kanban_contract::CreateTaskRequest {
        title: "Contract child".into(),
        description: Some("Exact create request".into()),
        status: Some(kanban_contract::ApiCreateTaskStatus::Ready),
        assignee: Some("worker-a".into()),
        priority: 1,
        scheduled_at: None,
        due_at: None,
        max_retries: Some(2),
        metadata: Some(BTreeMap::from([(
            "extension".into(),
            json!({"source":"fixture"}),
        )])),
        labels: vec!["core".into()],
        depends_on: vec!["project#1".into()],
        actor: Some("alice".into()),
    }
}

async fn produced_response() -> anyhow::Result<serde_json::Value> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    kanban_sqlite::api::create_label(
        &db,
        "project",
        kanban_sqlite::api::CreateLabel {
            name: "core".into(),
            color: None,
        },
    )?;
    kanban_sqlite::api::create_task(
        &db,
        "project",
        "seed",
        kanban_sqlite::api::CreateTask::ready("parent"),
    )?;
    let (status, response) = post_json(
        test.router(),
        "/api/v1/boards/project/tasks",
        fixture("create-task-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    Ok(normalized_response(response))
}

#[test]
fn create_task_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::CreateTaskPath {
            board: "project".into()
        })
        .unwrap(),
        fixture("create-task-path.v1.valid.json")
    );
}

#[tokio::test]
async fn create_task_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::CreateTaskPath =
        serde_json::from_value(fixture("create-task-path.v1.valid.json"))?;
    let (status, response) = post_json(
        TestApp::new()?.router(),
        &format!("/api/v1/boards/{}/tasks", path.board),
        json!({"title":"missing board"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(response["error"]["code"], "not_found");
    Ok(())
}

#[test]
fn create_task_request_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(request_dto()).unwrap(),
        fixture("create-task-request.v1.valid.json")
    );
}

#[tokio::test]
async fn create_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let response = produced_response().await?;
    let data = &response["data"];
    assert_eq!(data["board_slug"], "project");
    assert_eq!(data["created_by"], "alice");
    assert_eq!(data["priority"], 1);
    assert_eq!(data["max_retries"], 2);
    assert_eq!(data["metadata"], json!({"extension":{"source":"fixture"}}));
    assert_eq!(
        data["status"], "todo",
        "ready must degrade through service guards"
    );
    assert_eq!(data["unfinished_parent_count"], 1);
    assert_eq!(data["labels"][0]["name"], "core");
    Ok(())
}

#[tokio::test]
async fn create_task_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced_response().await?,
        fixture("create-task-response.v1.valid.json")
    );
    Ok(())
}

#[test]
fn create_task_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("create-task-response.v1.valid.json");
    let response: kanban_contract::CreateTaskResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);

    let invalid = fixture("create-task-response.v1.invalid.json");
    assert_eq!(
        invalid["data"].as_object().unwrap().len(),
        value["data"].as_object().unwrap().len() + 1,
        "privacy fixture must be complete and add only claim_token"
    );
    for (key, expected) in value["data"].as_object().unwrap() {
        assert_eq!(&invalid["data"][key], expected, "field {key} drifted");
    }
    let error = serde_json::from_value::<kanban_contract::CreateTaskResponse>(invalid)
        .expect_err("claim_token must be rejected as an unknown private field");
    assert!(error.to_string().contains("claim_token"), "{error}");
}

#[tokio::test]
async fn create_task_wire_rejects_invalid_status_and_non_object_metadata() -> anyhow::Result<()> {
    for body in [
        json!({"title":"bad status","status":"running"}),
        json!({"title":"bad metadata","metadata":[]}),
    ] {
        let (status, response) = post_json(
            TestApp::new()?.router(),
            "/api/v1/boards/default/tasks",
            body,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response["error"]["code"], "invalid_input");
    }
    Ok(())
}

#[tokio::test]
async fn create_task_request_preserves_missing_defaults_and_explicit_nulls() -> anyhow::Result<()> {
    let (status, response) = post_json(
        TestApp::new()?.router(),
        "/api/v1/boards/default/tasks",
        json!({
            "title":"minimal",
            "description":null,
            "status":null,
            "assignee":null,
            "scheduled_at":null,
            "due_at":null,
            "max_retries":null,
            "metadata":null,
            "actor":null
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["data"]["priority"], 3);
    assert_eq!(response["data"]["metadata"], json!({}));
    assert_eq!(response["data"]["labels"], json!([]));
    Ok(())
}

fn validate_create_task_handler_baseline(source: &str) -> Vec<String> {
    use syn::visit::Visit;
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let Some(function) = file.items.iter().find_map(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == "create_task" => Some(function),
        _ => None,
    }) else {
        return vec!["create_task count".into()];
    };
    struct Audit {
        types: Vec<String>,
        paths: Vec<String>,
        fields: Vec<String>,
    }
    impl<'a> Visit<'a> for Audit {
        fn visit_type_path(&mut self, value: &'a syn::TypePath) {
            self.types.extend(
                value
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string()),
            );
            syn::visit::visit_type_path(self, value);
        }
        fn visit_path(&mut self, value: &'a syn::Path) {
            self.paths.push(
                value
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
            syn::visit::visit_path(self, value);
        }
        fn visit_expr_field(&mut self, value: &'a syn::ExprField) {
            if let syn::Member::Named(name) = &value.member {
                self.fields.push(name.to_string())
            }
            syn::visit::visit_expr_field(self, value);
        }
    }
    let mut audit = Audit {
        types: vec![],
        paths: vec![],
        fields: vec![],
    };
    audit.visit_item_fn(function);
    let mut violations = Vec::new();
    for required in ["CreateTaskPath", "CreateTaskRequest", "CreateTaskResponse"] {
        if !audit
            .types
            .iter()
            .chain(audit.paths.iter())
            .any(|value| value.ends_with(required))
        {
            violations.push(required.into())
        }
    }
    if !audit
        .paths
        .iter()
        .any(|value| value.contains("ApiCreateTaskStatus::"))
    {
        violations.push("ApiCreateTaskStatus".into())
    }
    if audit
        .types
        .iter()
        .any(|value| value == "CreateTaskBody" || value == "DataEnvelope")
    {
        violations.push("private body/envelope".into())
    }
    if audit
        .paths
        .iter()
        .filter(|path| {
            path.as_str() == "kanban_sqlite::api::create_task_with_labels_and_dependencies"
        })
        .count()
        != 1
    {
        violations.push("service call".into())
    }
    for required in [
        "board",
        "title",
        "description",
        "status",
        "assignee",
        "priority",
        "scheduled_at",
        "due_at",
        "max_retries",
        "metadata",
        "labels",
        "depends_on",
        "actor",
    ] {
        if !audit.fields.iter().any(|field| field == required) {
            violations.push(format!("field {required}"))
        }
    }
    for status in ["Triage", "Todo", "Scheduled", "Ready"] {
        if !audit
            .paths
            .iter()
            .any(|path| path.ends_with(&format!("ApiCreateTaskStatus::{status}")))
        {
            violations.push(format!("status {status}"))
        }
    }
    violations
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn field_path(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Path(value) => Some(path_name(&value.path)),
        syn::Expr::Field(value) => {
            let base = field_path(&value.base)?;
            let syn::Member::Named(member) = &value.member else {
                return None;
            };
            Some(format!("{base}.{member}"))
        }
        syn::Expr::Reference(value) => field_path(&value.expr).map(|value| format!("&{value}")),
        syn::Expr::Paren(value) => field_path(&value.expr),
        _ => None,
    }
}

fn local<'a>(statement: &'a syn::Stmt, name: &str) -> Option<&'a syn::Expr> {
    let syn::Stmt::Local(local) = statement else {
        return None;
    };
    let syn::Pat::Ident(pattern) = &local.pat else {
        return None;
    };
    (pattern.ident == name).then(|| local.init.as_ref().map(|init| init.expr.as_ref()))?
}

fn struct_fields<'a>(
    expression: &'a syn::Expr,
    expected: &str,
) -> Option<std::collections::BTreeMap<String, &'a syn::Expr>> {
    let syn::Expr::Struct(value) = expression else {
        return None;
    };
    if path_name(&value.path) != expected || value.rest.is_some() {
        return None;
    }
    value
        .fields
        .iter()
        .map(|field| match &field.member {
            syn::Member::Named(name) => Some((name.to_string(), &field.expr)),
            _ => None,
        })
        .collect()
}

fn try_inner(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Try(value) => &value.expr,
        _ => expression,
    }
}

fn call<'a>(expression: &'a syn::Expr, expected: &str) -> Option<&'a syn::ExprCall> {
    let syn::Expr::Call(value) = try_inner(expression) else {
        return None;
    };
    let syn::Expr::Path(function) = value.func.as_ref() else {
        return None;
    };
    (path_name(&function.path) == expected).then_some(value)
}

fn method_call(expression: &syn::Expr, receiver: &str, method: &str) -> bool {
    matches!(
        expression,
        syn::Expr::MethodCall(value)
            if value.method == method
                && value.args.is_empty()
                && field_path(&value.receiver).as_deref() == Some(receiver)
    )
}

fn exact_actor_expression(expression: &syn::Expr) -> bool {
    let Some(actor_call) = call(expression, "actor") else {
        return false;
    };
    if actor_call.args.len() != 3 {
        return false;
    }
    let syn::Expr::MethodCall(as_deref) = &actor_call.args[0] else {
        return false;
    };
    as_deref.method == "as_deref"
        && as_deref.args.is_empty()
        && field_path(&as_deref.receiver).as_deref() == Some("body.actor")
        && field_path(&actor_call.args[1]).as_deref() == Some("&headers")
        && field_path(&actor_call.args[2]).as_deref() == Some("&state")
}

fn exact_metadata_expression(expression: &syn::Expr) -> bool {
    let Some(metadata_call) = call(expression, "metadata_json") else {
        return false;
    };
    let Some(syn::Expr::MethodCall(map)) = metadata_call.args.first() else {
        return false;
    };
    if metadata_call.args.len() != 1
        || map.method != "map"
        || field_path(&map.receiver).as_deref() != Some("body.metadata")
        || map.args.len() != 1
    {
        return false;
    }
    let syn::Expr::Closure(closure) = &map.args[0] else {
        return false;
    };
    let Some(syn::Pat::Ident(parameter)) = closure.inputs.first() else {
        return false;
    };
    if closure.inputs.len() != 1 || parameter.ident != "value" {
        return false;
    }
    let Some(object_call) = call(&closure.body, "serde_json::Value::Object") else {
        return false;
    };
    let Some(syn::Expr::MethodCall(collect)) = object_call.args.first() else {
        return false;
    };
    let syn::Expr::MethodCall(into_iter) = collect.receiver.as_ref() else {
        return false;
    };
    object_call.args.len() == 1
        && collect.method == "collect"
        && collect.args.is_empty()
        && into_iter.method == "into_iter"
        && into_iter.args.is_empty()
        && field_path(&into_iter.receiver).as_deref() == Some("value")
}

struct BodyAudit {
    fields: Vec<String>,
    explicit_returns: usize,
}

impl<'a> syn::visit::Visit<'a> for BodyAudit {
    fn visit_expr_field(&mut self, value: &'a syn::ExprField) {
        if field_path(&syn::Expr::Field(value.clone()))
            .is_some_and(|path| path.starts_with("body."))
            && let syn::Member::Named(name) = &value.member
        {
            self.fields.push(name.to_string());
        }
        syn::visit::visit_expr_field(self, value);
    }

    fn visit_expr_return(&mut self, value: &'a syn::ExprReturn) {
        self.explicit_returns += 1;
        syn::visit::visit_expr_return(self, value);
    }
}

fn validate_create_task_handler(source: &str) -> Vec<String> {
    let mut violations = validate_create_task_handler_baseline(source);
    let Ok(file) = syn::parse_file(source) else {
        return violations;
    };
    let Some(function) = file.items.iter().find_map(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == "create_task" => Some(function),
        _ => None,
    }) else {
        return violations;
    };
    if function.block.stmts.len() != 5 {
        violations.push("canonical five-statement body".into());
    }

    let mut audit = BodyAudit {
        fields: vec![],
        explicit_returns: 0,
    };
    syn::visit::Visit::visit_block(&mut audit, &function.block);
    if audit.explicit_returns != 0 {
        violations.push("explicit return".into());
    }
    audit.fields.sort();
    let expected_fields = [
        "actor",
        "assignee",
        "depends_on",
        "description",
        "due_at",
        "labels",
        "max_retries",
        "metadata",
        "priority",
        "scheduled_at",
        "status",
        "title",
    ];
    if audit.fields != expected_fields {
        violations.push(format!("request field flow {:?}", audit.fields));
    }

    let actor = function
        .block
        .stmts
        .get(1)
        .and_then(|statement| local(statement, "actor"));
    if !actor.is_some_and(exact_actor_expression) {
        violations.push("exact actor precedence expression".into());
    }

    let input = function
        .block
        .stmts
        .get(2)
        .and_then(|statement| local(statement, "input"));
    let fields = input.and_then(|value| struct_fields(value, "kanban_sqlite::api::CreateTask"));
    for (target, source) in [
        ("title", "body.title"),
        ("description", "body.description"),
        ("assignee", "body.assignee"),
        ("priority", "body.priority"),
        ("scheduled_at", "body.scheduled_at"),
        ("due_at", "body.due_at"),
        ("max_retries", "body.max_retries"),
    ] {
        if fields
            .as_ref()
            .and_then(|fields| fields.get(target))
            .and_then(|value| field_path(value))
            .as_deref()
            != Some(source)
        {
            violations.push(format!("input {target} <- {source}"));
        }
    }
    if fields.as_ref().is_none_or(|fields| fields.len() != 9) {
        violations.push("exact CreateTask fields".into());
    }

    let status = fields.as_ref().and_then(|fields| fields.get("status"));
    let valid_status = status.is_some_and(|status| {
        let syn::Expr::MethodCall(map) = status else {
            return false;
        };
        if map.method != "map"
            || field_path(&map.receiver).as_deref() != Some("body.status")
            || map.args.len() != 1
        {
            return false;
        }
        let syn::Expr::Closure(closure) = &map.args[0] else {
            return false;
        };
        let syn::Expr::Match(mapping) = closure.body.as_ref() else {
            return false;
        };
        let actual = mapping
            .arms
            .iter()
            .filter_map(|arm| {
                let syn::Pat::Path(left) = &arm.pat else {
                    return None;
                };
                let syn::Expr::Path(right) = arm.body.as_ref() else {
                    return None;
                };
                Some((path_name(&left.path), path_name(&right.path)))
            })
            .collect::<Vec<_>>();
        actual
            == [
                (
                    "ApiCreateTaskStatus::Triage".into(),
                    "TaskStatus::Triage".into(),
                ),
                (
                    "ApiCreateTaskStatus::Todo".into(),
                    "TaskStatus::Todo".into(),
                ),
                (
                    "ApiCreateTaskStatus::Scheduled".into(),
                    "TaskStatus::Scheduled".into(),
                ),
                (
                    "ApiCreateTaskStatus::Ready".into(),
                    "TaskStatus::Ready".into(),
                ),
            ]
    });
    if !valid_status {
        violations.push("four exact status mappings".into());
    }

    let metadata = fields
        .as_ref()
        .and_then(|fields| fields.get("metadata_json"));
    let mut metadata_audit = BodyAudit {
        fields: vec![],
        explicit_returns: 0,
    };
    if let Some(metadata) = metadata {
        syn::visit::Visit::visit_expr(&mut metadata_audit, metadata);
    }
    if metadata_audit.fields != ["metadata"]
        || !metadata.is_some_and(|expression| exact_metadata_expression(expression))
    {
        violations.push("metadata_json <- body.metadata".into());
    }

    let service = function
        .block
        .stmts
        .get(3)
        .and_then(|statement| local(statement, "task"))
        .and_then(|value| {
            call(
                value,
                "kanban_sqlite::api::create_task_with_labels_and_dependencies",
            )
        });
    let service_ok = service.is_some_and(|call| {
        call.args.len() == 6
            && method_call(&call.args[0], "state", "db_path")
            && field_path(&call.args[1]).as_deref() == Some("&path.board")
            && field_path(&call.args[2]).as_deref() == Some("&actor")
            && field_path(&call.args[3]).as_deref() == Some("input")
            && field_path(&call.args[4]).as_deref() == Some("&body.labels")
            && field_path(&call.args[5]).as_deref() == Some("&body.depends_on")
    });
    if !service_ok {
        violations.push("unique canonical service input".into());
    }

    let tail_ok = function.block.stmts.get(4).is_some_and(|statement| {
        let syn::Stmt::Expr(expression, None) = statement else {
            return false;
        };
        let Some(ok) = call(expression, "Ok") else {
            return false;
        };
        let Some(syn::Expr::Tuple(tuple)) = ok.args.first() else {
            return false;
        };
        if ok.args.len() != 1
            || tuple.elems.len() != 2
            || field_path(&tuple.elems[0]).as_deref() != Some("StatusCode::CREATED")
        {
            return false;
        }
        let Some(json) = call(&tuple.elems[1], "Json") else {
            return false;
        };
        let Some(response) = json
            .args
            .first()
            .and_then(|value| struct_fields(value, "CreateTaskResponse"))
        else {
            return false;
        };
        let Some(data) = response.get("data") else {
            return false;
        };
        let Some(adapter) = call(data, "api_task_from_record") else {
            return false;
        };
        json.args.len() == 1
            && response.len() == 1
            && adapter.args.len() == 1
            && field_path(&adapter.args[0]).as_deref() == Some("task")
    });
    if !tail_ok {
        violations.push("final implicit CreateTaskResponse tail".into());
    }
    violations
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    let start = source.find("pub(crate) async fn create_task(").unwrap();
    let end = source[start..]
        .find("pub(crate) async fn list_board_labels(")
        .map(|offset| start + offset)
        .unwrap();
    let handler = &source[start..end];
    assert_eq!(handler.matches(from).count(), 1, "mutation source {from:?}");
    format!(
        "{}{}{}",
        &source[..start],
        handler.replacen(from, to, 1),
        &source[end..]
    )
}

#[test]
fn create_task_handler_has_structured_contract_ownership_and_service_boundary() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/tasks.rs");
    let source = fs::read_to_string(path).unwrap();
    assert!(
        validate_create_task_handler(&source).is_empty(),
        "{:?}",
        validate_create_task_handler(&source)
    );
    for mutation in [
        replace_once(&source, "Json<CreateTaskRequest>", "Json<CreateTaskBody>"),
        replace_once(
            &source,
            "Json<CreateTaskResponse>",
            "Json<DataEnvelope<ApiTask>>",
        ),
        replace_once(&source, "title: body.title", "title: String::new()"),
        replace_once(
            &source,
            "description: body.description",
            "description: body.assignee",
        ),
        replace_once(
            &source,
            "assignee: body.assignee",
            "assignee: body.description",
        ),
        replace_once(&source, "priority: body.priority", "priority: 3"),
        replace_once(
            &source,
            "scheduled_at: body.scheduled_at",
            "scheduled_at: body.due_at",
        ),
        replace_once(&source, "due_at: body.due_at", "due_at: body.scheduled_at"),
        replace_once(
            &source,
            "max_retries: body.max_retries",
            "max_retries: None",
        ),
        replace_once(
            &source,
            "body.metadata",
            "None::<BTreeMap<String, serde_json::Value>>",
        ),
        replace_once(
            &source,
            "value.into_iter().collect()",
            "BTreeMap::<String, serde_json::Value>::new().into_iter().collect()",
        ),
        replace_once(
            &source,
            "body.metadata\n                .map",
            "Some(BTreeMap::<String, serde_json::Value>::new())\n                .map",
        ),
        replace_once(&source, "&body.labels", "&body.depends_on"),
        replace_once(&source, "&body.depends_on", "&body.labels"),
        replace_once(&source, "&path.board", "\"default\""),
        replace_once(&source, "&actor,", "\"system\","),
        replace_once(
            &source,
            "body.actor.as_deref()",
            "body.actor.as_deref().filter(|_| false)",
        ),
        replace_once(
            &source,
            "input,",
            "kanban_sqlite::api::CreateTask::ready(\"dummy\"),",
        ),
        replace_once(
            &source,
            "create_task_with_labels_and_dependencies",
            "create_task_with_labels",
        ),
        replace_once(
            &source,
            "ApiCreateTaskStatus::Triage => TaskStatus::Triage",
            "ApiCreateTaskStatus::Triage => TaskStatus::Todo",
        ),
        replace_once(
            &source,
            "ApiCreateTaskStatus::Todo => TaskStatus::Todo",
            "ApiCreateTaskStatus::Todo => TaskStatus::Triage",
        ),
        replace_once(
            &source,
            "ApiCreateTaskStatus::Scheduled => TaskStatus::Scheduled",
            "ApiCreateTaskStatus::Scheduled => TaskStatus::Ready",
        ),
        replace_once(
            &source,
            "ApiCreateTaskStatus::Ready => TaskStatus::Ready",
            "ApiCreateTaskStatus::Ready => TaskStatus::Scheduled",
        ),
        replace_once(
            &source,
            "let task = kanban_sqlite",
            "if false {\n        let _dead = kanban_sqlite::api::create_task_with_labels_and_dependencies(\n            state.db_path(), &path.board, &actor, input, &body.labels, &body.depends_on,\n        );\n    }\n    let task = kanban_sqlite",
        ),
        replace_once(
            &source,
            "Ok((\n        StatusCode::CREATED,",
            "return Ok((\n        StatusCode::CREATED,",
        ),
        replace_once(
            &source,
            "data: api_task_from_record(task)?",
            "data: api_task_from_record(dummy_task)?",
        ),
        replace_once(&source, "Json(CreateTaskResponse", "Json(DataEnvelope"),
    ] {
        assert!(
            syn::parse_file(&mutation).is_ok(),
            "mutation must be valid syntax"
        );
        assert!(!validate_create_task_handler(&mutation).is_empty());
    }
}
