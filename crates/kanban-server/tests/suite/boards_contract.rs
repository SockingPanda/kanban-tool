use quote::ToTokens;
use std::collections::BTreeSet;
use syn::{
    Expr, Item, ItemFn, UseTree,
    visit::{self, Visit},
};

const OWNED: &[&str] = &[
    "ApiBoard",
    "ListBoardsQuery",
    "CreateBoardRequest",
    "ArchiveBoardRequest",
    "GetBoardPath",
    "ArchiveBoardPath",
    "ListBoardsResponse",
    "CreateBoardResponse",
    "GetBoardResponse",
    "ArchiveBoardResponse",
];

#[derive(Default)]
struct Imports {
    canonical: BTreeSet<String>,
    violations: Vec<String>,
}

fn walk_use(tree: &UseTree, prefix: &mut Vec<String>, imports: &mut Imports) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            walk_use(&path.tree, prefix, imports);
            prefix.pop();
        }
        UseTree::Name(name) => {
            if prefix.as_slice() == ["kanban_contract"]
                && OWNED.contains(&name.ident.to_string().as_str())
            {
                imports.canonical.insert(name.ident.to_string());
            }
        }
        UseTree::Rename(rename) => {
            if prefix.first().is_some_and(|root| root == "kanban_contract")
                || OWNED.contains(&rename.ident.to_string().as_str())
                || OWNED.contains(&rename.rename.to_string().as_str())
            {
                imports.violations.push("contract alias".into());
            }
        }
        UseTree::Glob(_) if prefix.first().is_some_and(|root| root == "kanban_contract") => {
            imports.violations.push("contract glob".into());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                walk_use(item, prefix, imports);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Audit {
    types: BTreeSet<String>,
    calls: Vec<String>,
    path_board_refs: usize,
}

impl<'ast> Visit<'ast> for Audit {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "api_board")
        {
            self.calls.push("api_board".into());
        }
        visit::visit_expr_path(self, path);
    }

    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        self.types.extend(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string()),
        );
        visit::visit_type_path(self, path);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*call.func {
            self.calls.push(
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if matches!(&*reference.expr,
            Expr::Field(field)
                if matches!(&field.member, syn::Member::Named(member) if member == "board")
                && matches!(&*field.base, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "path")))
        {
            self.path_board_refs += 1;
        }
        visit::visit_expr_reference(self, reference);
    }
}

fn tokens(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn expected_handler(name: &str) -> ItemFn {
    let source = match name {
        "list_boards" => {
            r#"async fn list_boards(
            State(state): State<AppState>,
            query: Result<Query<ListBoardsQuery>, QueryRejection>,
        ) -> Result<Json<ListBoardsResponse>, ApiError> {
            let Query(query) = query.map_err(extractor_error)?;
            let boards = kanban_sqlite::api::list_boards(
                state.db_path(),
                kanban_sqlite::api::BoardListOptions {
                    include_archived: query.include_archived,
                },
            )?;
            Ok(Json(ListBoardsResponse {
                data: boards.into_iter().map(api_board).collect(),
            }))
        }"#
        }
        "create_board" => {
            r#"async fn create_board(
            State(state): State<AppState>,
            headers: HeaderMap,
            body: Result<Json<CreateBoardRequest>, JsonRejection>,
        ) -> Result<(StatusCode, Json<CreateBoardResponse>), ApiError> {
            let Json(body) = body.map_err(extractor_error)?;
            let actor = actor(body.actor.as_deref(), &headers, &state);
            let board = kanban_sqlite::api::create_board(
                state.db_path(),
                &actor,
                kanban_sqlite::api::CreateBoard {
                    slug: body.slug,
                    name: body.name,
                    description: body.description,
                },
            )?;
            Ok((
                StatusCode::CREATED,
                Json(CreateBoardResponse {
                    data: api_board(board),
                }),
            ))
        }"#
        }
        "get_board" => {
            r#"async fn get_board(
            State(state): State<AppState>,
            Path(path): Path<GetBoardPath>,
        ) -> Result<Json<GetBoardResponse>, ApiError> {
            let board = kanban_sqlite::api::get_board(state.db_path(), &path.board)?;
            Ok(Json(GetBoardResponse {
                data: api_board(board),
            }))
        }"#
        }
        "archive_board" => {
            r#"async fn archive_board(
            State(state): State<AppState>,
            Path(path): Path<ArchiveBoardPath>,
            headers: HeaderMap,
            body: Result<Json<ArchiveBoardRequest>, JsonRejection>,
        ) -> Result<Json<ArchiveBoardResponse>, ApiError> {
            let body = optional_json_body(body)?;
            let actor = actor(body.actor.as_deref(), &headers, &state);
            let board = kanban_sqlite::api::archive_board(state.db_path(), &path.board, &actor)?;
            Ok(Json(ArchiveBoardResponse {
                data: api_board(board),
            }))
        }"#
        }
        _ => panic!("unknown boards handler {name}"),
    };
    syn::parse_str(source).expect("canonical boards handler must parse")
}

fn validate(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let mut violations = Vec::new();
    let mut imports = Imports::default();
    for item in &file.items {
        match item {
            Item::Use(item) => walk_use(&item.tree, &mut Vec::new(), &mut imports),
            Item::Struct(item) if OWNED.contains(&item.ident.to_string().as_str()) => {
                violations.push(format!("private contract shadow {}", item.ident));
            }
            Item::Type(item) if OWNED.contains(&item.ident.to_string().as_str()) => {
                violations.push(format!("private contract alias {}", item.ident));
            }
            _ => {}
        }
    }
    violations.extend(imports.violations);
    for name in OWNED {
        if !imports.canonical.contains(*name) {
            violations.push(format!("missing canonical import {name}"));
        }
    }

    let adapters = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == "api_board" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let board_fields = [
        "id",
        "slug",
        "name",
        "description",
        "created_at",
        "updated_at",
        "archived_at",
    ];
    if adapters.len() != 1 {
        violations.push("api_board count".into());
    } else {
        let exact_mapping = matches!(adapters[0].block.stmts.last(),
        Some(syn::Stmt::Expr(Expr::Struct(adapter), None))
            if adapter.path.segments.last().is_some_and(|segment| segment.ident == "ApiBoard")
            && adapter.rest.is_none()
            && adapter.fields.len() == board_fields.len()
            && board_fields.iter().all(|expected| adapter.fields.iter().any(|field| {
                matches!((&field.member, &field.expr),
                    (syn::Member::Named(output), Expr::Field(input))
                        if output == expected
                        && matches!(&input.member, syn::Member::Named(member) if member == expected)
                        && matches!(&*input.base, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "board")))
            })));
        if !exact_mapping {
            violations.push("api_board exact field mapping".into());
        }
    }

    let specs = [
        (
            "list_boards",
            &["ListBoardsQuery", "ListBoardsResponse"][..],
            "kanban_sqlite::api::list_boards",
            false,
        ),
        (
            "create_board",
            &["CreateBoardRequest", "CreateBoardResponse"][..],
            "kanban_sqlite::api::create_board",
            false,
        ),
        (
            "get_board",
            &["GetBoardPath", "GetBoardResponse"][..],
            "kanban_sqlite::api::get_board",
            true,
        ),
        (
            "archive_board",
            &[
                "ArchiveBoardPath",
                "ArchiveBoardRequest",
                "ArchiveBoardResponse",
            ][..],
            "kanban_sqlite::api::archive_board",
            true,
        ),
    ];
    for (name, expected_types, service, requires_path_board) in specs {
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function) if function.sig.ident == name => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        if functions.len() != 1 {
            violations.push(format!("{name} count"));
            continue;
        }
        let expected = expected_handler(name);
        if tokens(&functions[0].sig) != tokens(&expected.sig) {
            violations.push(format!("{name} exact extractor/return signature"));
        }
        if tokens(&functions[0].block) != tokens(&expected.block) {
            violations.push(format!("{name} exact service/adapter/tail block"));
        }
        let mut audit = Audit::default();
        audit.visit_item_fn(functions[0]);
        for expected in expected_types {
            if !audit.types.contains(*expected) {
                violations.push(format!("{name} type {expected}"));
            }
        }
        if audit
            .calls
            .iter()
            .filter(|call| call.as_str() == service)
            .count()
            != 1
        {
            violations.push(format!("{name} service"));
        }
        if !audit.calls.iter().any(|call| call == "api_board") {
            violations.push(format!("{name} adapter"));
        }
        if requires_path_board && audit.path_board_refs != 1 {
            violations.push(format!("{name} path board"));
        }
        if audit.types.contains("BoardRecord") || audit.types.contains("CreateBoardBody") {
            violations.push(format!("{name} private wire leak"));
        }
    }
    violations
}

#[test]
fn boards_handlers_have_contract_owned_ast_and_mutation_lock() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/boards.rs");
    let source = std::fs::read_to_string(path).unwrap();
    let baseline = validate(&source);
    assert!(baseline.is_empty(), "{baseline:#?}");

    let mutations = [
        (
            "private same-name",
            source.replace(
                "use kanban_contract::{",
                "struct ListBoardsQuery;\nuse kanban_contract::{",
            ),
        ),
        (
            "list dummy extractor",
            source.replace(
                "query: Result<Query<ListBoardsQuery>, QueryRejection>,",
                "query: Result<Query<ListBoardsQuery>, QueryRejection>, _dummy: Json<ApiBoard>,",
            ),
        ),
        (
            "list raw response",
            source.replace(
                "Json<ListBoardsResponse>",
                "Json<DataEnvelope<Vec<kanban_sqlite::api::BoardRecord>>>",
            ),
        ),
        (
            "list Value response",
            source.replace("Json<ListBoardsResponse>", "Json<serde_json::Value>"),
        ),
        (
            "list hardcoded query",
            source.replace(
                "include_archived: query.include_archived",
                "include_archived: false",
            ),
        ),
        (
            "list dummy call",
            source.replace(
                "let boards = kanban_sqlite::api::list_boards(",
                "let _dummy = String::new(); let boards = kanban_sqlite::api::list_boards(",
            ),
        ),
        (
            "list explicit return",
            source.replace(
                "Ok(Json(ListBoardsResponse {",
                "return Ok(Json(ListBoardsResponse {",
            ),
        ),
        (
            "create private request",
            source.replace("CreateBoardRequest", "PrivateCreateBoardRequest"),
        ),
        (
            "create wrong response",
            source.replace("Json<CreateBoardResponse>", "Json<GetBoardResponse>"),
        ),
        (
            "create hardcoded service",
            source.replace("slug: body.slug", "slug: String::from(\"hardcoded\")"),
        ),
        (
            "create wrong tail",
            source.replace("data: api_board(board),", "data: foreign_api_board(board),"),
        ),
        (
            "get wrong path",
            source.replace(
                "Path(path): Path<GetBoardPath>",
                "Path(path): Path<ArchiveBoardPath>",
            ),
        ),
        (
            "get hardcoded path",
            source.replace("&path.board)?;", "&String::from(\"default\"))?;"),
        ),
        (
            "get private service",
            source.replace("kanban_sqlite::api::get_board", "private_get_board"),
        ),
        (
            "get dead code",
            source.replace(
                "let board = kanban_sqlite::api::get_board",
                "if false { panic!() } let board = kanban_sqlite::api::get_board",
            ),
        ),
        (
            "archive private request",
            source.replace("ArchiveBoardRequest", "PrivateArchiveBoardRequest"),
        ),
        (
            "archive Value response",
            source.replace("Json<ArchiveBoardResponse>", "Json<serde_json::Value>"),
        ),
        (
            "archive hardcoded actor",
            source.replace("&path.board, &actor", "&path.board, \"system\""),
        ),
        (
            "adapter swapped field",
            source.replace(
                "archived_at: board.archived_at",
                "archived_at: board.created_at",
            ),
        ),
    ];
    for (label, mutation) in mutations {
        assert_ne!(mutation, source, "mutation did not apply: {label}");
        syn::parse_file(&mutation).unwrap_or_else(|error| {
            panic!("mutation must remain valid Rust syntax: {label}: {error}")
        });
        assert!(
            !validate(&mutation).is_empty(),
            "mutation escaped ownership audit: {label}"
        );
    }
}
