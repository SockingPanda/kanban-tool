use quote::ToTokens;
#[test]
fn runs_contract_has_no_private_owner() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dto = std::fs::read_to_string(root.join("crates/kanban-server/src/dto.rs")).unwrap();
    let handler =
        std::fs::read_to_string(root.join("crates/kanban-server/src/handlers/runs.rs")).unwrap();
    assert!(!dto.contains("struct RunDto"));
    assert!(!dto.contains("struct ClaimDto"));
    assert!(handler.contains("Path(path): Path<ListRunsPath>"));
    assert!(handler.contains("Path(path): Path<GetRunPath>"));
    assert!(!handler.contains("claim_token"));
    assert!(!handler.contains("log_path: run.log_path"));
}

fn peel(mut expr: &syn::Expr) -> &syn::Expr {
    loop {
        expr = match expr {
            syn::Expr::Paren(value) => &value.expr,
            syn::Expr::Group(value) => &value.expr,
            _ => return expr,
        };
    }
}

fn exact_tail(expr: &syn::Expr, response: &str) -> bool {
    let syn::Expr::Call(ok) = peel(expr) else {
        return false;
    };
    let syn::Expr::Path(ok_path) = peel(&ok.func) else {
        return false;
    };
    if ok_path
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != "Ok")
        || ok.args.len() != 1
    {
        return false;
    }
    let Some(first) = ok.args.first() else {
        return false;
    };
    let syn::Expr::Call(json) = peel(first) else {
        return false;
    };
    let syn::Expr::Path(json_path) = peel(&json.func) else {
        return false;
    };
    if json_path
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != "Json")
        || json.args.len() != 1
    {
        return false;
    }
    matches!(json.args.first().map(peel), Some(syn::Expr::Struct(value)) if value.path.segments.last().is_some_and(|segment| segment.ident == response) && value.rest.is_none() && value.fields.len() == 1 && matches!(&value.fields[0].member, syn::Member::Named(name) if name == "data"))
}

#[derive(Default)]
struct Audit {
    returns: usize,
    list_calls: usize,
    get_calls: usize,
    adapters: usize,
    forbidden: Vec<String>,
}

impl<'ast> syn::visit::Visit<'ast> for Audit {
    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        self.returns += 1;
        syn::visit::visit_expr_return(self, node);
    }
    fn visit_path(&mut self, path: &'ast syn::Path) {
        let joined = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if joined == "kanban_sqlite::api::list_runs" {
            self.list_calls += 1;
        }
        if joined == "kanban_sqlite::api::get_run_by_id_global" {
            self.get_calls += 1;
        }
        if joined == "api_run" {
            self.adapters += 1;
        }
        if matches!(
            path.segments
                .last()
                .map(|segment| segment.ident.to_string())
                .as_deref(),
            Some("RunDto" | "ClaimDto" | "RunRecord" | "Value")
        ) {
            self.forbidden.push(joined);
        }
        syn::visit::visit_path(self, path);
    }
    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if matches!(&field.member, syn::Member::Named(name) if name == "claim_token" || name == "log_path")
        {
            self.forbidden
                .push(field.member.to_token_stream().to_string());
        }
        syn::visit::visit_expr_field(self, field);
    }
}

fn imported(file: &syn::File, name: &str) -> bool {
    file.items.iter().any(|item| matches!(item, syn::Item::Use(item) if item.to_token_stream().to_string().starts_with("use kanban_contract") && !item.to_token_stream().to_string().contains(" as ") && item.to_token_stream().to_string().split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == name)))
}

fn validate(source: &str) -> Vec<String> {
    use quote::ToTokens;
    use syn::visit::Visit;
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let mut violations = Vec::new();
    let Some(adapter) = file.items.iter().find_map(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == "api_run" => Some(function),
        _ => None,
    }) else {
        return vec!["api_run: missing".into()];
    };
    let adapter_tokens = adapter.block.to_token_stream().to_string();
    if !adapter_tokens.contains("has_log : run . log_path . is_some ()") {
        violations.push("api_run: has_log must derive from log_path presence".into());
    }
    for (name, path, response, service) in [
        ("list_runs", "ListRunsPath", "ListRunsResponse", "list"),
        ("get_run", "GetRunPath", "GetRunResponse", "get"),
    ] {
        if !imported(&file, path) || !imported(&file, response) || !imported(&file, "ApiRun") {
            violations.push(format!("{name}: canonical imports"));
        }
        let Some(function) = file.items.iter().find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        }) else {
            violations.push(format!("{name}: missing"));
            continue;
        };
        let signature = function.sig.to_token_stream().to_string();
        if !signature.contains(path) || !signature.contains(response) {
            violations.push(format!("{name}: typed signature"));
        }
        let Some(tail) = function
            .block
            .stmts
            .last()
            .and_then(|statement| match statement {
                syn::Stmt::Expr(expr, None) => Some(expr),
                _ => None,
            })
        else {
            violations.push(format!("{name}: implicit tail"));
            continue;
        };
        if !exact_tail(tail, response) {
            violations.push(format!("{name}: canonical tail"));
        }
        let mut audit = Audit::default();
        audit.visit_block(&function.block);
        let service_count = if service == "list" {
            audit.list_calls
        } else {
            audit.get_calls
        };
        if service_count != 1 {
            violations.push(format!("{name}: service count {service_count}"));
        }
        if audit.adapters != 1 {
            violations.push(format!("{name}: adapter count {}", audit.adapters));
        }
        if audit.returns != 0 {
            violations.push(format!("{name}: explicit return"));
        }
        violations.extend(
            audit
                .forbidden
                .into_iter()
                .map(|value| format!("{name}: forbidden {value}")),
        );
    }
    violations
}

#[test]
fn runs_handlers_are_structurally_bound_and_mutations_fail_closed() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("crates/kanban-server/src/handlers/runs.rs")).unwrap();
    assert!(validate(&source).is_empty(), "{:?}", validate(&source));
    let mutations = [
        source.replace("Ok(Json(ListRunsResponse { data }))", "let _dead = ListRunsResponse { data: vec![] }; Ok(Json(GetRunResponse { data: api_run(kanban_sqlite::api::get_run_by_id_global(state.db_path(), \"r_dead\")?)? }))"),
        source.replace("Ok(Json(GetRunResponse { data }))", "return Ok(Json(GetRunResponse { data }));"),
        source.replace("ListRunsPath,", "ListRunsPath as PrivatePath,"),
        source.replace("use kanban_contract::{", "use foreign_contract::{"),
        source.replace("Path(path): Path<ListRunsPath>", "Path(path): Path<RunDto>"),
        source.replace("Path(path): Path<ListRunsPath>", "Path(path): Path<serde_json::Value>"),
        source.replace(".map(api_run)", ".map(|run| Ok(run))"),
        source.replace("Path(path): Path<GetRunPath>", "Path(path): Path<kanban_sqlite::api::RunRecord>"),
        source.replace("&path.run_id", "&run.claim_token"),
        source.replace("GetRunResponse { data }", "serde_json::Value::Null"),
        source.replace("has_log: run.log_path.is_some()", "has_log: false"),
    ];
    for mutation in mutations {
        assert_ne!(mutation, source);
        assert!(syn::parse_file(&mutation).is_ok());
        assert!(!validate(&mutation).is_empty());
    }
}
