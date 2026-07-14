use std::collections::BTreeSet;

use syn::{
    Expr, FnArg, GenericArgument, Item, ItemFn, PathArguments, Type, UseTree,
    visit::{self, Visit},
};

#[derive(Clone, Copy)]
struct HandlerContract {
    handler: &'static str,
    extractor: &'static str,
    path: &'static str,
    query: &'static str,
}

const HANDLERS: &[HandlerContract] = &[
    HandlerContract {
        handler: "list_tasks",
        extractor: "ListTasksRequest",
        path: "ListTasksPath",
        query: "ListTasksQuery",
    },
    HandlerContract {
        handler: "list_tasks_by_status",
        extractor: "ListTasksByStatusRequest",
        path: "ListTasksByStatusPath",
        query: "ListTasksByStatusQuery",
    },
];

const OWNED_TYPES: &[&str] = &[
    "ListTasksPath",
    "ListTasksQuery",
    "ListTasksByStatusPath",
    "ListTasksByStatusQuery",
];

const RETIRED_PRIVATE_TYPES: &[&str] = &["TaskListQuery"];

#[derive(Default)]
struct Imports {
    contract_names: BTreeSet<String>,
    violations: Vec<String>,
}

fn protected_names() -> BTreeSet<&'static str> {
    OWNED_TYPES
        .iter()
        .chain(RETIRED_PRIVATE_TYPES)
        .copied()
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
        }
        UseTree::Rename(rename) => {
            let original = rename.ident.to_string();
            let alias = rename.rename.to_string();
            let protected = protected_names();
            if prefix.first().is_some_and(|root| root == "kanban_contract")
                || protected.contains(original.as_str())
                || protected.contains(alias.as_str())
            {
                imports.violations.push(format!(
                    "task-read contract 禁止 use alias: {}::{} as {}",
                    prefix.join("::"),
                    original,
                    alias
                ));
            }
        }
        UseTree::Glob(_) => imports.violations.push(format!(
            "task-read ownership 禁止 glob import: {}::*",
            prefix.join("::")
        )),
        UseTree::Group(group) => {
            for item in &group.items {
                walk_use(item, prefix, imports);
            }
        }
    }
}

#[derive(Default)]
struct TypeNames(BTreeSet<String>);

impl<'ast> Visit<'ast> for TypeNames {
    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        for segment in &path.path.segments {
            self.0.insert(segment.ident.to_string());
        }
        visit::visit_type_path(self, path);
    }
}

fn handler_type_names(function: &ItemFn) -> BTreeSet<String> {
    let mut names = TypeNames::default();
    for argument in &function.sig.inputs {
        if let FnArg::Typed(argument) = argument {
            names.visit_type(&argument.ty);
        }
    }
    names.0
}

struct ParserCalls<'a> {
    expected_query: &'a str,
    matches: Vec<bool>,
}

impl<'ast> Visit<'ast> for ParserCalls<'_> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let Expr::Path(path) = &*call.func else {
            visit::visit_expr_call(self, call);
            return;
        };
        let Some(segment) = path.path.segments.last() else {
            visit::visit_expr_call(self, call);
            return;
        };
        if segment.ident == "parse_task_read_query" {
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                self.matches.push(false);
                visit::visit_expr_call(self, call);
                return;
            };
            let query = arguments.args.iter().find_map(|argument| {
                let GenericArgument::Type(Type::Path(path)) = argument else {
                    return None;
                };
                path.path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
            });
            self.matches
                .push(query.as_deref() == Some(self.expected_query));
        }
        visit::visit_expr_call(self, call);
    }
}

fn impl_self_type_name(item: &syn::ItemImpl) -> Option<String> {
    let Type::Path(path) = &*item.self_ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_from_request_parts_impl(item: &syn::ItemImpl) -> bool {
    item.trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .is_some_and(|segment| segment.ident == "FromRequestParts")
}

fn is_parts_uri(expression: &Expr) -> bool {
    let Expr::Field(field) = expression else {
        return false;
    };
    if !matches!(&field.member, syn::Member::Named(member) if member == "uri") {
        return false;
    }
    matches!(
        &*field.base,
        Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "parts")
    )
}

#[derive(Default)]
struct UriQueryCalls(usize);

impl<'ast> Visit<'ast> for UriQueryCalls {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "query" && is_parts_uri(&call.receiver) {
            self.0 += 1;
        }
        visit::visit_expr_method_call(self, call);
    }
}

fn is_path_board_reference(expression: &Expr) -> bool {
    let Expr::Reference(reference) = expression else {
        return false;
    };
    let Expr::Field(field) = &*reference.expr else {
        return false;
    };
    if !matches!(&field.member, syn::Member::Named(member) if member == "board") {
        return false;
    }
    matches!(
        &*field.base,
        Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "path")
    )
}

#[derive(Default)]
struct ListTasksPageBoardArguments(Vec<bool>);

impl<'ast> Visit<'ast> for ListTasksPageBoardArguments {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let is_list_tasks_page = matches!(
            &*call.func,
            Expr::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "list_tasks_page")
        );
        if is_list_tasks_page {
            self.0
                .push(call.args.iter().nth(1).is_some_and(is_path_board_reference));
        }
        visit::visit_expr_call(self, call);
    }
}

fn validate_source(source: &str) -> Vec<String> {
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
    let mut violations = imports.violations;

    for owned in OWNED_TYPES {
        if !imports.contract_names.contains(*owned) {
            violations.push(format!("handler 必须直接导入 kanban_contract::{owned}"));
        }
    }

    let protected = protected_names();
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
                "handler module 禁止声明 task-read private/shadow DTO: {}",
                declared.expect("declared name")
            ));
        }
    }

    for contract in HANDLERS {
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function) if function.sig.ident == contract.handler => Some(function),
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
        let names = handler_type_names(function);
        if !names.contains(contract.extractor) {
            violations.push(format!(
                "{} 缺少 server-local typed extractor {}",
                contract.handler, contract.extractor
            ));
        }
        for forbidden in [
            contract.path,
            contract.query,
            "Path",
            "PathRejection",
            "RawQuery",
            "Query",
            "QueryRejection",
            "TaskListQuery",
        ] {
            if names.contains(forbidden) {
                violations.push(format!(
                    "{} 禁止直接持有 extractor/type {forbidden}",
                    contract.handler
                ));
            }
        }
        let mut handler_calls = ParserCalls {
            expected_query: contract.query,
            matches: Vec::new(),
        };
        handler_calls.visit_item_fn(function);
        if !handler_calls.matches.is_empty() {
            violations.push(format!(
                "{} 禁止直接调用 raw-query parser，实际 {:?}",
                contract.handler, handler_calls.matches
            ));
        }
        let mut board_arguments = ListTasksPageBoardArguments::default();
        board_arguments.visit_item_fn(function);
        if board_arguments.0 != [true] {
            violations.push(format!(
                "{} 必须且只能把 &path.board 作为 list_tasks_page board 参数，实际 {:?}",
                contract.handler, board_arguments.0
            ));
        }

        let request_structs = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(item) if item.ident == contract.extractor => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        if request_structs.len() != 1 {
            violations.push(format!(
                "{} 必须精确声明一个 server-local request struct，实际 {} 个",
                contract.extractor,
                request_structs.len()
            ));
            continue;
        }
        for (field_name, expected_type) in [("path", contract.path), ("query", contract.query)] {
            let field = request_structs[0].fields.iter().find(|field| {
                field
                    .ident
                    .as_ref()
                    .is_some_and(|ident| ident == field_name)
            });
            let Some(field) = field else {
                violations.push(format!("{} 缺少字段 {field_name}", contract.extractor));
                continue;
            };
            let mut field_types = TypeNames::default();
            field_types.visit_type(&field.ty);
            if !field_types.0.contains(expected_type) {
                violations.push(format!(
                    "{}::{field_name} 必须绑定 {expected_type}",
                    contract.extractor
                ));
            }
        }

        let extractor_impls = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Impl(item)
                    if is_from_request_parts_impl(item)
                        && impl_self_type_name(item).as_deref() == Some(contract.extractor) =>
                {
                    Some(item)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if extractor_impls.len() != 1 {
            violations.push(format!(
                "{} 必须精确实现一次 FromRequestParts，实际 {} 次",
                contract.extractor,
                extractor_impls.len()
            ));
            continue;
        }
        let extractor_impl = extractor_impls[0];
        let mut extractor_types = TypeNames::default();
        extractor_types.visit_item_impl(extractor_impl);
        for required in [contract.path, contract.query] {
            if !extractor_types.0.contains(required) {
                violations.push(format!(
                    "{} 缺少 path/query binding {required}",
                    contract.extractor
                ));
            }
        }
        for forbidden in ["RawQuery", "Query", "QueryRejection", "TaskListQuery"] {
            if extractor_types.0.contains(forbidden) {
                violations.push(format!(
                    "{} 禁止持有第二 query extractor/type {forbidden}",
                    contract.extractor
                ));
            }
        }

        let mut calls = ParserCalls {
            expected_query: contract.query,
            matches: Vec::new(),
        };
        calls.visit_item_impl(extractor_impl);
        if calls.matches != [true] {
            violations.push(format!(
                "{} 必须且只能调用一次 parse_task_read_query::<{}>，实际 {:?}",
                contract.extractor, contract.query, calls.matches
            ));
        }
        let mut uri_calls = UriQueryCalls::default();
        uri_calls.visit_item_impl(extractor_impl);
        if uri_calls.0 != 1 {
            violations.push(format!(
                "{} 必须且只能消费一次 parts.uri.query()，实际 {} 次",
                contract.extractor, uri_calls.0
            ));
        }
    }

    let mut uri_calls = UriQueryCalls::default();
    uri_calls.visit_file(&file);
    if uri_calls.0 != 2 {
        violations.push(format!(
            "raw URI 必须且只能由两个 typed extractor 各消费一次，实际 {} 次",
            uri_calls.0
        ));
    }

    let mut parser_calls = ParserCalls {
        expected_query: "",
        matches: Vec::new(),
    };
    parser_calls.visit_file(&file);
    if parser_calls.matches.len() != 2 {
        violations.push(format!(
            "ordered raw-query parser 必须且只能由两个 typed extractor 调用，实际 {} 次",
            parser_calls.matches.len()
        ));
    }

    let parsers = file
        .items
        .iter()
        .filter(|item| matches!(item, Item::Fn(function) if function.sig.ident == "parse_task_read_query"))
        .count();
    if parsers != 1 {
        violations.push(format!(
            "ordered raw-query parser 必须精确声明一次，实际 {parsers} 次"
        ));
    }
    violations
}

#[test]
fn task_read_handler_ownership_is_exact_and_uses_one_ordered_parser() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest.join("src/handlers/tasks.rs")).unwrap();
    let violations = validate_source(&source);
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn task_read_ownership_rejects_private_alias_wrong_path_query_and_dual_source_mutations() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest.join("src/handlers/tasks.rs")).unwrap();
    assert!(validate_source(&source).is_empty());

    let mutations = [
        (
            "contract alias",
            "ListTasksPath, ListTasksQuery,",
            "ListTasksPath as PrivateListTasksPath, ListTasksQuery,",
            "禁止 use alias",
        ),
        (
            "private DTO",
            "pub(crate) async fn list_tasks(",
            "struct TaskListQuery;\npub(crate) async fn list_tasks(",
            "禁止声明 task-read private/shadow DTO",
        ),
        (
            "wrong path binding",
            "Path::<ListTasksPath>::from_request_parts(parts, state)",
            "Path::<String>::from_request_parts(parts, state)",
            "缺少 path/query binding ListTasksPath",
        ),
        (
            "wrong query contract",
            "parse_task_read_query::<ListTasksQuery>",
            "parse_task_read_query::<ListTasksByStatusQuery>",
            "parse_task_read_query::<ListTasksQuery>",
        ),
        (
            "handler dual source",
            "ListTasksRequest { path, query }: ListTasksRequest,\n) -> Result<Json<ListTasksResponse>, ApiError> {",
            "ListTasksRequest { path, query }: ListTasksRequest,\n    RawQuery(_raw_query): RawQuery,\n) -> Result<Json<ListTasksResponse>, ApiError> {",
            "禁止直接持有 extractor/type RawQuery",
        ),
        (
            "list handler path.board 被 default 替换",
            "let page = application_api::list_tasks_page(\n        &application,\n        &path.board,\n        application_api::TaskListOptions {",
            "let page = application_api::list_tasks_page(\n        &application,\n        \"default\",\n        application_api::TaskListOptions {",
            "必须且只能把 &path.board 作为 list_tasks_page board 参数",
        ),
        (
            "by-status handler path.board 被 default 替换",
            "let page = application_api::list_tasks_page(\n            &application,\n            &path.board,\n            application_api::TaskListOptions {",
            "let page = application_api::list_tasks_page(\n            &application,\n            \"default\",\n            application_api::TaskListOptions {",
            "必须且只能把 &path.board 作为 list_tasks_page board 参数",
        ),
        (
            "second raw parser",
            "let query = parse_task_read_query::<ListTasksQuery>(parts.uri.query())?;",
            "let query = parse_task_read_query::<ListTasksQuery>(parts.uri.query())?;\n        let _shadow = parse_task_read_query::<ListTasksQuery>(parts.uri.query())?;",
            "必须且只能调用一次 parse_task_read_query::<ListTasksQuery>",
        ),
    ];

    for (label, from, to, expected) in mutations {
        assert_eq!(
            source.matches(from).count(),
            1,
            "{label} 必须基于唯一合法位置做单点 mutation"
        );
        let mutated = source.replacen(from, to, 1);
        let violations = validate_source(&mutated);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(expected)),
            "{label} 未命中 {expected:?}: {violations:#?}"
        );
    }
}

#[test]
fn task_read_adoption_witnesses_keep_producer_and_router_consumer_independent() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(manifest.join("tests/suite/task_read_request_adoption.rs"))
            .unwrap();

    assert_eq!(source.matches("request_producer_witness!(").count(), 4);
    assert_eq!(
        source
            .matches("_fixture_is_consumed_by_real_router")
            .count(),
        4
    );

    let producer_start = source
        .find("macro_rules! request_producer_witness")
        .expect("producer macro");
    let consumer_start = source
        .find("fn create_fixture_board")
        .expect("router consumer helper boundary");
    let producer_region = &source[producer_start..consumer_start];
    let consumer_region = &source[consumer_start..];
    assert!(producer_region.contains("assert_request_dto_matches_fixture"));
    assert!(!producer_region.contains("TestApp"));
    assert!(!producer_region.contains("get_json"));
    assert!(consumer_region.contains("TestApp"));
    assert!(consumer_region.contains("get_json"));
    assert!(!consumer_region.contains("assert_request_dto_matches_fixture"));
}
#[test]
fn task_read_handlers_delegate_raw_uri_to_two_typed_extractors() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(manifest.join("src/handlers/tasks.rs")).unwrap();

    for (handler, extractor) in [
        ("list_tasks", "ListTasksRequest"),
        ("list_tasks_by_status", "ListTasksByStatusRequest"),
    ] {
        let function = syn::parse_file(&source)
            .unwrap()
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.sig.ident == handler => Some(function),
                _ => None,
            })
            .unwrap_or_else(|| panic!("缺少 handler {handler}"));
        let names = handler_type_names(&function);
        assert!(
            names.contains(extractor),
            "{handler} 必须只通过 {extractor} 接收已绑定 path 与 ordered query"
        );
        for forbidden in [
            "Path",
            "PathRejection",
            "RawQuery",
            "Query",
            "QueryRejection",
        ] {
            assert!(
                !names.contains(forbidden),
                "{handler} 不得直接持有 extractor/type {forbidden}"
            );
        }
    }
}
