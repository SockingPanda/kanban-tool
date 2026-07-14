#[test]
fn comments_contract_ownership_has_no_private_dto_or_body() {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dto = std::fs::read_to_string(workspace.join("crates/kanban-server/src/dto.rs")).unwrap();
    let shared =
        std::fs::read_to_string(workspace.join("crates/kanban-server/src/handlers/shared.rs"))
            .unwrap();
    let handler =
        std::fs::read_to_string(workspace.join("crates/kanban-server/src/handlers/comments.rs"))
            .unwrap();
    assert!(
        !dto.contains("struct CommentDto"),
        "private CommentDto must be removed"
    );
    assert!(
        !shared.contains("struct CommentBody"),
        "private CommentBody must be removed"
    );
    assert!(handler.contains("Path(path): Path<ListCommentsPath>"));
    assert!(handler.contains("Path(path): Path<CreateCommentPath>"));
    assert!(handler.contains("Json<CreateCommentRequest>"));
    assert!(handler.contains("ListCommentsResponse"));
    assert!(handler.contains("CreateCommentResponse"));
    assert!(!handler.contains("DataEnvelope<CommentRecord>"));
}

fn peel(mut e: &syn::Expr) -> &syn::Expr {
    loop {
        e = match e {
            syn::Expr::Paren(x) => &x.expr,
            syn::Expr::Group(x) => &x.expr,
            _ => return e,
        }
    }
}
fn call<'a>(e: &'a syn::Expr, n: &str) -> Option<&'a syn::ExprCall> {
    let syn::Expr::Call(c) = peel(e) else {
        return None;
    };
    let syn::Expr::Path(p) = peel(&c.func) else {
        return None;
    };
    (p.path.segments.len() == 1 && p.path.segments[0].ident == n).then_some(c)
}
fn structure<'a>(e: &'a syn::Expr, n: &str) -> Option<&'a syn::ExprStruct> {
    let syn::Expr::Struct(s) = peel(e) else {
        return None;
    };
    (s.path.segments.last()?.ident == n && s.rest.is_none()).then_some(s)
}
fn fields(s: &syn::ExprStruct, n: &[&str]) -> bool {
    s.fields.len() == n.len()
        && n.iter().all(|x| {
            s.fields
                .iter()
                .any(|f| matches!(&f.member,syn::Member::Named(i)if i==x))
        })
}
fn list_tail(e: &syn::Expr) -> bool {
    let Some(o) = call(e, "Ok") else { return false };
    let Some(j) = o.args.first().and_then(|x| call(x, "Json")) else {
        return false;
    };
    o.args.len() == 1
        && j.args.len() == 1
        && j.args
            .first()
            .and_then(|x| structure(x, "ListCommentsResponse"))
            .is_some_and(|s| fields(s, &["data"]))
}
fn create_tail(e: &syn::Expr) -> bool {
    let Some(o) = call(e, "Ok") else { return false };
    let Some(syn::Expr::Tuple(t)) = o.args.first().map(peel) else {
        return false;
    };
    if o.args.len() != 1 || t.elems.len() != 2 {
        return false;
    }
    let status = matches!(peel(&t.elems[0]),syn::Expr::Path(p)if p.path.segments.iter().map(|s|s.ident.to_string()).collect::<Vec<_>>()==["StatusCode","CREATED"]);
    let Some(j) = call(&t.elems[1], "Json") else {
        return false;
    };
    status
        && j.args.len() == 1
        && j.args
            .first()
            .and_then(|x| structure(x, "CreateCommentResponse"))
            .is_some_and(|s| fields(s, &["data"]))
}
#[derive(Default)]
struct Audit {
    returns: usize,
    forbidden: Vec<String>,
    calls: Vec<String>,
    claim: usize,
}
impl<'a> syn::visit::Visit<'a> for Audit {
    fn visit_expr_return(&mut self, x: &'a syn::ExprReturn) {
        self.returns += 1;
        syn::visit::visit_expr_return(self, x)
    }
    fn visit_expr_field(&mut self, x: &'a syn::ExprField) {
        if matches!(&x.member,syn::Member::Named(i)if i=="claim_token") {
            self.claim += 1
        }
        syn::visit::visit_expr_field(self, x)
    }
    fn visit_path(&mut self, p: &'a syn::Path) {
        let n = p
            .segments
            .last()
            .map(|x| x.ident.to_string())
            .unwrap_or_default();
        if n == "api_comment" {
            self.calls.push(n.clone());
        }
        if matches!(
            n.as_str(),
            "DataEnvelope" | "Value" | "CommentDto" | "CommentBody" | "CommentRecord"
        ) {
            self.forbidden.push(n)
        }
        syn::visit::visit_path(self, p)
    }
    fn visit_expr_call(&mut self, c: &'a syn::ExprCall) {
        if let syn::Expr::Path(p) = peel(&c.func) {
            self.calls.push(
                p.path
                    .segments
                    .iter()
                    .map(|x| x.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
        }
        syn::visit::visit_expr_call(self, c)
    }
}
fn imported(f: &syn::File, n: &str) -> bool {
    fn walk(t: &syn::UseTree, p: &mut Vec<String>, n: &str, ok: &mut bool, bad: &mut bool) {
        match t {
            syn::UseTree::Path(x) => {
                p.push(x.ident.to_string());
                walk(&x.tree, p, n, ok, bad);
                p.pop();
            }
            syn::UseTree::Name(x) if x.ident == n => {
                if p.as_slice() == ["kanban_contract"] {
                    *ok = true
                } else {
                    *bad = true
                }
            }
            syn::UseTree::Rename(x) if x.ident == n || x.rename == n => *bad = true,
            syn::UseTree::Glob(_) if p.as_slice() == ["kanban_contract"] => *bad = true,
            syn::UseTree::Group(g) => {
                for x in &g.items {
                    walk(x, p, n, ok, bad)
                }
            }
            _ => {}
        }
    }
    let (mut ok, mut bad) = (false, false);
    for i in &f.items {
        if let syn::Item::Use(u) = i {
            walk(&u.tree, &mut vec![], n, &mut ok, &mut bad)
        }
    }
    ok && !bad
}
fn names(x: &syn::ItemFn) -> Vec<String> {
    struct V(Vec<String>);
    impl<'a> syn::visit::Visit<'a> for V {
        fn visit_type_path(&mut self, x: &'a syn::TypePath) {
            self.0
                .extend(x.path.segments.iter().map(|s| s.ident.to_string()));
            syn::visit::visit_type_path(self, x)
        }
    }
    let mut v = V(vec![]);
    use syn::visit::Visit;
    for x in &x.sig.inputs {
        v.visit_fn_arg(x)
    }
    v.visit_return_type(&x.sig.output);
    v.0
}
fn validate_comments_handlers(src: &str) -> Vec<String> {
    use syn::visit::Visit;
    let file = match syn::parse_file(src) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let specs = [
        (
            "list_comments",
            "ListCommentsPath",
            "ListCommentsResponse",
            "kanban_sqlite::api::list_comments",
            false,
        ),
        (
            "create_comment",
            "CreateCommentPath",
            "CreateCommentResponse",
            "kanban_sqlite::api::create_comment_with_options",
            true,
        ),
    ];
    let mut violations = Vec::new();
    for (handler, path, response, service, create) in specs {
        for import in [path, response] {
            if !imported(&file, import) {
                violations.push(format!("{handler} import {import}"));
            }
        }
        if create && !imported(&file, "CreateCommentRequest") {
            violations.push(format!("{handler} request import"));
        }
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == handler => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        if functions.len() != 1 {
            violations.push(format!("{handler} count"));
            continue;
        }
        let function = functions[0];
        let type_names = names(function);
        for expected in [
            "State", "AppState", "Path", path, "Result", "Json", response, "ApiError",
        ] {
            if !type_names.iter().any(|name| name == expected) {
                violations.push(format!("{handler} type {expected}"));
            }
        }
        if create {
            for expected in [
                "HeaderMap",
                "CreateCommentRequest",
                "JsonRejection",
                "StatusCode",
            ] {
                if !type_names.iter().any(|name| name == expected) {
                    violations.push(format!("{handler} type {expected}"));
                }
            }
        }
        let mut audit = Audit::default();
        audit.visit_item_fn(function);
        let tail = matches!(
            function.block.stmts.last(),
            Some(syn::Stmt::Expr(expression, None))
                if if create { create_tail(expression) } else { list_tail(expression) }
        );
        if !tail || audit.returns != 0 {
            violations.push(format!("{handler} tail"));
        }
        if audit
            .calls
            .iter()
            .filter(|call| call.as_str() == service)
            .count()
            != 1
        {
            violations.push(format!("{handler} service"));
        }
        if !audit.calls.iter().any(|call| call == "api_comment") {
            violations.push(format!("{handler} adapter"));
        }
        if !audit.forbidden.is_empty() || audit.claim != 0 {
            violations.push(format!("{handler} escape"));
        }
    }
    violations
}
const VALID: &str = r#"use kanban_contract::{CreateCommentPath,CreateCommentRequest,CreateCommentResponse,ListCommentsPath,ListCommentsResponse};
async fn list_comments(State(state):State<AppState>,Path(path):Path<ListCommentsPath>)->Result<Json<ListCommentsResponse>,ApiError>{let data=kanban_sqlite::api::list_comments(state.db_path(),&path.task_id)?.into_iter().map(api_comment).collect::<Result<Vec<_>,_>>()?;Ok(Json(ListCommentsResponse{data}))}
async fn create_comment(State(state):State<AppState>,Path(path):Path<CreateCommentPath>,headers:HeaderMap,body:Result<Json<CreateCommentRequest>,JsonRejection>)->Result<(StatusCode,Json<CreateCommentResponse>),ApiError>{let comment=kanban_sqlite::api::create_comment_with_options(state.db_path(),&path.task_id,input)?;Ok((StatusCode::CREATED,Json(CreateCommentResponse{data:api_comment(comment)?})))}
"#;
#[test]
fn comments_handlers_have_structured_canonical_ownership_and_tail() {
    assert!(
        validate_comments_handlers(VALID).is_empty(),
        "{:?}",
        validate_comments_handlers(VALID)
    );
    let ms = [
        VALID.replace(
            "Ok(Json(ListCommentsResponse{data}))",
            "Ok(Json(ListCommentsResponse{data}));alternate_response()",
        ),
        VALID.replace(
            "Ok(Json(ListCommentsResponse{data}))",
            "Ok(Json(ListCommentsResponse{data}));return alternate_response()",
        ),
        VALID
            .replace(
                "ListCommentsResponse};",
                "ListCommentsResponse as Private};",
            )
            .replace("Json<ListCommentsResponse>", "Json<Private>"),
        VALID.replace("Json<ListCommentsResponse>", "Json<CreateCommentResponse>"),
        VALID.replace("Json<ListCommentsResponse>", "Json<CommentDto>"),
        VALID.replace(
            "Json<ListCommentsResponse>",
            "Json<DataEnvelope<Vec<ApiComment>>>",
        ),
        VALID.replace("Json<ListCommentsResponse>", "Json<serde_json::Value>"),
        VALID.replace("let data=", "let _:CommentRecord;let data="),
        VALID.replace(
            "Ok(Json(ListCommentsResponse",
            "let _=record.claim_token;Ok(Json(ListCommentsResponse",
        ),
        VALID.replace(
            "Ok(Json(ListCommentsResponse{data}))",
            "response_helper(data)",
        ),
        VALID.replace("kanban_sqlite::api::list_comments", "private_list_comments"),
        VALID.replace(".map(api_comment)", ".map(private_adapter)"),
        VALID.replace("Json<CreateCommentResponse>", "Json<ListCommentsResponse>"),
    ];
    for m in ms {
        assert_ne!(m, VALID);
        syn::parse_file(&m).expect("mutation syntax");
        assert!(!validate_comments_handlers(&m).is_empty())
    }
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/comments.rs");
    let s = std::fs::read_to_string(p).unwrap();
    let v = validate_comments_handlers(&s);
    assert!(v.is_empty(), "{v:#?}")
}
