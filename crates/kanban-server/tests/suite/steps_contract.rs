use syn::{Expr, FnArg, Item, ItemFn, Pat, Stmt, Type, visit::Visit};

struct Endpoint {
    handler: &'static str,
    path: &'static str,
    request: Option<&'static str>,
    response: &'static str,
    service: &'static str,
    fields: &'static [&'static str],
}

const ENDPOINTS: &[Endpoint] = &[
    Endpoint {
        handler: "list_steps",
        path: "ListStepsPath",
        request: None,
        response: "ListStepsResponse",
        service: "list_steps",
        fields: &["path.task_id"],
    },
    Endpoint {
        handler: "create_step",
        path: "CreateStepPath",
        request: Some("CreateStepRequest"),
        response: "CreateStepResponse",
        service: "create_step",
        fields: &[
            "path.task_id",
            "body.title",
            "body.body",
            "body.linked_task_ref",
            "body.position",
            "body.required",
            "actor",
        ],
    },
    Endpoint {
        handler: "update_step",
        path: "UpdateStepPath",
        request: Some("UpdateStepRequest"),
        response: "UpdateStepResponse",
        service: "update_step",
        fields: &[
            "path.task_id",
            "path.step_id",
            "body.title",
            "body.body",
            "body.linked_task_ref",
            "body.unlink_task",
            "body.position",
            "body.required",
            "actor",
        ],
    },
    Endpoint {
        handler: "remove_step",
        path: "RemoveStepPath",
        request: None,
        response: "RemoveStepResponse",
        service: "remove_step",
        fields: &["path.task_id", "path.step_id", "actor"],
    },
    Endpoint {
        handler: "complete_step",
        path: "CompleteStepPath",
        request: Some("CompleteStepRequest"),
        response: "CompleteStepResponse",
        service: "complete_step",
        fields: &["path.task_id", "path.step_id", "body.note", "actor"],
    },
    Endpoint {
        handler: "skip_step",
        path: "SkipStepPath",
        request: Some("SkipStepRequest"),
        response: "SkipStepResponse",
        service: "skip_step",
        fields: &["path.task_id", "path.step_id", "body.reason", "actor"],
    },
    Endpoint {
        handler: "reopen_step",
        path: "ReopenStepPath",
        request: Some("ReopenStepRequest"),
        response: "ReopenStepResponse",
        service: "reopen_step",
        fields: &["path.task_id", "path.step_id", "body.reason", "actor"],
    },
];

fn type_contains(ty: &Type, wanted: &str) -> bool {
    match ty {
        Type::Path(path) => path.path.segments.iter().any(|segment| segment.ident == wanted || match &segment.arguments {
            syn::PathArguments::AngleBracketed(args) => args.args.iter().any(|arg| matches!(arg, syn::GenericArgument::Type(ty) if type_contains(ty, wanted))),
            _ => false,
        }),
        Type::Reference(reference) => type_contains(&reference.elem, wanted),
        Type::Tuple(tuple) => tuple.elems.iter().any(|ty| type_contains(ty, wanted)),
        _ => false,
    }
}

fn field_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        Expr::Field(field) => Some(format!(
            "{}.{}",
            field_name(&field.base)?,
            match &field.member {
                syn::Member::Named(id) => id.to_string(),
                syn::Member::Unnamed(i) => i.index.to_string(),
            }
        )),
        Expr::Reference(reference) => field_name(&reference.expr),
        Expr::MethodCall(call) => field_name(&call.receiver),
        Expr::Paren(paren) => field_name(&paren.expr),
        _ => None,
    }
}

#[derive(Default)]
struct BlockFacts {
    service_calls: Vec<(String, Vec<String>)>,
    explicit_returns: usize,
}
impl<'ast> Visit<'ast> for BlockFacts {
    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        self.explicit_returns += 1;
        syn::visit::visit_expr_return(self, node);
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*node.func {
            let segments = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>();
            if segments.len() == 3 && segments[0] == "kanban_sqlite" && segments[1] == "api" {
                let mut fields = Vec::new();
                for arg in &node.args {
                    let mut collector = FieldCollector(&mut fields);
                    collector.visit_expr(arg);
                }
                self.service_calls.push((segments[2].clone(), fields));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}
struct FieldCollector<'a>(&'a mut Vec<String>);
impl<'ast> Visit<'ast> for FieldCollector<'_> {
    fn visit_expr(&mut self, node: &'ast Expr) {
        if let Some(name) = field_name(node)
            && (name.starts_with("path.") || name.starts_with("body.") || name == "actor")
        {
            self.0.push(name);
        }
        syn::visit::visit_expr(self, node);
    }
}
struct IdentFinder<'a> {
    wanted: &'a str,
    found: bool,
}
impl<'ast> Visit<'ast> for IdentFinder<'_> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if ident == self.wanted {
            self.found = true;
        }
    }
}

fn function<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemFn> {
    file.items.iter().find_map(|item| match item {
        Item::Fn(fun) if fun.sig.ident == name => Some(fun),
        _ => None,
    })
}

fn validate(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let mut errors = Vec::new();
    for endpoint in ENDPOINTS {
        let Some(fun) = function(&file, endpoint.handler) else {
            errors.push(format!("missing {}", endpoint.handler));
            continue;
        };
        let input_types = fun
            .sig
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Typed(input) => Some(&*input.ty),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !input_types
            .iter()
            .any(|ty| type_contains(ty, endpoint.path))
        {
            errors.push(format!("{}: path owner", endpoint.handler));
        }
        if endpoint
            .request
            .is_some_and(|request| !input_types.iter().any(|ty| type_contains(ty, request)))
        {
            errors.push(format!("{}: request owner", endpoint.handler));
        }
        if !matches!(&fun.sig.output, syn::ReturnType::Type(_, ty) if type_contains(ty, endpoint.response))
        {
            errors.push(format!("{}: response owner", endpoint.handler));
        }
        if fun.sig.inputs.iter().any(|input| matches!(input, FnArg::Typed(input) if matches!(&*input.pat, Pat::Ident(_)) && type_contains(&input.ty, "Value"))) { errors.push(format!("{}: opaque input", endpoint.handler)); }
        let mut facts = BlockFacts::default();
        facts.visit_block(&fun.block);
        let matching = facts
            .service_calls
            .iter()
            .filter(|(name, _)| name == endpoint.service)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            errors.push(format!(
                "{}: canonical service count {}",
                endpoint.handler,
                matching.len()
            ));
        }
        if let Some((_, fields)) = matching.first() {
            for expected in endpoint.fields {
                if !fields.iter().any(|field| field == expected) {
                    errors.push(format!("{}: missing flow {expected}", endpoint.handler));
                }
            }
        }
        if facts.explicit_returns != 0 {
            errors.push(format!("{}: explicit return", endpoint.handler));
        }
        let tail = fun.block.stmts.last().and_then(|stmt| match stmt {
            Stmt::Expr(expr, None) => Some(expr),
            _ => None,
        });
        let mut finder = IdentFinder {
            wanted: endpoint.response,
            found: false,
        };
        if let Some(tail) = tail {
            finder.visit_expr(tail);
        }
        if !finder.found {
            errors.push(format!(
                "{}: final implicit response tail",
                endpoint.handler
            ));
        }
    }
    errors
}

fn validate_api_step(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let Some(fun) = function(&file, "api_step") else {
        return vec!["missing api_step".into()];
    };
    let mut errors = Vec::new();
    struct StructFinder<'a> {
        fields: &'a mut Vec<(String, String)>,
    }
    impl<'ast> Visit<'ast> for StructFinder<'_> {
        fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
            if node
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "ApiTaskStep")
            {
                for field in &node.fields {
                    if let syn::Member::Named(name) = &field.member {
                        self.fields.push((
                            name.to_string(),
                            field_name(&field.expr).unwrap_or_default(),
                        ));
                    }
                }
            }
            syn::visit::visit_expr_struct(self, node);
        }
    }
    let mut fields = Vec::new();
    StructFinder {
        fields: &mut fields,
    }
    .visit_block(&fun.block);
    for (target, source) in [
        ("id", "step.id"),
        ("parent_task_id", "step.parent_task_id"),
        ("title", "step.title"),
        ("body", "step.body"),
        ("position", "step.position"),
        ("required", "step.required"),
        ("resolution_note", "step.resolution_note"),
        ("resolved_by", "step.resolved_by"),
        ("resolved_at", "step.resolved_at"),
        ("created_by", "step.created_by"),
        ("created_at", "step.created_at"),
        ("updated_by", "step.updated_by"),
        ("updated_at", "step.updated_at"),
    ] {
        if !fields
            .iter()
            .any(|(name, value)| name == target && value == source)
        {
            errors.push(format!("api_step: {target} must map from {source}"));
        }
    }
    for canonical in [
        "kanban_sqlite::api::StepStatus::Todo => ApiStepStatus::Todo",
        "kanban_sqlite::api::StepStatus::Done => ApiStepStatus::Done",
        "kanban_sqlite::api::StepStatus::Skipped => ApiStepStatus::Skipped",
        ".linked_task\n            .map(crate::dto::api_task_from_record)\n            .transpose()?",
    ] {
        if !source.contains(canonical) {
            errors.push(format!(
                "api_step: missing canonical adapter chain {canonical}"
            ));
        }
    }
    errors
}

fn validate_api_execution_plan(source: &str) -> Vec<String> {
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let Some(fun) = function(&file, "api_execution_plan") else {
        return vec!["missing api_execution_plan".into()];
    };
    let mut fields = Vec::new();
    struct Finder<'a>(&'a mut Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Finder<'_> {
        fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
            if node
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "ApiExecutionPlan")
            {
                for field in &node.fields {
                    if let syn::Member::Named(name) = &field.member {
                        self.0.push((
                            name.to_string(),
                            field_name(&field.expr).unwrap_or_default(),
                        ));
                    }
                }
            }
            syn::visit::visit_expr_struct(self, node);
        }
    }
    Finder(&mut fields).visit_block(&fun.block);
    let mut errors = Vec::new();
    for (target, from) in [
        ("board_id", "plan.board_id"),
        ("task_id", "plan.task_id"),
        ("reason", "plan.reason"),
        ("updated_by", "plan.updated_by"),
        ("updated_at", "plan.updated_at"),
    ] {
        if !fields
            .iter()
            .any(|(name, value)| name == target && value == from)
        {
            errors.push(format!("api_execution_plan: {target} must map from {from}"));
        }
    }
    if !source.contains("state: crate::dto::api_execution_plan_state_from_record(plan.state)") {
        errors.push("api_execution_plan: non-canonical state adapter".into());
    }
    errors
}

#[test]
fn steps_contract_ownership_has_no_private_dto_or_request_owner() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let handler = std::fs::read_to_string(root.join("src/handlers/steps.rs")).unwrap();
    let dto = std::fs::read_to_string(root.join("src/dto.rs")).unwrap();
    for forbidden in [
        "TaskStepDto",
        "TaskExecutionPlanDto",
        "TaskStepsDto",
        "CreateStepBody",
        "UpdateStepBody",
        "ResolveStepDoneBody",
        "ResolveStepReasonBody",
    ] {
        assert!(
            !handler.contains(forbidden) && !dto.contains(forbidden),
            "private owner remains: {forbidden}"
        );
    }
    assert!(validate(&handler).is_empty(), "{:?}", validate(&handler));
    assert!(
        validate_api_step(&handler).is_empty(),
        "{:?}",
        validate_api_step(&handler)
    );
    assert!(
        validate_api_execution_plan(&handler).is_empty(),
        "{:?}",
        validate_api_execution_plan(&handler)
    );
}

#[test]
fn all_steps_handler_ast_hostile_mutations_are_rejected() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("src/handlers/steps.rs")).unwrap();
    assert!(
        validate(&source).is_empty(),
        "baseline: {:?}",
        validate(&source)
    );
    for endpoint in ENDPOINTS {
        let mutation = source.replacen(
            &format!("kanban_sqlite::api::{}(", endpoint.service),
            "kanban_sqlite::api::wrong_step_service(",
            1,
        );
        assert!(
            !validate(&mutation).is_empty(),
            "{} accepted cross-service mutation",
            endpoint.handler
        );
        let replacement = if endpoint.response == "ListStepsResponse" {
            "RemoveStepResponse"
        } else {
            "ListStepsResponse"
        };
        let mutation = source.replace(
            &format!("Json<{}>", endpoint.response),
            &format!("Json<{replacement}>"),
        );
        assert!(
            !validate(&mutation).is_empty(),
            "{} accepted response-owner mutation",
            endpoint.handler
        );
    }
    let identity_mutation = source.replace(
        "parent_task_id: step.parent_task_id",
        "parent_task_id: step.id",
    );
    assert!(
        !validate_api_step(&identity_mutation).is_empty(),
        "adapter identity mutation accepted"
    );
    for mutation in [
        source.replace(
            "resolution_note: step.resolution_note",
            "resolution_note: step.resolved_by",
        ),
        source.replace(
            "resolved_by: step.resolved_by",
            "resolved_by: step.updated_by",
        ),
        source.replace(
            ".map(crate::dto::api_task_from_record)",
            ".map(|_| unreachable!())",
        ),
        source.replace(
            "StepStatus::Todo => ApiStepStatus::Todo",
            "StepStatus::Todo => ApiStepStatus::Done",
        ),
    ] {
        assert!(
            !validate_api_step(&mutation).is_empty(),
            "api_step hostile mutation accepted"
        );
    }
    for mutation in [
        source.replace("board_id: plan.board_id", "board_id: plan.task_id"),
        source.replace("task_id: plan.task_id", "task_id: plan.board_id"),
        source.replace(
            "api_execution_plan_state_from_record(plan.state)",
            "api_execution_plan_state_from_record(Default::default())",
        ),
    ] {
        assert!(
            !validate_api_execution_plan(&mutation).is_empty(),
            "execution-plan hostile mutation accepted"
        );
    }
}
