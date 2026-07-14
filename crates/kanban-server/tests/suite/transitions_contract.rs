use quote::ToTokens;
use syn::{Expr, Item, ItemFn, Stmt, visit::Visit};

const HANDLERS: &[(&str, &str, &str, &str)] = &[
    (
        "specify_task",
        "SpecifyTaskPath",
        "SpecifyTaskResponse",
        "specify_task",
    ),
    (
        "promote_task",
        "PromoteTaskPath",
        "PromoteTaskResponse",
        "promote_task",
    ),
    (
        "reopen_task",
        "ReopenTaskPath",
        "ReopenTaskResponse",
        "reopen_task",
    ),
    (
        "unblock_task",
        "UnblockTaskPath",
        "UnblockTaskResponse",
        "unblock_task",
    ),
    (
        "archive_task",
        "ArchiveTaskPath",
        "ArchiveTaskResponse",
        "archive_task",
    ),
];

fn peel(mut expr: &Expr) -> &Expr {
    loop {
        expr = match expr {
            Expr::Paren(x) => &x.expr,
            Expr::Group(x) => &x.expr,
            _ => return expr,
        };
    }
}

fn path_is(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Path(path) = peel(expr) else {
        return false;
    };
    path.path.leading_colon.is_none()
        && path
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .eq(expected.iter().copied())
}

fn exact_tail(function: &ItemFn) -> bool {
    let Some(Stmt::Expr(tail, None)) = function.block.stmts.last() else {
        return false;
    };
    let Expr::Call(ok) = peel(tail) else {
        return false;
    };
    if !path_is(&ok.func, &["Ok"]) || ok.args.len() != 1 {
        return false;
    }
    let Some(Expr::Call(json)) = ok.args.first().map(peel) else {
        return false;
    };
    if !path_is(&json.func, &["Json"]) || json.args.len() != 1 {
        return false;
    }
    let Some(Expr::Call(envelope)) = json.args.first().map(peel) else {
        return false;
    };
    path_is(&envelope.func, &["DataEnvelope", "new"]) && envelope.args.len() == 1
}

#[derive(Default)]
struct Audit {
    service: usize,
    adapter: usize,
    envelope: usize,
    returns: usize,
    forbidden: Vec<String>,
}
impl<'a> Visit<'a> for Audit {
    fn visit_expr_call(&mut self, call: &'a syn::ExprCall) {
        if let Expr::Path(path) = peel(&call.func) {
            let joined = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if joined == "api_task_from_record" {
                self.adapter += 1;
            }
            if joined == "DataEnvelope::new" {
                self.envelope += 1;
            }
            if joined.starts_with("kanban_sqlite::api::") {
                self.service += 1;
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
    fn visit_expr_return(&mut self, value: &'a syn::ExprReturn) {
        self.returns += 1;
        syn::visit::visit_expr_return(self, value);
    }
    fn visit_path(&mut self, path: &'a syn::Path) {
        let last = path.segments.last().map(|s| s.ident.to_string());
        if matches!(last.as_deref(), Some("Envelope" | "claim_token")) {
            self.forbidden.push(path.to_token_stream().to_string());
        }
        syn::visit::visit_path(self, path);
    }
    fn visit_expr_field(&mut self, field: &'a syn::ExprField) {
        if matches!(&field.member, syn::Member::Named(name) if name == "claim_token") {
            self.forbidden.push("claim_token".into());
        }
        syn::visit::visit_expr_field(self, field);
    }
}

fn exact_import(file: &syn::File, name: &str) -> bool {
    file.items.iter().any(|item| {
        let Item::Use(item) = item else { return false };
        let source = item.to_token_stream().to_string();
        source.starts_with("use kanban_contract")
            && !source.contains(" as ")
            && source
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == name)
    })
}

fn validate(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let mut violations = Vec::new();
    for (_, path, response, _) in HANDLERS {
        if !exact_import(&file, path) || !exact_import(&file, response) {
            violations.push(format!("canonical import {path}/{response}"));
        }
        for item in &file.items {
            match item {
                Item::Struct(x) if x.ident == *path || x.ident == *response => {
                    violations.push(format!("shadow {}", x.ident))
                }
                Item::Type(x) if x.ident == *path || x.ident == *response => {
                    violations.push(format!("alias shadow {}", x.ident))
                }
                _ => {}
            }
        }
    }
    if !exact_import(&file, "DataEnvelope") {
        violations.push("canonical DataEnvelope import".into());
    }
    for (name, path, response, service) in HANDLERS {
        let Some(function) = file.items.iter().find_map(|item| match item {
            Item::Fn(f) if f.sig.ident == *name => Some(f),
            _ => None,
        }) else {
            violations.push(format!("missing {name}"));
            continue;
        };
        let signature = function.sig.to_token_stream().to_string().replace(' ', "");
        let exact_path = format!("Path({path}{{task_id}}):Path<{path}>");
        if !signature.contains(&exact_path)
            || !signature.contains(&format!("Result<Json<{response}>,ApiError>"))
        {
            violations.push(format!("{name}: exact signature"));
        }
        if !exact_tail(function) {
            violations.push(format!("{name}: implicit canonical tail"));
        }
        let mut audit = Audit::default();
        audit.visit_block(&function.block);
        let body = function.block.to_token_stream().to_string();
        let compact_body = body.replace(' ', "").replace(",)", ")");
        let service_path = format!("kanban_sqlite :: api :: {service}");
        if audit.service != if *name == "specify_task" { 1 } else { 2 }
            || body.matches(&service_path).count() != 1
        {
            violations.push(format!("{name}: unique service/lookup"));
        }
        let critical_args = match *name {
            "specify_task" => {
                "specify_task(state.db_path(),&actor,&task_id,body.description,body.scheduled_at)"
            }
            "promote_task" => "&task.board_id,&actor,&task_id",
            "reopen_task" => {
                "reopen_task(state.db_path(),&task.board_id,&actor,&task_id,&body.reason)"
            }
            "unblock_task" => "&task.board_id,&actor,&task_id",
            "archive_task" => {
                "archive_task(state.db_path(),&task.board_id,&actor,&task_id,body.force)"
            }
            _ => unreachable!(),
        };
        if !compact_body.contains(critical_args) {
            violations.push(format!("{name}: critical service arguments"));
        }
        if audit.adapter != 1 || audit.envelope != 1 {
            violations.push(format!("{name}: adapter/envelope cardinality"));
        }
        if audit.returns != 0 {
            violations.push(format!("{name}: explicit return"));
        }
        violations.extend(
            audit
                .forbidden
                .into_iter()
                .map(|x| format!("{name}: forbidden {x}")),
        );
    }
    violations
}

#[test]
fn transition_handlers_are_exact_and_hostile_mutations_fail_closed() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("crates/kanban-server/src/handlers/transitions.rs"))
            .unwrap();
    assert!(validate(&source).is_empty(), "{:?}", validate(&source));
    let mutations = [
        source.replace("SpecifyTaskPath,", "SpecifyTaskPath as PrivatePath,"),
        source.replace(
            "Path(SpecifyTaskPath { task_id }): Path<SpecifyTaskPath>",
            "Path(PrivatePath { task_id }): Path<PrivatePath>",
        ),
        source.replace(
            "SpecifyTaskResponse,",
            "SpecifyTaskResponse as PrivateResponse,",
        ),
        source.replace(
            "Result<Json<SpecifyTaskResponse>, ApiError>",
            "Result<Json<PromoteTaskResponse>, ApiError>",
        ),
        source.replace(
            "kanban_sqlite::api::specify_task(",
            "kanban_sqlite::api::promote_task(",
        ),
        source.replace(
            "state.db_path(),\n            &actor,",
            "state.db_path(),\n            &task_id,",
        ),
        source.replace(
            "Ok(Json(DataEnvelope::new(api_task_from_record(",
            "return Ok(Json(DataEnvelope::new(api_task_from_record(",
        ),
        source.replace("DataEnvelope::new", "PrivateEnvelope::new"),
        source.replace("body.description,", "body.claim_token,"),
        source.replace(
            "Path(PromoteTaskPath { task_id }): Path<PromoteTaskPath>",
            "Path(SpecifyTaskPath { task_id }): Path<SpecifyTaskPath>",
        ),
    ];
    for (index, mutation) in mutations.into_iter().enumerate() {
        assert_ne!(mutation, source, "mutation {index}");
        assert!(syn::parse_file(&mutation).is_ok());
        assert!(!validate(&mutation).is_empty(), "mutation {index}");
    }
}
