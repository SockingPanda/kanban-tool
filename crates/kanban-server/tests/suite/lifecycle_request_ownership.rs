use std::collections::BTreeSet;

use quote::ToTokens;
use syn::{
    Expr, FnArg, GenericArgument, Item, ItemFn, Member, Pat, PathArguments, Stmt, Type, UseTree,
    visit::{self, Visit},
};

#[derive(Clone, Copy)]
struct HandlerRequest {
    file: &'static str,
    handler: &'static str,
    request: &'static str,
    service_call: &'static str,
    actor_argument: usize,
}

const HANDLER_REQUESTS: &[HandlerRequest] = &[
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "specify_task",
        request: "SpecifyTaskRequest",
        service_call: "specify_task",
        actor_argument: 1,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "promote_task",
        request: "PromoteTaskRequest",
        service_call: "promote_task",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "claim_task",
        request: "ClaimTaskRequest",
        service_call: "claim_task_with_profile_and_metadata",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "reclaim_task",
        request: "ReclaimTaskRequest",
        service_call: "reclaim_task_to",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "reopen_task",
        request: "ReopenTaskRequest",
        service_call: "reopen_task",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "heartbeat_task",
        request: "HeartbeatTaskRequest",
        service_call: "heartbeat_task_with_note",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "complete_task",
        request: "CompleteTaskRequest",
        service_call: "complete_task_with_summary_and_result",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "submit_review_task",
        request: "SubmitReviewTaskRequest",
        service_call: "submit_review_task_with_summary",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "block_task",
        request: "BlockTaskRequest",
        service_call: "block_task",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "unblock_task",
        request: "UnblockTaskRequest",
        service_call: "unblock_task",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/transitions.rs",
        handler: "archive_task",
        request: "ArchiveTaskRequest",
        service_call: "archive_task",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/boards.rs",
        handler: "archive_board",
        request: "ArchiveBoardRequest",
        service_call: "archive_board",
        actor_argument: 2,
    },
    HandlerRequest {
        file: "src/handlers/dependencies.rs",
        handler: "add_dependency",
        request: "AddDependencyRequest",
        service_call: "add_dependency_with_outcome",
        actor_argument: 2,
    },
];

const RETIRED_PRIVATE_BODIES: &[&str] = &[
    "ActorBody",
    "ArchiveBody",
    "BlockBody",
    "ClaimBody",
    "HeartbeatBody",
    "ReclaimBody",
    "ReopenBody",
    "SpecifyBody",
    "TokenBody",
    "AddDependencyBody",
];

#[derive(Default)]
struct Imports {
    contract_names: BTreeSet<String>,
    canonical_actor: bool,
    violations: Vec<String>,
}

fn lifecycle_names() -> BTreeSet<&'static str> {
    HANDLER_REQUESTS
        .iter()
        .map(|entry| entry.request)
        .chain(RETIRED_PRIVATE_BODIES.iter().copied())
        .collect()
}

fn walk_use(tree: &UseTree, prefix: &mut Vec<String>, imports: &mut Imports) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            walk_use(&path.tree, prefix, imports);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            path.push(name.ident.to_string());
            if path.first().is_some_and(|root| root == "kanban_contract") && path.len() == 2 {
                imports.contract_names.insert(path[1].clone());
            }
            if path == ["super", "shared", "actor"] {
                imports.canonical_actor = true;
            }
            if RETIRED_PRIVATE_BODIES.contains(&name.ident.to_string().as_str()) {
                imports
                    .violations
                    .push(format!("禁止导入已退役私有 body: {}", path.join("::")));
            }
        }
        UseTree::Rename(rename) => {
            let original = rename.ident.to_string();
            let alias = rename.rename.to_string();
            let names = lifecycle_names();
            if prefix.first().is_some_and(|root| root == "kanban_contract")
                || names.contains(original.as_str())
                || names.contains(alias.as_str())
            {
                imports.violations.push(format!(
                    "lifecycle request 禁止 use alias: {}::{} as {}",
                    prefix.join("::"),
                    original,
                    alias
                ));
            }
        }
        UseTree::Glob(_) => imports.violations.push(format!(
            "handler request ownership 禁止 glob import: {}::*",
            prefix.join("::")
        )),
        UseTree::Group(group) => {
            for item in &group.items {
                walk_use(item, prefix, imports);
            }
        }
    }
}

fn request_type_from_handler(function: &ItemFn) -> Option<&Type> {
    let body_type = function.sig.inputs.iter().find_map(|argument| {
        let FnArg::Typed(argument) = argument else {
            return None;
        };
        let Pat::Ident(pattern) = &*argument.pat else {
            return None;
        };
        (pattern.ident == "body").then_some(&*argument.ty)
    })?;
    generic_type(body_type, "Result", 0).and_then(|result| generic_type(result, "Json", 0))
}

fn generic_type<'a>(ty: &'a Type, outer: &str, index: usize) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != outer {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .nth(index)
}

fn canonical_request_type(ty: &Type, expected: &str, imports: &Imports) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        [name] if name == expected => imports.contract_names.contains(expected),
        [owner, name] if owner == "kanban_contract" && name == expected => true,
        _ => false,
    }
}

fn path_is(expr: &Expr, expected: &str) -> bool {
    matches!(
        expr,
        Expr::Path(path)
            if path.qself.is_none()
                && path.path.segments.len() == 1
                && path.path.is_ident(expected)
    )
}

fn reference_is(expr: &Expr, expected: &str) -> bool {
    matches!(
        expr,
        Expr::Reference(reference)
            if reference.mutability.is_none() && path_is(&reference.expr, expected)
    )
}

fn canonical_service_path_is(expr: &Expr, expected: &str) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    segments == ["kanban_sqlite", "api", expected]
}

struct ServiceActorUse<'a> {
    service_call: &'a str,
    actor_argument: usize,
    matches: Vec<bool>,
}

impl<'ast> Visit<'ast> for ServiceActorUse<'_> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if canonical_service_path_is(&call.func, self.service_call) {
            self.matches.push(
                call.args
                    .get(self.actor_argument)
                    .is_some_and(|argument| reference_is(argument, "actor")),
            );
        }
        visit::visit_expr_call(self, call);
    }
}

fn has_canonical_service_actor_use(function: &ItemFn, entry: HandlerRequest) -> bool {
    let mut visitor = ServiceActorUse {
        service_call: entry.service_call,
        actor_argument: entry.actor_argument,
        matches: Vec::new(),
    };
    visitor.visit_item_fn(function);
    visitor.matches == [true]
}

fn body_actor_as_deref(expr: &Expr) -> bool {
    let Expr::MethodCall(method) = expr else {
        return false;
    };
    if method.method != "as_deref" || !method.args.is_empty() {
        return false;
    }
    let Expr::Field(field) = &*method.receiver else {
        return false;
    };
    matches!(&field.member, Member::Named(name) if name == "actor") && path_is(&field.base, "body")
}

fn has_canonical_actor_binding(function: &ItemFn) -> bool {
    let actor_bindings = function
        .block
        .stmts
        .iter()
        .filter_map(|statement| {
            let Stmt::Local(local) = statement else {
                return None;
            };
            let Pat::Ident(pattern) = &local.pat else {
                return None;
            };
            (pattern.ident == "actor").then_some(local)
        })
        .collect::<Vec<_>>();
    if actor_bindings.len() != 1 {
        return false;
    }
    let Some(init) = &actor_bindings[0].init else {
        return false;
    };
    let Expr::Call(call) = &*init.expr else {
        return false;
    };
    if !path_is(&call.func, "actor") {
        return false;
    }
    let mut arguments = call.args.iter();
    matches!(
        (
            arguments.next(),
            arguments.next(),
            arguments.next(),
            arguments.next(),
        ),
        (Some(body_actor), Some(headers), Some(state), None)
            if body_actor_as_deref(body_actor)
                && reference_is(headers, "headers")
                && reference_is(state, "state")
    )
}

fn validate_source(source: &str, entries: &[HandlerRequest]) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![format!("Rust AST 解析失败: {error}")],
    };
    let mut imports = Imports::default();
    for item in &file.items {
        if let Item::Use(item) = item {
            walk_use(&item.tree, &mut Vec::new(), &mut imports);
        }
    }

    let mut violations = if entries.is_empty() {
        Vec::new()
    } else {
        std::mem::take(&mut imports.violations)
    };
    let protected = lifecycle_names();
    for item in &file.items {
        let declared = match item {
            Item::Struct(item) => Some(item.ident.to_string()),
            Item::Enum(item) => Some(item.ident.to_string()),
            Item::Type(item) => Some(item.ident.to_string()),
            Item::Union(item) => Some(item.ident.to_string()),
            _ => None,
        };
        if declared
            .as_deref()
            .is_some_and(|name| protected.contains(name))
        {
            violations.push(format!(
                "handler module 禁止声明 lifecycle request/private body: {}",
                declared.unwrap()
            ));
        }
    }

    for entry in entries {
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function) if function.sig.ident == entry.handler => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        if functions.len() != 1 {
            violations.push(format!(
                "{} 必须精确声明一次，实际 {} 次",
                entry.handler,
                functions.len()
            ));
            continue;
        }
        let Some(actual) = request_type_from_handler(functions[0]) else {
            violations.push(format!(
                "{} 的 body 必须是 Result<Json<{}>, JsonRejection>",
                entry.handler, entry.request
            ));
            continue;
        };
        if !canonical_request_type(actual, entry.request, &imports) {
            violations.push(format!(
                "{} body owner/type 漂移，必须直接使用 kanban_contract::{}",
                entry.handler, entry.request
            ));
        }
        if !imports.canonical_actor || !has_canonical_actor_binding(functions[0]) {
            violations.push(format!(
                "{} 必须通过 let actor = actor(body.actor.as_deref(), &headers, &state) 解析 actor",
                entry.handler
            ));
        }
        if !has_canonical_service_actor_use(functions[0], *entry) {
            violations.push(format!(
                "{} service actor 参数必须为 &actor：kanban_sqlite::api::{} argument[{}]",
                entry.handler, entry.service_call, entry.actor_argument
            ));
        }
    }
    violations
}

#[test]
fn lifecycle_request_handler_ownership_catalog_is_exact() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in HANDLER_REQUESTS
        .iter()
        .map(|entry| entry.file)
        .chain(std::iter::once("src/handlers/shared.rs"))
        .collect::<BTreeSet<_>>()
    {
        let source = std::fs::read_to_string(manifest.join(file)).unwrap();
        let entries = HANDLER_REQUESTS
            .iter()
            .copied()
            .filter(|entry| entry.file == file)
            .collect::<Vec<_>>();
        let violations = validate_source(&source, &entries);
        assert!(violations.is_empty(), "{file}: {violations:#?}");
    }
}

#[test]
fn b3_c2_path_success_and_query_header_topology_matches_real_handlers() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/transitions.rs"),
    )
    .unwrap();
    let file = syn::parse_file(&source).unwrap();
    for (handler, operation, path, response) in [
        (
            "claim_task",
            "api.claim-task",
            "ClaimTaskPath",
            "ClaimTaskResponse",
        ),
        (
            "reclaim_task",
            "api.reclaim-task",
            "ReclaimTaskPath",
            "ReclaimTaskResponse",
        ),
        (
            "heartbeat_task",
            "api.heartbeat-task",
            "HeartbeatTaskPath",
            "HeartbeatTaskResponse",
        ),
        (
            "complete_task",
            "api.complete-task",
            "CompleteTaskPath",
            "CompleteTaskResponse",
        ),
        (
            "submit_review_task",
            "api.submit-review-task",
            "SubmitReviewTaskPath",
            "SubmitReviewTaskResponse",
        ),
        (
            "block_task",
            "api.block-task",
            "BlockTaskPath",
            "BlockTaskResponse",
        ),
    ] {
        let function = file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.sig.ident == handler => Some(function),
                _ => None,
            })
            .expect("handler");
        let signature = function.sig.to_token_stream().to_string();
        assert!(
            signature.contains(&format!("Path < {path} >")),
            "{handler}: {signature}"
        );
        assert!(
            signature.contains(&format!("Json < {response} >")),
            "{handler}: {signature}"
        );
        assert!(
            !signature.contains("Query <"),
            "{handler} unexpectedly consumes query"
        );
        assert!(
            signature.contains("HeaderMap"),
            "{handler} must retain actor header input"
        );
        let endpoint = kanban_contract::endpoint_descriptor(operation).unwrap();
        assert_eq!(
            endpoint.obligations.query,
            kanban_contract::EndpointObligation::NotApplicable
        );
        match endpoint.obligations.headers {
            kanban_contract::EndpointObligation::Contract(contract_id) => {
                assert_eq!(contract_id, format!("{operation}.headers"));
            }
            other => panic!("{operation} must own an exact header contract, got {other:?}"),
        }
    }
}

#[test]
fn lifecycle_request_ownership_rejects_private_alias_glob_foreign_wrong_dto_and_actor_bypasses() {
    const CLAIM: HandlerRequest = HandlerRequest {
        file: "synthetic.rs",
        handler: "claim_task",
        request: "ClaimTaskRequest",
        service_call: "claim_task_with_profile_and_metadata",
        actor_argument: 2,
    };
    let valid = r#"
        use axum::{Json, extract::rejection::JsonRejection};
        use kanban_contract::ClaimTaskRequest;
        use super::shared::actor;
        async fn claim_task(
            body: Result<Json<ClaimTaskRequest>, JsonRejection>,
            headers: HeaderMap,
            state: AppState,
        ) {
            let actor = actor(body.actor.as_deref(), &headers, &state);
            kanban_sqlite::api::claim_task_with_profile_and_metadata(
                state.db_path(),
                "default",
                &actor,
                "t_1",
                300_000,
                "manual",
                "{}",
            );
        }
    "#;
    assert!(validate_source(valid, &[CLAIM]).is_empty());

    let assert_mutation_diagnostic =
        |label: &str, from: &str, to: &str, expected_diagnostic: &str| {
            assert_eq!(
                valid.matches(from).count(),
                1,
                "{label} 必须从同一合法 baseline 做单点突变"
            );
            let mutation = valid.replacen(from, to, 1);
            let violations = validate_source(&mutation, &[CLAIM]);
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(expected_diagnostic)),
                "{label} 必须命中目标 diagnostic {expected_diagnostic:?}: {violations:#?}"
            );
        };

    assert_mutation_diagnostic(
        "private body",
        "use kanban_contract::ClaimTaskRequest;",
        "struct ClaimBody;",
        "禁止声明 lifecycle request/private body: ClaimBody",
    );
    assert_mutation_diagnostic(
        "request alias",
        "use kanban_contract::ClaimTaskRequest;",
        "use kanban_contract::ClaimTaskRequest as ClaimBody;",
        "lifecycle request 禁止 use alias",
    );
    assert_mutation_diagnostic(
        "glob import",
        "use kanban_contract::ClaimTaskRequest;",
        "use kanban_contract::*;",
        "handler request ownership 禁止 glob import",
    );
    assert_mutation_diagnostic(
        "foreign owner",
        "use kanban_contract::ClaimTaskRequest;",
        "use evil::ClaimTaskRequest;",
        "claim_task body owner/type 漂移",
    );
    assert_mutation_diagnostic(
        "wrong DTO",
        "Result<Json<ClaimTaskRequest>, JsonRejection>",
        "Result<Json<kanban_contract::CompleteTaskRequest>, JsonRejection>",
        "claim_task body owner/type 漂移",
    );
    assert_mutation_diagnostic(
        "actor binding bypass",
        "let actor = actor(body.actor.as_deref(), &headers, &state);",
        "let actor = \"bypassed\";",
        "claim_task 必须通过 let actor = actor",
    );
    assert_mutation_diagnostic(
        "actor use bypass",
        "&actor,",
        "&\"hard-coded-actor\",",
        "claim_task service actor 参数必须为 &actor",
    );
}

#[test]
fn lifecycle_request_adoption_witnesses_are_directionally_independent() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(manifest.join("tests/suite/lifecycle_request_adoption.rs"))
            .unwrap();

    for forbidden in [
        "macro_rules! adoption_tests",
        "exercise_",
        "fixture_is_produced_by_real_router",
    ] {
        assert!(
            !source.contains(forbidden),
            "request adoption 不能让 producer/consumer 共用同一执行壳或声称 router 生产 request: {forbidden}"
        );
    }
    for required in [
        "macro_rules! request_producer_witness",
        "assert_request_dto_matches_fixture",
        "committed_request_fixture",
        "_request_dto_serializes_to_committed_fixture",
        "_request_fixture_is_consumed_by_real_router",
    ] {
        assert!(
            source.contains(required),
            "request adoption 缺少方向独立证据结构: {required}"
        );
    }

    assert_eq!(source.matches("request_producer_witness!(").count(), 13);
    assert_eq!(
        source
            .matches("_request_fixture_is_consumed_by_real_router")
            .count(),
        13
    );
    assert_eq!(source.matches("committed_request_fixture::<").count(), 13);
    assert_eq!(source.matches(".v1.invalid.json").count(), 13);
    assert!(source.contains(r#"request_raw_json(app.clone(), "POST", uri, raw_fixture)"#));

    let macro_start = source
        .find("macro_rules! request_producer_witness")
        .expect("producer macro");
    let first_invocation = source
        .find("request_producer_witness!(")
        .expect("producer invocation");
    let producer_macro = &source[macro_start..first_invocation];
    assert!(producer_macro.contains("assert_request_dto_matches_fixture"));
    assert!(!producer_macro.contains("committed_request_fixture"));
    assert!(!producer_macro.contains("router"));

    let consumer_start = source
        .find("async fn specify_task_request_fixture_is_consumed_by_real_router")
        .expect("first consumer witness");
    let consumer_region = &source[consumer_start..];
    assert!(consumer_region.contains("committed_request_fixture::<"));
    assert!(!consumer_region.contains("assert_request_dto_matches_fixture"));
}
