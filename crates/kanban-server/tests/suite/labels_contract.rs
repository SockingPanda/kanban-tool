fn validate(source: &str) -> Vec<String> {
    use quote::ToTokens;
    let file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => return vec![error.to_string()],
    };
    let expected = syn::parse_file(VALID).expect("validator fixture parses");
    let mut violations = Vec::new();
    for expected in expected.items.iter().filter_map(|item| match item {
        syn::Item::Fn(function) => Some(function),
        _ => None,
    }) {
        let name = expected.sig.ident.to_string();
        let functions = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(f) if f.sig.ident == expected.sig.ident => Some(f),
                _ => None,
            })
            .collect::<Vec<_>>();
        if functions.len() != 1 {
            violations.push(format!("{name}:count"));
            continue;
        }
        let actual = functions[0];
        if actual.sig.to_token_stream().to_string() != expected.sig.to_token_stream().to_string() {
            violations.push(format!("{name}:exact-signature"));
        }
        if actual.block.to_token_stream().to_string()
            != expected.block.to_token_stream().to_string()
        {
            violations.push(format!("{name}:exact-dataflow"));
        }
    }
    violations
}

const VALID: &str = r#"
pub(crate) async fn list_task_labels(State(state):State<AppState>,Path(path):Path<kanban_contract::ListTaskLabelsPath>,)->Result<Json<DataEnvelope<Vec<ApiLabel>>>,ApiError>{let task=kanban_sqlite::api::get_task_by_id_global(state.db_path(),&path.task_id)?;Ok(Json(DataEnvelope::new(task.labels.into_iter().map(api_label_from_record).collect(),)))}
pub(crate) async fn add_task_label(State(state):State<AppState>,Path(path):Path<kanban_contract::AddTaskLabelPath>,headers:HeaderMap,body:Result<Json<kanban_contract::AddTaskLabelRequest>,JsonRejection>,)->Result<(StatusCode,Json<OptionalMetadataEnvelope<ApiTask,CreatedLabelsMeta<ApiLabel>>>,),ApiError,>{let Json(body)=body.map_err(extractor_error)?;let actor=actor(body.actor.as_deref(),&headers,&state);let label_names=body.label_names().map_err(invalid_input)?;let result=kanban_sqlite::api::add_task_labels_by_id_with_options(state.db_path(),&actor,&path.task_id,&label_names,body.create_missing,)?;let created_labels=result.created_labels.into_iter().map(api_label_from_record).collect::<Vec<_>>();let meta=if created_labels.is_empty(){None}else{Some(CreatedLabelsMeta{created_labels})};Ok((StatusCode::CREATED,Json(OptionalMetadataEnvelope::new(api_task_from_record(result.task)?,meta,)),))}
pub(crate) async fn remove_task_label(State(state):State<AppState>,Path(path):Path<kanban_contract::RemoveTaskLabelPath>,headers:HeaderMap,)->Result<Json<DataEnvelope<ApiTask>>,ApiError>{let actor=actor(None,&headers,&state);let task=kanban_sqlite::api::remove_task_label_by_id(state.db_path(),&actor,&path.task_id,&path.label_id,)?;Ok(Json(DataEnvelope::new(api_task_from_record(task)?)))}
"#;

#[test]
fn task_label_handlers_have_fail_closed_syn_ownership() {
    assert!(validate(VALID).is_empty(), "{:?}", validate(VALID));
    let mutations = [
        VALID.replace("ListTaskLabelsPath", "PrivateListPath"),
        VALID.replace("AddTaskLabelRequest", "PrivateAddBody"),
        VALID.replace("kanban_sqlite::api::get_task_by_id_global", "private_get_task"),
        VALID.replace("kanban_sqlite::api::add_task_labels_by_id_with_options", "private_add_labels"),
        VALID.replace("kanban_sqlite::api::remove_task_label_by_id", "private_remove_label"),
        VALID.replace("map(api_label_from_record)", "map(private_label_adapter)"),
        VALID.replace("api_task_from_record(result.task)", "private_task_adapter(result.task)"),
        VALID.replace("api_task_from_record(task)", "private_task_adapter(task)"),
        VALID.replace("Ok(Json(DataEnvelope::new(task.labels.into_iter().map(api_label_from_record).collect(),)))", "alternate_response()"),
        VALID.replace("Ok((StatusCode::CREATED", "return Ok((StatusCode::CREATED"),
        VALID.replace("OptionalMetadataEnvelope<ApiTask", "DataEnvelope<ApiTask"),
        VALID.replace("&path.task_id", "&\"constant-task\".to_owned()"),
        VALID.replace("body.create_missing", "false"),
        VALID.replace("&path.label_id", "&path.task_id"),
        VALID.replace("DataEnvelope<Vec<ApiLabel>>", "DataEnvelope<Vec<PrivateLabel>>"),
        VALID.replace("DataEnvelope<ApiTask>", "DataEnvelope<PrivateTask>"),
        VALID.replace("body.label_names()", "vec![\"constant\".to_owned()]"),
        VALID.replace("actor(body.actor.as_deref(),&headers,&state)", "actor(None,&headers,&state)"),
    ];
    for mutation in mutations {
        assert_ne!(mutation, VALID);
        syn::parse_file(&mutation).expect("hostile mutation must remain valid Rust syntax");
        assert!(
            !validate(&mutation).is_empty(),
            "mutation escaped validator: {mutation}"
        );
    }
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/tasks.rs"),
    )
    .unwrap();
    let violations = validate(&source);
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn label_handlers_do_not_redeclare_contract_owned_wire_types() {
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/tasks.rs"),
    )
    .unwrap();
    for forbidden in [
        "CreateLabelBody",
        "BootstrapTaskLabelBody",
        "BootstrapTaskLabelDto",
        "LabelProposalBody",
        "LabelProposalDecisionBody",
        "LabelOntologyActorBody",
        "LabelOntologyCandidateAtomBody",
        "LabelOntologySignalBody",
        "LabelOntologyObservationBody",
        "LabelOntologyActionBody",
        "LabelOntologyAtomApplyBody",
        "LabelOntologyRevertBody",
        "LabelOntologyValidationBody",
        "UpsertLabelSemanticsBody",
        "LabelSuggestionQuery",
        "LabelAtomIndexQuery",
        "LabelOntologySignalQuery",
        "SignalQuery",
        "LabelOntologyReviewQuery",
        "DeleteLabelSemanticsQuery",
    ] {
        assert!(
            !source.contains(&format!("struct {forbidden}")),
            "private public-wire mirror must stay removed: {forbidden}"
        );
    }
}
