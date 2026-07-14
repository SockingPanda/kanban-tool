use std::{
    fs,
    path::{Path, PathBuf},
};
use syn::visit::Visit;

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(&path).expect("read Rust source");
                sources.push((path, source));
            }
        }
    }
    sources
}

fn has_identifier(source: &str, expected: &str) -> bool {
    source
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|identifier| identifier == expected)
}

#[test]
fn checkpoint_a_public_task_and_label_components_are_contract_owned() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let contract_lib = fs::read_to_string(workspace.join("crates/kanban-contract/src/lib.rs"))
        .expect("read kanban-contract lib.rs");
    let server_dto = fs::read_to_string(workspace.join("crates/kanban-server/src/dto.rs"))
        .expect("read kanban-server dto.rs");

    assert!(
        contract_lib.contains("ApiTask")
            && contract_lib.contains("ApiLabel")
            && contract_lib.contains("ApiTaskStatus")
            && contract_lib.contains("ApiTaskPriority")
            && contract_lib.contains("ApiExecutionPlanState"),
        "公开 task/label/status/priority/execution-plan component 必须由 kanban-contract 导出"
    );
    assert!(
        !server_dto.contains("struct ApiTask") && !server_dto.contains("struct ApiLabel"),
        "server 不得继续私有拥有 ApiTask/ApiLabel"
    );

    for (path, source) in rust_sources(&workspace.join("crates/kanban-server/src")) {
        for forbidden in ["TaskDto", "LabelDto"] {
            assert!(
                !has_identifier(&source, forbidden),
                "production source {} 仍引用 {forbidden}",
                path.display(),
            );
        }
    }

    for (path, source) in rust_sources(&workspace.join("crates/kanban-contract/src")) {
        for forbidden in ["TaskReadStatus", "TaskReadPriority"] {
            assert!(
                !has_identifier(&source, forbidden),
                "contract source {} 仍保留旧 alias {forbidden}",
                path.display(),
            );
        }
    }

    let task_mapping_start = server_dto
        .find("let TaskRecord {")
        .expect("TaskRecord adapter must destructure the record");
    let task_mapping_end = server_dto[task_mapping_start..]
        .find("} = task;")
        .map(|offset| task_mapping_start + offset)
        .expect("TaskRecord destructure end");
    let task_mapping = &server_dto[task_mapping_start..task_mapping_end];
    assert!(
        task_mapping.contains("claim_token: _"),
        "TaskRecord adapter 必须显式丢弃 claim_token",
    );
    assert!(
        !task_mapping.contains(".."),
        "TaskRecord adapter 禁止 rest/wildcard 字段映射",
    );

    let label_mapping_start = server_dto
        .find("let LabelRecord {")
        .expect("LabelRecord adapter must destructure the record");
    let label_mapping_end = server_dto[label_mapping_start..]
        .find("} = label;")
        .map(|offset| label_mapping_start + offset)
        .expect("LabelRecord destructure end");
    assert!(
        !server_dto[label_mapping_start..label_mapping_end].contains(".."),
        "LabelRecord adapter 禁止 rest/wildcard 字段映射",
    );
}

fn path_is_exact(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == expected && matches!(segment.arguments, syn::PathArguments::None)
            })
}

fn type_path_is_bare_or_contract(path: &syn::Path, name: &str) -> bool {
    let names = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    names == [name] || names == ["kanban_contract", name]
}

fn is_plain_label_atom_explain_dto(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && (path_is_exact(&path.path, &["LabelAtomExplainDto"])
            || path_is_exact(&path.path, &["kanban_server", "dto", "LabelAtomExplainDto"]))
        && path.path.segments.last().is_some_and(|segment| {
            segment.ident == "LabelAtomExplainDto"
                && matches!(segment.arguments, syn::PathArguments::None)
        })
}

fn peel_expression(mut expression: &syn::Expr) -> &syn::Expr {
    loop {
        expression = match expression {
            syn::Expr::Group(group) => &group.expr,
            syn::Expr::Paren(paren) => &paren.expr,
            syn::Expr::Try(try_expression) => &try_expression.expr,
            _ => return expression,
        };
    }
}

fn is_sqlite_explain_call(expression: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = peel_expression(expression) else {
        return false;
    };
    let syn::Expr::Path(function) = peel_expression(&call.func) else {
        return false;
    };
    path_is_exact(
        &function.path,
        &["kanban_sqlite", "api", "explain_label_atom"],
    )
}

fn is_guarded_explain_envelope_argument(expression: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = peel_expression(expression) else {
        return false;
    };
    let syn::Expr::Path(function) = peel_expression(&call.func) else {
        return false;
    };
    (path_is_exact(&function.path, &["LabelAtomExplainDto", "try_from"])
        || path_is_exact(
            &function.path,
            &["kanban_server", "dto", "LabelAtomExplainDto", "try_from"],
        ))
        && call.args.len() == 1
        && call.args.first().is_some_and(is_sqlite_explain_call)
}

#[derive(Default)]
struct ExplainReturnAudit {
    envelope_types: Vec<bool>,
}

impl<'ast> Visit<'ast> for ExplainReturnAudit {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if type_path_is_bare_or_contract(&path.path, "ExplainLabelAtomResponse") {
            self.envelope_types.push(true);
        }
        if type_path_is_bare_or_contract(&path.path, "DataEnvelope") {
            let valid = path.path.segments.last().is_some_and(|segment| {
                let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                    return false;
                };
                let types = arguments
                    .args
                    .iter()
                    .filter_map(|argument| match argument {
                        syn::GenericArgument::Type(ty) => Some(ty),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                arguments.args.len() == 1
                    && types.len() == 1
                    && is_plain_label_atom_explain_dto(types[0])
            });
            self.envelope_types.push(valid);
        }
        syn::visit::visit_type_path(self, path);
    }
}

#[derive(Default)]
struct ExplainBodyAudit {
    envelope_arguments: Vec<bool>,
    adapter_calls: usize,
    forbidden_record_paths: Vec<String>,
}

impl<'ast> Visit<'ast> for ExplainBodyAudit {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(function) = peel_expression(&call.func) {
            if path_is_exact(&function.path, &["DataEnvelope", "new"])
                || path_is_exact(&function.path, &["kanban_contract", "DataEnvelope", "new"])
            {
                self.envelope_arguments.push(
                    call.args.len() == 1
                        && call
                            .args
                            .first()
                            .is_some_and(is_guarded_explain_envelope_argument),
                );
            }
            if path_is_exact(&function.path, &["LabelAtomExplainDto", "try_from"])
                || path_is_exact(
                    &function.path,
                    &["kanban_server", "dto", "LabelAtomExplainDto", "try_from"],
                )
            {
                self.adapter_calls += 1;
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(identifier) = path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            && matches!(
                identifier.as_str(),
                "LabelAtomExplainRecord" | "LabelAtomExplainSignal" | "TaskRecord"
            )
        {
            self.forbidden_record_paths.push(identifier);
        }
        syn::visit::visit_path(self, path);
    }
}

fn validate_explain_label_atom_ownership(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![format!("无法解析 handler source: {error}")],
    };
    let functions = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "explain_label_atom" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    if functions.len() != 1 {
        return vec![format!(
            "explain_label_atom 顶层函数数量必须为 1，实际为 {}",
            functions.len()
        )];
    }
    let function = functions[0];
    let mut violations = Vec::new();
    let mut return_audit = ExplainReturnAudit::default();
    match &function.sig.output {
        syn::ReturnType::Type(_, ty) => return_audit.visit_type(ty),
        syn::ReturnType::Default => violations.push("handler 缺少显式返回类型".to_owned()),
    }
    if return_audit.envelope_types != [true] {
        violations.push(format!(
            "返回值必须唯一使用 DataEnvelope<LabelAtomExplainDto>: {:?}",
            return_audit.envelope_types
        ));
    }

    if function.block.stmts.len() != 1
        || !matches!(function.block.stmts.first(), Some(syn::Stmt::Expr(_, None)))
    {
        violations.push(
            "handler body 必须是唯一 tail success expression，禁止 return/control-flow/dead guard"
                .to_owned(),
        );
    }
    let mut body_audit = ExplainBodyAudit::default();
    body_audit.visit_item_fn(function);
    if body_audit.envelope_arguments != [true] {
        violations.push(format!(
            "DataEnvelope::new 必须唯一包裹 LabelAtomExplainDto::try_from(sqlite explain): {:?}",
            body_audit.envelope_arguments
        ));
    }
    if body_audit.adapter_calls != 1 {
        violations.push(format!(
            "LabelAtomExplainDto::try_from 调用数量必须为 1，实际为 {}",
            body_audit.adapter_calls
        ));
    }
    if !body_audit.forbidden_record_paths.is_empty() {
        violations.push(format!(
            "handler 不得直接引用 sqlite explain/task record: {:?}",
            body_audit.forbidden_record_paths
        ));
    }
    violations
}

const VALID_EXPLAIN_HANDLER: &str = r#"
async fn explain_label_atom() -> Result<Json<kanban_contract::ExplainLabelAtomResponse>, ApiError> {
    Ok(Json(DataEnvelope::new(LabelAtomExplainDto::try_from(
        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,
    )?)))
}
"#;

#[test]
fn label_atom_explain_handler_ast_ownership_rejects_raw_record_bypasses() {
    assert!(
        validate_explain_label_atom_ownership(VALID_EXPLAIN_HANDLER).is_empty(),
        "synthetic baseline 必须满足 ownership contract"
    );

    let mutations = [
        (
            "raw aggregate return",
            VALID_EXPLAIN_HANDLER
                .replace("kanban_contract::ExplainLabelAtomResponse>,", "DataEnvelope<kanban_sqlite::api::LabelAtomExplainRecord>>,")
                .replace(
                    "LabelAtomExplainDto::try_from(\n        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,\n    )?",
                    "kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?",
                ),
        ),
        (
            "typed return with direct raw serialization",
            VALID_EXPLAIN_HANDLER.replace(
                "LabelAtomExplainDto::try_from(\n        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,\n    )?",
                "kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?",
            ),
        ),
        (
            "dead adapter beside raw serialization",
            VALID_EXPLAIN_HANDLER.replace(
                "Ok(Json(DataEnvelope::new(LabelAtomExplainDto::try_from(\n        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,\n    )?)))",
                "let _guard = LabelAtomExplainDto::try_from(other)?;\n    Ok(Json(DataEnvelope::new(kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?)))",
            ),
        ),
        (
            "nested TaskRecord return bypass",
            VALID_EXPLAIN_HANDLER.replace(
                "kanban_contract::ExplainLabelAtomResponse",
                "DataEnvelope<LabelAtomExplainDto<TaskRecord>>",
            ),
        ),
        (
            "body TaskRecord bypass",
            VALID_EXPLAIN_HANDLER.replace(
                "Ok(Json",
                "let _raw: TaskRecord = raw;\n    Ok(Json",
            ),
        ),
        (
            "foreign suffix-root constructor",
            VALID_EXPLAIN_HANDLER
                .replace("DataEnvelope", "foreign::kanban_contract::DataEnvelope")
                .replace("LabelAtomExplainDto", "foreign::dto::LabelAtomExplainDto"),
        ),
        (
            "dead canonical constructor plus alternate return",
            VALID_EXPLAIN_HANDLER.replace("    Ok(Json", "    if false { let _ = DataEnvelope::new(LabelAtomExplainDto::try_from(other)?); }\n    return Ok(Json"),
        ),
        (
            "constructor alias",
            VALID_EXPLAIN_HANDLER.replace("DataEnvelope::new", "EnvelopeAlias::new"),
        ),
        (
            "wrong root constructor",
            VALID_EXPLAIN_HANDLER.replace("DataEnvelope::new", "foreign::DataEnvelope::new"),
        ),
        (
            "bare Value response",
            VALID_EXPLAIN_HANDLER.replace(
                "kanban_contract::ExplainLabelAtomResponse",
                "DataEnvelope<Value>",
            ),
        ),
        (
            "bare adapter bypasses canonical argument guard",
            VALID_EXPLAIN_HANDLER.replace(
                "LabelAtomExplainDto::try_from(\n        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,\n    )?",
                "LabelAtomExplainDto::try_from(foreign_source, extra)?",
            ),
        ),
        (
            "dead canonical constructor plus foreign actual tail",
            VALID_EXPLAIN_HANDLER.replace(
                "Ok(Json(DataEnvelope::new(LabelAtomExplainDto::try_from(\n        kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?,\n    )?)))",
                "let _dead = DataEnvelope::new(LabelAtomExplainDto::try_from(kanban_sqlite::api::explain_label_atom(db_path, board, atom_ref)?)?);\n    Ok(Json(foreign::DataEnvelope::new(foreign_payload)))",
            ),
        ),
    ];
    for (label, mutation) in mutations {
        let violations = validate_explain_label_atom_ownership(&mutation);
        assert!(
            !violations.is_empty(),
            "mutation `{label}` 必须被 ownership validator 拒绝"
        );
    }

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let production =
        fs::read_to_string(workspace.join("crates/kanban-server/src/handlers/tasks.rs"))
            .expect("read production task handlers");
    let violations = validate_explain_label_atom_ownership(&production);
    assert!(
        violations.is_empty(),
        "production explain handler ownership violations: {violations:#?}"
    );
}

fn contract_fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_str(&fs::read_to_string(root.join(path)).expect("fixture"))
        .expect("typed committed fixture")
}

fn normalize_task_read_identity_and_time(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["id", "board_id"] {
                if map.contains_key(key) {
                    map.insert(
                        key.to_owned(),
                        serde_json::Value::String(
                            if key == "id" {
                                "task-fixture"
                            } else {
                                "board-fixture"
                            }
                            .to_owned(),
                        ),
                    );
                }
            }
            if map.contains_key("ref") {
                map.insert(
                    "ref".to_owned(),
                    serde_json::Value::String("other#1".to_owned()),
                );
            }
            if map.contains_key("seq") {
                map.insert("seq".to_owned(), serde_json::json!(1));
            }
            for key in ["created_at", "updated_at"] {
                if map.contains_key(key) {
                    map.insert(
                        key.to_owned(),
                        serde_json::json!(if key == "created_at" { 1 } else { 2 }),
                    );
                }
            }
            for child in map.values_mut() {
                normalize_task_read_identity_and_time(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                normalize_task_read_identity_and_time(child);
            }
        }
        _ => {}
    }
}

fn assert_no_claim_token(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            assert!(!map.contains_key("claim_token"));
            for value in map.values() {
                assert_no_claim_token(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                assert_no_claim_token(value);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn list_tasks_response_producer_fixture() -> anyhow::Result<()> {
    let fixture: kanban_contract::ListTasksResponse =
        contract_fixture("schemas/fixtures/api/list-tasks-response.v1.valid.json");
    let test = crate::common::TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    crate::common::create_ready_task_for_test(
        test.db_path(),
        "other",
        "fixture",
        "Unicode 标签任务",
    )?;
    let (status, raw) = crate::common::get_json(
        test.router(),
        "/api/v1/boards/other/tasks?limit=25&offset=0",
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::OK);
    let response: kanban_contract::ListTasksResponse = serde_json::from_value(raw.clone())?;
    assert_eq!(
        serde_json::to_value(&response)?,
        raw,
        "raw router response must exact DTO roundtrip"
    );
    assert_no_claim_token(&raw);
    let mut normalized = raw;
    normalize_task_read_identity_and_time(&mut normalized);
    assert_eq!(normalized, serde_json::to_value(fixture)?);
    Ok(())
}

#[test]
fn list_tasks_response_consumer_fixture() {
    let response: kanban_contract::ListTasksResponse =
        contract_fixture("schemas/fixtures/api/list-tasks-response.v1.valid.json");
    assert_eq!(response.meta.total, 1);
    assert_no_claim_token(&serde_json::to_value(response).unwrap());
}

#[tokio::test]
async fn list_tasks_by_status_response_producer_fixture() -> anyhow::Result<()> {
    let fixture: kanban_contract::ListTasksByStatusResponse =
        contract_fixture("schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json");
    let test = crate::common::TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    crate::common::create_ready_task_for_test(
        test.db_path(),
        "other",
        "fixture",
        "Unicode 标签任务",
    )?;
    let (status, raw) = crate::common::get_json(
        test.router(),
        "/api/v1/boards/other/tasks/by-status?status=ready&status=blocked&limit=25&offset=0",
    )
    .await?;
    assert_eq!(status, axum::http::StatusCode::OK);
    let response: kanban_contract::ListTasksByStatusResponse = serde_json::from_value(raw.clone())?;
    assert_eq!(
        serde_json::to_value(&response)?,
        raw,
        "raw router response must exact DTO roundtrip"
    );
    assert_no_claim_token(&raw);
    let mut normalized = raw;
    normalize_task_read_identity_and_time(&mut normalized);
    assert_eq!(normalized, serde_json::to_value(fixture)?);
    Ok(())
}

#[test]
fn list_tasks_by_status_response_consumer_fixture() {
    let response: kanban_contract::ListTasksByStatusResponse =
        contract_fixture("schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json");
    assert_eq!(response.meta.limit, 25);
    assert_eq!(response.data.statuses[0].page.total, 1);
    assert_no_claim_token(&serde_json::to_value(response).unwrap());
}

#[derive(Clone, Copy)]
struct TaskReadSuccessContract {
    handler: &'static str,
    response: &'static str,
    meta: &'static str,
}

const TASK_READ_SUCCESS_CONTRACTS: &[TaskReadSuccessContract] = &[
    TaskReadSuccessContract {
        handler: "list_tasks",
        response: "ListTasksResponse",
        meta: "TotalPaginationMeta",
    },
    TaskReadSuccessContract {
        handler: "list_tasks_by_status",
        response: "ListTasksByStatusResponse",
        meta: "OffsetPaginationMeta",
    },
];

fn type_path_is_plain(ty: &syn::Type, expected: &str) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected
}

fn type_path_arguments<'a>(
    ty: &'a syn::Type,
    expected: &str,
) -> Option<&'a syn::AngleBracketedGenericArguments> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some()
        || path.path.segments.len() != 1
        || path.path.segments[0].ident != expected
    {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &path.path.segments[0].arguments else {
        return None;
    };
    Some(arguments)
}

fn generic_type_arguments(arguments: &syn::AngleBracketedGenericArguments) -> Vec<&syn::Type> {
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
}

fn is_exact_success_return(ty: &syn::Type, response: &str) -> bool {
    let Some(result_arguments) = type_path_arguments(ty, "Result") else {
        return false;
    };
    let result_types = generic_type_arguments(result_arguments);
    if result_arguments.args.len() != 2
        || result_types.len() != 2
        || !type_path_is_plain(result_types[1], "ApiError")
    {
        return false;
    }
    let Some(json_arguments) = type_path_arguments(result_types[0], "Json") else {
        return false;
    };
    let json_types = generic_type_arguments(json_arguments);
    json_arguments.args.len() == 1
        && json_types.len() == 1
        && type_path_is_plain(json_types[0], response)
}

fn path_is_plain(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn expression_is_struct<'a>(
    expression: &'a syn::Expr,
    expected: &str,
) -> Option<&'a syn::ExprStruct> {
    let expression = peel_expression(expression);
    let syn::Expr::Struct(structure) = expression else {
        return None;
    };
    (path_is_plain(&structure.path, expected)
        || path_is_exact(&structure.path, &["kanban_contract", expected]))
    .then_some(structure)
}

fn field_value<'a>(structure: &'a syn::ExprStruct, expected: &str) -> Option<&'a syn::Expr> {
    structure
        .fields
        .iter()
        .find_map(|field| match &field.member {
            syn::Member::Named(name) if name == expected => Some(&field.expr),
            _ => None,
        })
}

fn exact_named_struct_fields(structure: &syn::ExprStruct, expected: &[&str]) -> bool {
    structure.rest.is_none()
        && structure.fields.len() == expected.len()
        && expected
            .iter()
            .all(|name| field_value(structure, name).is_some())
}

fn is_meta_constructor(expression: &syn::Expr, expected: &str) -> bool {
    let Some(structure) = expression_is_struct(expression, expected) else {
        return false;
    };
    exact_named_struct_fields(
        structure,
        if expected == "TotalPaginationMeta" {
            &["limit", "offset", "total"]
        } else {
            &["limit", "offset"]
        },
    )
}

fn is_exact_success_constructor(expression: &syn::Expr, contract: TaskReadSuccessContract) -> bool {
    let Some(response) = expression_is_struct(expression, contract.response) else {
        return false;
    };
    if !exact_named_struct_fields(response, &["data", "meta"])
        || !is_meta_constructor(
            field_value(response, "meta").expect("checked meta"),
            contract.meta,
        )
    {
        return false;
    }
    match contract.handler {
        "list_tasks" => true,
        "list_tasks_by_status" => {
            let Some(data) = expression_is_struct(
                field_value(response, "data").expect("checked data"),
                "ListTasksByStatusData",
            ) else {
                return false;
            };
            exact_named_struct_fields(data, &["statuses"])
        }
        _ => false,
    }
}

#[derive(Default)]
struct SuccessBodyAudit {
    forbidden_paths: Vec<String>,
    claim_token_fields: usize,
    explicit_returns: usize,
}

impl<'ast> Visit<'ast> for SuccessBodyAudit {
    fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
        self.explicit_returns += 1;
        syn::visit::visit_expr_return(self, expression);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if matches!(&field.member, syn::Member::Named(member) if member == "claim_token") {
            self.claim_token_fields += 1;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(name) = path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            && matches!(
                name.as_str(),
                "MetadataEnvelope"
                    | "OptionalMetadataEnvelope"
                    | "Value"
                    | "TaskDto"
                    | "LabelDto"
                    | "TaskPageMetaDto"
                    | "TaskStatusWindowDto"
                    | "TaskRecord"
            )
        {
            self.forbidden_paths.push(name);
        }
        syn::visit::visit_path(self, path);
    }
}

fn canonical_response_imported(file: &syn::File, expected: &str) -> bool {
    struct Imports<'a> {
        expected: &'a str,
        canonical: bool,
        violation: bool,
    }
    fn walk(tree: &syn::UseTree, prefix: &mut Vec<String>, imports: &mut Imports<'_>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                walk(&path.tree, prefix, imports);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                if name.ident == imports.expected {
                    if prefix.as_slice() == ["kanban_contract"] {
                        imports.canonical = true;
                    } else {
                        imports.violation = true;
                    }
                }
            }
            syn::UseTree::Rename(rename) => {
                if rename.ident == imports.expected || rename.rename == imports.expected {
                    imports.violation = true;
                }
            }
            syn::UseTree::Glob(_) => {
                if prefix.as_slice() == ["kanban_contract"] {
                    imports.violation = true;
                }
            }
            syn::UseTree::Group(group) => {
                for child in &group.items {
                    walk(child, prefix, imports);
                }
            }
        }
    }
    let mut imports = Imports {
        expected,
        canonical: false,
        violation: false,
    };
    for item in &file.items {
        if let syn::Item::Use(item) = item {
            walk(&item.tree, &mut Vec::new(), &mut imports);
        }
    }
    imports.canonical && !imports.violation
}

fn has_shadow_declaration(file: &syn::File, expected: &str) -> bool {
    file.items.iter().any(|item| match item {
        syn::Item::Struct(item) => item.ident == expected,
        syn::Item::Enum(item) => item.ident == expected,
        syn::Item::Type(item) => item.ident == expected,
        syn::Item::Union(item) => item.ident == expected,
        _ => false,
    })
}

fn is_exact_tail_success(expression: &syn::Expr, contract: TaskReadSuccessContract) -> bool {
    let syn::Expr::Call(ok) = peel_expression(expression) else {
        return false;
    };
    let syn::Expr::Path(ok_function) = peel_expression(&ok.func) else {
        return false;
    };
    if !path_is_plain(&ok_function.path, "Ok") || ok.args.len() != 1 {
        return false;
    }
    let syn::Expr::Call(json) = peel_expression(ok.args.first().expect("one arg")) else {
        return false;
    };
    let syn::Expr::Path(json_function) = peel_expression(&json.func) else {
        return false;
    };
    path_is_plain(&json_function.path, "Json")
        && json.args.len() == 1
        && is_exact_success_constructor(json.args.first().expect("one arg"), contract)
}

fn validate_task_read_success_ownership(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![format!("无法解析 task-read handler source: {error}")],
    };
    let mut violations = Vec::new();
    for contract in TASK_READ_SUCCESS_CONTRACTS {
        if !canonical_response_imported(&file, contract.response) {
            violations.push(format!(
                "{} 必须直接导入 kanban_contract::{}，禁止 alias/glob/foreign owner",
                contract.handler, contract.response
            ));
        }
        if has_shadow_declaration(&file, contract.response) {
            violations.push(format!(
                "{} 禁止声明 shadow response DTO {}",
                contract.handler, contract.response
            ));
        }
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == contract.handler => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        if functions.len() != 1 {
            violations.push(format!(
                "{} 必须精确声明一次，实际 {} 次",
                contract.handler,
                functions.len()
            ));
            continue;
        }
        let function = functions[0];
        let return_is_exact = match &function.sig.output {
            syn::ReturnType::Type(_, ty) => is_exact_success_return(ty, contract.response),
            syn::ReturnType::Default => false,
        };
        if !return_is_exact {
            violations.push(format!(
                "{} 返回签名必须为 Result<Json<{}>, ApiError>",
                contract.handler, contract.response
            ));
        }
        let mut body = SuccessBodyAudit::default();
        body.visit_item_fn(function);
        let tail_constructor_is_exact = matches!(
            function.block.stmts.last(),
            Some(syn::Stmt::Expr(expression, None)) if is_exact_tail_success(expression, *contract)
        );
        if !tail_constructor_is_exact || body.explicit_returns != 0 {
            violations.push(format!(
                "{} success path 必须是唯一无分号 tail Ok(Json({} {{ data, meta }}))，禁止 return/alternate/control-flow，explicit_returns={}",
                contract.handler, contract.response, body.explicit_returns
            ));
        }
        if body.claim_token_fields != 0 {
            violations.push(format!(
                "{} 不得读取 record.claim_token，实际 {} 次",
                contract.handler, body.claim_token_fields
            ));
        }
        if !body.forbidden_paths.is_empty() {
            violations.push(format!(
                "{} 禁止 response 逃逸类型: {:?}",
                contract.handler, body.forbidden_paths
            ));
        }
    }
    violations
}

const VALID_TASK_READ_SUCCESS_HANDLERS: &str = r#"
use kanban_contract::{ListTasksByStatusData, ListTasksByStatusResponse, ListTasksResponse, OffsetPaginationMeta, TotalPaginationMeta};
async fn list_tasks() -> Result<Json<ListTasksResponse>, ApiError> {
    Ok(Json(ListTasksResponse { data: tasks, meta: TotalPaginationMeta { limit, offset, total } }))
}
async fn list_tasks_by_status() -> Result<Json<ListTasksByStatusResponse>, ApiError> {
    Ok(Json(ListTasksByStatusResponse { data: ListTasksByStatusData { statuses: windows }, meta: OffsetPaginationMeta { limit, offset } }))
}
"#;

#[test]
fn c2b_task_read_handlers_use_distinct_contract_owned_exact_responses() {
    assert!(validate_task_read_success_ownership(VALID_TASK_READ_SUCCESS_HANDLERS).is_empty());
    let mutations = [
        ("dead canonical reference", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse { data: tasks, meta: TotalPaginationMeta { limit, offset, total } }))", "let _: ListTasksResponse; Ok(Json(MetadataEnvelope::new(tasks, TotalPaginationMeta { limit, offset, total })))")),
        ("dead canonical constructor plus alternate return", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse { data: tasks, meta: TotalPaginationMeta { limit, offset, total } }))", "Ok(Json(ListTasksResponse { data: tasks, meta: TotalPaginationMeta { limit, offset, total } })); return alternate_response()")),
        ("foreign owner", VALID_TASK_READ_SUCCESS_HANDLERS.replace("use kanban_contract::{ListTasksByStatusData, ListTasksByStatusResponse, ListTasksResponse, OffsetPaginationMeta, TotalPaginationMeta};", "use foreign::ListTasksResponse; use kanban_contract::{ListTasksByStatusData, ListTasksByStatusResponse, OffsetPaginationMeta, TotalPaginationMeta};")),
        ("shadow response", VALID_TASK_READ_SUCCESS_HANDLERS.replace("async fn list_tasks()", "struct ListTasksResponse; async fn list_tasks()")),
        ("alias response", VALID_TASK_READ_SUCCESS_HANDLERS.replace("ListTasksResponse, OffsetPaginationMeta", "ListTasksResponse as PrivateResponse, OffsetPaginationMeta").replace("Json<ListTasksResponse>", "Json<PrivateResponse>")),
        ("wrong root", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Json<ListTasksResponse>", "Json<ListTasksByStatusResponse>")),
        ("bare value", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Json<ListTasksResponse>", "Json<serde_json::Value>")),
        ("private DTO", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Json<ListTasksResponse>", "Json<TaskPageMetaDto>")),
        ("wrong constructor", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse {", "Ok(Json(ListTasksByStatusResponse {")),
        ("constructor alias literal", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse {", "Ok(Json(ResponseAlias {")),
        ("wrong-root constructor literal", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse {", "Ok(Json(foreign::ListTasksResponse {")),
        ("claim token field", VALID_TASK_READ_SUCCESS_HANDLERS.replace("Ok(Json(ListTasksResponse {", "let _ = record.claim_token; Ok(Json(ListTasksResponse {")),
    ];
    for (label, mutation) in mutations {
        let violations = validate_task_read_success_ownership(&mutation);
        assert!(
            !violations.is_empty(),
            "mutation `{label}` 必须被 production 同一 validator 拒绝"
        );
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let production =
        fs::read_to_string(workspace.join("crates/kanban-server/src/handlers/tasks.rs"))
            .expect("read production task handlers");
    let violations = validate_task_read_success_ownership(&production);
    assert!(
        violations.is_empty(),
        "production task-read success ownership violations: {violations:#?}"
    );
}
