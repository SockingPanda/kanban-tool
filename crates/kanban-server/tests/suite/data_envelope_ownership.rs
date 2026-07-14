use crate::common::*;
use kanban_contract::DataEnvelope;
use serde_json::Value;
use syn::{ItemFn, ItemUse, ReturnType, UseTree, visit::Visit};
const EXPECTED: &[(&str, usize, usize)] = &[
    ("context.rs", 1, 1),
    ("maintenance.rs", 1, 1),
    ("task_graph.rs", 2, 2),
    ("transitions.rs", 10, 11),
    ("vector.rs", 1, 1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportKind {
    Direct,
    Alias,
    Glob,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportEntry {
    path: Vec<String>,
    kind: ImportKind,
    binding: Option<String>,
    absolute: bool,
}

fn uses(t: &UseTree, p: Vec<String>, absolute: bool, o: &mut Vec<ImportEntry>) {
    match t {
        UseTree::Path(x) => {
            let mut q = p;
            q.push(x.ident.to_string());
            uses(&x.tree, q, absolute, o)
        }
        UseTree::Name(x) => {
            let mut q = p;
            q.push(x.ident.to_string());
            o.push(ImportEntry {
                path: q,
                kind: ImportKind::Direct,
                binding: Some(x.ident.to_string()),
                absolute,
            })
        }
        UseTree::Rename(x) => {
            let mut q = p;
            q.push(x.ident.to_string());
            o.push(ImportEntry {
                path: q,
                kind: ImportKind::Alias,
                binding: Some(x.rename.to_string()),
                absolute,
            })
        }
        UseTree::Group(x) => {
            for i in &x.items {
                uses(i, p.clone(), absolute, o)
            }
        }
        UseTree::Glob(_) => o.push(ImportEntry {
            path: p,
            kind: ImportKind::Glob,
            binding: None,
            absolute,
        }),
    }
}
fn isdata(p: &syn::Path) -> bool {
    p.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "DataEnvelope"
                | "TaskNeighborhoodResponse"
                | "BoardTaskMapResponse"
                | "SpecifyTaskResponse"
                | "PromoteTaskResponse"
                | "ReopenTaskResponse"
                | "UnblockTaskResponse"
                | "ArchiveTaskResponse"
                | "ReclaimTaskResponse"
                | "HeartbeatTaskResponse"
                | "CompleteTaskResponse"
                | "SubmitReviewTaskResponse"
                | "BlockTaskResponse"
                | "BuildContextResponse"
                | "StatsResponse"
                | "VectorStatusResponse"
        )
    })
}
#[derive(Default)]
struct C {
    r: usize,
    n: usize,
    e: usize,
}
impl<'a> Visit<'a> for C {
    fn visit_item_fn(&mut self, f: &'a ItemFn) {
        if let ReturnType::Type(_, t) = &f.sig.output {
            struct T(usize);
            impl<'a> Visit<'a> for T {
                fn visit_type_path(&mut self, x: &'a syn::TypePath) {
                    if isdata(&x.path) {
                        self.0 += 1
                    }
                    syn::visit::visit_type_path(self, x)
                }
            }
            let mut x = T(0);
            x.visit_type(t);
            self.r += x.0;
        }
        syn::visit::visit_item_fn(self, f)
    }
    fn visit_expr_call(&mut self, x: &'a syn::ExprCall) {
        if let syn::Expr::Path(p) = &*x.func {
            let s = &p.path.segments;
            if s.len() >= 2
                && s[s.len() - 2].ident == "DataEnvelope"
                && s.last().is_some_and(|z| z.ident == "new")
            {
                self.n += 1
            }
        }
        syn::visit::visit_expr_call(self, x)
    }
    fn visit_path(&mut self, p: &'a syn::Path) {
        if p.segments.last().is_some_and(|s| s.ident == "Envelope") {
            self.e += 1
        }
        syn::visit::visit_path(self, p)
    }
}
#[test]
fn g1_data_only_handlers_use_contract_data_envelope_exclusively() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
    let mut total = 0;
    for (f, expected_returns, expected_constructors) in EXPECTED {
        let a: syn::File =
            syn::parse_file(&std::fs::read_to_string(root.join(f)).unwrap()).unwrap();
        let mut u = Vec::new();
        for i in &a.items {
            if let syn::Item::Use(ItemUse {
                leading_colon,
                tree,
                ..
            }) = i
            {
                uses(tree, Vec::new(), leading_colon.is_some(), &mut u)
            }
        }
        let owner = u
            .iter()
            .filter(|entry| {
                entry.path
                    == vec![
                        String::from("kanban_contract"),
                        String::from("DataEnvelope"),
                    ]
                    && entry.kind == ImportKind::Direct
            })
            .count();
        assert_eq!(owner, 1, "{f}: owner import {owner}");
        assert!(
            !u.iter()
                .any(|entry| entry.path.ends_with(&["dto".into(), "Envelope".into()]))
        );
        let mut c = C::default();
        c.visit_file(&a);
        assert_eq!(c.e, 0, "{f}: private Envelope {}", c.e);
        assert_eq!(c.r, *expected_returns, "{f}: return {}", c.r);
        assert_eq!(c.n, *expected_constructors, "{f}: new {}", c.n);
        total += c.r;
    }
    assert_eq!(total, 15)
}
#[tokio::test]
async fn boards_router_bridge_is_data_only_contract_envelope() -> anyhow::Result<()> {
    let t = TestApp::new()?;
    let (st, v) = get_json(t.router(), "/api/v1/boards").await?;
    assert_eq!(st, axum::http::StatusCode::OK);
    let o = v.as_object().unwrap();
    assert_eq!(o.len(), 1);
    assert!(o.contains_key("data"));
    let _: DataEnvelope<Value> = serde_json::from_value(v)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanonicalOwner {
    Local,
    Contract,
    CrateDto,
    KanbanEntity,
    KanbanSqliteApi,
    PreludeVec,
    PreludeOption,
}

fn owner_segments_match(actual: &[&syn::PathSegment], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(segment, expected)| {
            segment.ident == expected && matches!(segment.arguments, syn::PathArguments::None)
        })
}

fn canonical_owner_matches(actual: &[&syn::PathSegment], owner: CanonicalOwner) -> bool {
    match owner {
        CanonicalOwner::Local => false,
        CanonicalOwner::Contract => owner_segments_match(actual, &["kanban_contract"]),
        CanonicalOwner::CrateDto => owner_segments_match(actual, &["crate", "dto"]),
        CanonicalOwner::KanbanEntity => owner_segments_match(actual, &["kanban_entity"]),
        CanonicalOwner::KanbanSqliteApi => owner_segments_match(actual, &["kanban_sqlite", "api"]),
        CanonicalOwner::PreludeVec => {
            owner_segments_match(actual, &["std", "vec"])
                || owner_segments_match(actual, &["alloc", "vec"])
        }
        CanonicalOwner::PreludeOption => {
            owner_segments_match(actual, &["std", "option"])
                || owner_segments_match(actual, &["core", "option"])
        }
    }
}

fn canonical_path_segment<'a>(
    path: &'a syn::Path,
    owner: CanonicalOwner,
    leaf: &str,
    suffix: Option<&str>,
    shadowed: &std::collections::BTreeSet<String>,
) -> Option<&'a syn::PathSegment> {
    let segments = path.segments.iter().collect::<Vec<_>>();
    let suffix_len = usize::from(suffix.is_some());
    if segments.len() <= suffix_len {
        return None;
    }
    if let Some(suffix) = suffix {
        let suffix_segment = segments.last()?;
        if suffix_segment.ident != suffix
            || !matches!(suffix_segment.arguments, syn::PathArguments::None)
        {
            return None;
        }
    }
    let leaf_index = segments.len() - suffix_len - 1;
    let leaf_segment = segments[leaf_index];
    if leaf_segment.ident != leaf {
        return None;
    }
    let owner_segments = &segments[..leaf_index];
    if owner_segments.is_empty() {
        if path.leading_colon.is_some() || shadowed.contains("*") || shadowed.contains(leaf) {
            return None;
        }
    } else {
        if path.leading_colon.is_none()
            && (shadowed.contains("*") || shadowed.contains(&owner_segments[0].ident.to_string()))
        {
            return None;
        }
        if !canonical_owner_matches(owner_segments, owner) {
            return None;
        }
    }
    Some(leaf_segment)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeSpec {
    Named {
        owner: CanonicalOwner,
        name: &'static str,
    },
    VecOf(&'static TypeSpec),
    OptionOf(&'static TypeSpec),
    Generic {
        owner: CanonicalOwner,
        name: &'static str,
        args: &'static [TypeSpec],
    },
}

const fn named(owner: CanonicalOwner, name: &'static str) -> TypeSpec {
    TypeSpec::Named { owner, name }
}

fn type_path_matches_args(
    actual: &syn::TypePath,
    expected_owner: CanonicalOwner,
    expected_name: &str,
    expected_args: &[TypeSpec],
    shadowed: &std::collections::BTreeSet<String>,
) -> bool {
    if actual.qself.is_some() {
        return false;
    }
    let Some(segment) =
        canonical_path_segment(&actual.path, expected_owner, expected_name, None, shadowed)
    else {
        return false;
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    segment.ident == expected_name
        && arguments.colon2_token.is_none()
        && arguments.args.len() == expected_args.len()
        && arguments
            .args
            .iter()
            .zip(expected_args)
            .all(|(actual, expected)| {
                matches!(
                    actual,
                    syn::GenericArgument::Type(ty)
                        if type_matches_in_scope(ty, *expected, shadowed)
                )
            })
}

fn type_matches_in_scope(
    actual: &syn::Type,
    expected: TypeSpec,
    shadowed: &std::collections::BTreeSet<String>,
) -> bool {
    match (actual, expected) {
        (syn::Type::Path(path), TypeSpec::Named { owner, name }) => {
            path.qself.is_none()
                && canonical_path_segment(&path.path, owner, name, None, shadowed)
                    .is_some_and(|segment| matches!(segment.arguments, syn::PathArguments::None))
        }
        (syn::Type::Path(path), TypeSpec::VecOf(inner)) => type_path_matches_args(
            path,
            CanonicalOwner::PreludeVec,
            "Vec",
            std::slice::from_ref(inner),
            shadowed,
        ),
        (syn::Type::Path(path), TypeSpec::OptionOf(inner)) => type_path_matches_args(
            path,
            CanonicalOwner::PreludeOption,
            "Option",
            std::slice::from_ref(inner),
            shadowed,
        ),
        (syn::Type::Path(path), TypeSpec::Generic { owner, name, args }) => {
            type_path_matches_args(path, owner, name, args, shadowed)
        }
        _ => false,
    }
}

fn type_matches(actual: &syn::Type, expected: TypeSpec) -> bool {
    type_matches_in_scope(actual, expected, &std::collections::BTreeSet::new())
}

static RELATION: TypeSpec = named(CanonicalOwner::KanbanEntity, "Relation");
static VEC_RELATION: TypeSpec = TypeSpec::VecOf(&RELATION);
static SEARCH_TASKS_DTO: TypeSpec = named(CanonicalOwner::CrateDto, "SearchTasksDto");
static SEARCH_TASK_STATUS_WINDOWS_DTO: TypeSpec =
    named(CanonicalOwner::Local, "SearchTaskStatusWindowsDto");
static TASK_DTO: TypeSpec = named(CanonicalOwner::Contract, "ApiTask");
static VEC_TASK_DTO: TypeSpec = TypeSpec::VecOf(&TASK_DTO);
static TASK_STATUS_WINDOWS_DTO: TypeSpec = named(CanonicalOwner::Local, "TaskStatusWindowsDto");
static LABEL_DTO: TypeSpec = named(CanonicalOwner::Contract, "ApiLabel");
static SIGNAL_RECORD: TypeSpec = named(CanonicalOwner::KanbanSqliteApi, "SignalRecord");
static VEC_SIGNAL_RECORD: TypeSpec = TypeSpec::VecOf(&SIGNAL_RECORD);
static LABEL_ONTOLOGY_REVIEW_GROUP: TypeSpec =
    named(CanonicalOwner::KanbanSqliteApi, "LabelOntologyReviewGroup");
static VEC_LABEL_ONTOLOGY_REVIEW_GROUP: TypeSpec = TypeSpec::VecOf(&LABEL_ONTOLOGY_REVIEW_GROUP);
static TASK_ONTOLOGY_SUMMARY: TypeSpec =
    named(CanonicalOwner::KanbanSqliteApi, "TaskOntologySummary");
static OPTIONAL_TASK_ONTOLOGY_SUMMARY: TypeSpec = TypeSpec::OptionOf(&TASK_ONTOLOGY_SUMMARY);
static LIMIT_META: TypeSpec = named(CanonicalOwner::Contract, "LimitMeta");
static OFFSET_META: TypeSpec = named(CanonicalOwner::Contract, "OffsetPaginationMeta");
static TOTAL_META: TypeSpec = named(CanonicalOwner::Contract, "TotalPaginationMeta");
static SIGNAL_FILTER_META: TypeSpec = named(CanonicalOwner::Contract, "SignalFilterMeta");
static LABEL_ONTOLOGY_REVIEW_META: TypeSpec =
    named(CanonicalOwner::Contract, "LabelOntologyReviewMeta");
static CREATED_LABELS_META_ARGS: [TypeSpec; 1] = [LABEL_DTO];
static CREATED_LABELS_META: TypeSpec = TypeSpec::Generic {
    owner: CanonicalOwner::Contract,
    name: "CreatedLabelsMeta",
    args: &CREATED_LABELS_META_ARGS,
};
static TASK_ONTOLOGY_DETAILS_META_ARGS: [TypeSpec; 1] = [OPTIONAL_TASK_ONTOLOGY_SUMMARY];
static TASK_ONTOLOGY_DETAILS_META: TypeSpec = TypeSpec::Generic {
    owner: CanonicalOwner::Contract,
    name: "TaskOntologyDetailsMeta",
    args: &TASK_ONTOLOGY_DETAILS_META_ARGS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequiredMetadataSpec {
    function: &'static str,
    data: TypeSpec,
    meta: TypeSpec,
    fields: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptionalMetadataSpec {
    function: &'static str,
    data: TypeSpec,
    meta: TypeSpec,
}

static G3_GRAPH: &[RequiredMetadataSpec] = &[RequiredMetadataSpec {
    function: "graph_neighbors",
    data: VEC_RELATION,
    meta: LIMIT_META,
    fields: &["limit"],
}];
static G3_SEARCH: &[RequiredMetadataSpec] = &[
    RequiredMetadataSpec {
        function: "search_tasks",
        data: SEARCH_TASKS_DTO,
        meta: OFFSET_META,
        fields: &["limit", "offset"],
    },
    RequiredMetadataSpec {
        function: "search_tasks_by_status",
        data: SEARCH_TASK_STATUS_WINDOWS_DTO,
        meta: OFFSET_META,
        fields: &["limit", "offset"],
    },
];
static G3_TASKS: &[RequiredMetadataSpec] = &[
    RequiredMetadataSpec {
        function: "list_tasks",
        data: VEC_TASK_DTO,
        meta: TOTAL_META,
        fields: &["limit", "offset", "total"],
    },
    RequiredMetadataSpec {
        function: "list_tasks_by_status",
        data: TASK_STATUS_WINDOWS_DTO,
        meta: OFFSET_META,
        fields: &["limit", "offset"],
    },
];

static G4_TASKS: &[RequiredMetadataSpec] = &[
    RequiredMetadataSpec {
        function: "list_signals",
        data: VEC_SIGNAL_RECORD,
        meta: SIGNAL_FILTER_META,
        fields: &["include_all", "limit"],
    },
    RequiredMetadataSpec {
        function: "review_signals",
        data: VEC_SIGNAL_RECORD,
        meta: SIGNAL_FILTER_META,
        fields: &["include_all", "limit"],
    },
    RequiredMetadataSpec {
        function: "review_label_ontology",
        data: VEC_LABEL_ONTOLOGY_REVIEW_GROUP,
        meta: LABEL_ONTOLOGY_REVIEW_META,
        fields: &["group_by", "include_all", "limit"],
    },
];

static G4_TASKS_OPTIONAL: &[OptionalMetadataSpec] = &[
    OptionalMetadataSpec {
        function: "add_task_label",
        data: TASK_DTO,
        meta: CREATED_LABELS_META,
    },
    OptionalMetadataSpec {
        function: "get_task",
        data: TASK_DTO,
        meta: TASK_ONTOLOGY_DETAILS_META,
    },
];

static TASK_DATA_ONLY: &[&str] = &[
    "create_task",
    "list_board_labels",
    "create_board_label",
    "list_label_semantics",
    "get_label_semantics",
    "upsert_label_semantics",
    "delete_label_semantics",
    "list_label_atoms",
    "explain_label_atom",
    "label_atom_index_status",
    "rebuild_label_atom_index",
    "query_label_atom_index",
    "list_task_labels",
    "suggest_task_labels",
    "bootstrap_task_label",
    "propose_task_label",
    "list_task_label_proposals",
    "record_label_ontology_observation",
    "list_label_ontology_signals",
    "get_signal",
    "get_label_ontology_signal",
    "create_label_ontology_action",
    "apply_label_ontology_atom",
    "revert_label_ontology_mutation",
    "validate_label_ontology_action",
    "get_label_proposal",
    "accept_label_proposal",
    "reject_label_proposal",
    "remove_task_label",
    "update_task",
];

#[derive(Debug)]
struct HandlerOwnership {
    file: &'static str,
    data_only: &'static [&'static str],
    private_metadata: &'static [&'static str],
    required_metadata: &'static [RequiredMetadataSpec],
    optional_metadata: &'static [OptionalMetadataSpec],
}

const G4_CATALOG: &[HandlerOwnership] = &[
    HandlerOwnership {
        file: "graph.rs",
        data_only: &["graph_status"],
        private_metadata: &[],
        required_metadata: G3_GRAPH,
        optional_metadata: &[],
    },
    HandlerOwnership {
        file: "search.rs",
        data_only: &["search_status"],
        private_metadata: &[],
        required_metadata: G3_SEARCH,
        optional_metadata: &[],
    },
    HandlerOwnership {
        file: "tasks.rs",
        data_only: TASK_DATA_ONLY,
        private_metadata: &[],
        required_metadata: G4_TASKS,
        optional_metadata: G4_TASKS_OPTIONAL,
    },
    HandlerOwnership {
        file: "events.rs",
        data_only: &[],
        private_metadata: &[],
        required_metadata: &[],
        optional_metadata: &[],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandlerKind {
    DataOnly,
    PrivateMetadata,
    RequiredMetadata(RequiredMetadataSpec),
    OptionalMetadata(OptionalMetadataSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeKind {
    Data,
    TypedDataAlias,
    Private,
    Metadata,
    OptionalMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViolationKind {
    Parse,
    DuplicateCatalog,
    DuplicateFunction,
    MissingSource,
    DataImportCount,
    DataImportAlias,
    PrivateImportCount,
    PrivateImportAlias,
    MetadataEnvelopeImportCount,
    MetadataEnvelopeImportAlias,
    OptionalMetadataEnvelopeImportCount,
    OptionalMetadataEnvelopeImportAlias,
    RequiredMetaImportCount,
    RequiredMetaImportAlias,
    OwnerNamespaceGlob,
    MissingFunction,
    MissingEnvelopeResponse,
    UnregisteredEnvelopeResponse,
    WrongReturnEnvelope,
    WrongDataConstructorCount,
    WrongDataBodyPathCount,
    DataStructLiteral,
    ForbiddenPrivateBodyPath,
    WrongPrivateLiteralCount,
    WrongPrivateLiteralFields,
    WrongPrivateBodyPathCount,
    ForbiddenDataBodyPath,
    ForbiddenContractMetadataBodyPath,
    WrongRequiredMetadataConstructor,
    WrongRequiredMetadataBodyType,
    WrongRequiredMetadataFields,
    ForbiddenRequiredMetadataFamily,
    ForbiddenRequiredMetadataLiteral,
    WrongOptionalMetadataConstructor,
    ForbiddenOptionalMetadataFamily,
    ForbiddenOptionalMetadataLiteral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    kind: ViolationKind,
    file: String,
    function: Option<String>,
    detail: String,
}

fn push_violation(
    violations: &mut Vec<Violation>,
    kind: ViolationKind,
    file: &str,
    function: Option<&str>,
    detail: impl Into<String>,
) {
    violations.push(Violation {
        kind,
        file: file.to_owned(),
        function: function.map(str::to_owned),
        detail: detail.into(),
    });
}

#[derive(Clone, Copy)]
struct EnvelopePathMatch<'a> {
    kind: EnvelopeKind,
    constructor: bool,
    segment: &'a syn::PathSegment,
}

fn envelope_path<'a>(
    path: &'a syn::Path,
    has_qself: bool,
    shadowed: &std::collections::BTreeSet<String>,
) -> Option<EnvelopePathMatch<'a>> {
    if has_qself {
        return None;
    }
    let specs = [
        (EnvelopeKind::Data, CanonicalOwner::Contract, "DataEnvelope"),
        (
            EnvelopeKind::TypedDataAlias,
            CanonicalOwner::Contract,
            "DeleteResponse",
        ),
        (
            EnvelopeKind::TypedDataAlias,
            CanonicalOwner::Contract,
            "CreateTaskResponse",
        ),
        (
            EnvelopeKind::TypedDataAlias,
            CanonicalOwner::Contract,
            "LabelOntologySignalsResponse",
        ),
        (EnvelopeKind::Private, CanonicalOwner::CrateDto, "Envelope"),
        (
            EnvelopeKind::Metadata,
            CanonicalOwner::Contract,
            "MetadataEnvelope",
        ),
        (
            EnvelopeKind::OptionalMetadata,
            CanonicalOwner::Contract,
            "OptionalMetadataEnvelope",
        ),
    ];
    for (kind, owner, leaf) in specs {
        for (suffix, constructor) in [(None, false), (Some("new"), true)] {
            if let Some(segment) = canonical_path_segment(path, owner, leaf, suffix, shadowed) {
                return Some(EnvelopePathMatch {
                    kind,
                    constructor,
                    segment,
                });
            }
        }
    }
    const CONTRACT_DATA_RESPONSES: &[&str] = &[
        "GraphStatusResponse",
        "GraphNeighborsResponse",
        "SearchStatusResponse",
        "SearchTasksResponse",
        "SearchTasksByStatusResponse",
        "ListBoardLabelsResponse",
        "CreateBoardLabelResponse",
        "ListLabelSemanticsResponse",
        "GetLabelSemanticsResponse",
        "UpsertLabelSemanticsResponse",
        "ListLabelAtomsResponse",
        "ExplainLabelAtomResponse",
        "LabelAtomIndexStatusResponse",
        "RebuildLabelAtomIndexResponse",
        "QueryLabelAtomIndexResponse",
        "SuggestTaskLabelsResponse",
        "BootstrapTaskLabelResponse",
        "ProposeTaskLabelResponse",
        "ListTaskLabelProposalsResponse",
        "RecordLabelOntologyObservationResponse",
        "ListSignalsResponse",
        "ReviewSignalsResponse",
        "ReviewLabelOntologyResponse",
        "GetSignalResponse",
        "GetLabelOntologySignalResponse",
        "LabelOntologyActionResponse",
        "GetLabelProposalResponse",
        "LabelProposalDecisionResponse",
        "UpdateTaskResponse",
        "GetTaskResponse",
    ];
    for leaf in CONTRACT_DATA_RESPONSES {
        for (suffix, constructor) in [(None, false), (Some("new"), true)] {
            if let Some(segment) =
                canonical_path_segment(path, CanonicalOwner::Contract, leaf, suffix, shadowed)
            {
                return Some(EnvelopePathMatch {
                    kind: EnvelopeKind::TypedDataAlias,
                    constructor,
                    segment,
                });
            }
        }
    }
    None
}

fn exact_data_response_alias(function: &str) -> Option<&'static str> {
    Some(match function {
        "graph_status" => "GraphStatusResponse",
        "search_status" => "SearchStatusResponse",
        "create_task" => "CreateTaskResponse",
        "list_board_labels" => "ListBoardLabelsResponse",
        "create_board_label" => "CreateBoardLabelResponse",
        "list_label_semantics" => "ListLabelSemanticsResponse",
        "get_label_semantics" => "GetLabelSemanticsResponse",
        "upsert_label_semantics" => "UpsertLabelSemanticsResponse",
        "delete_label_semantics" => "DeleteResponse",
        "list_label_atoms" => "ListLabelAtomsResponse",
        "explain_label_atom" => "ExplainLabelAtomResponse",
        "label_atom_index_status" => "LabelAtomIndexStatusResponse",
        "rebuild_label_atom_index" => "RebuildLabelAtomIndexResponse",
        "query_label_atom_index" => "QueryLabelAtomIndexResponse",
        "suggest_task_labels" => "SuggestTaskLabelsResponse",
        "bootstrap_task_label" => "BootstrapTaskLabelResponse",
        "propose_task_label" => "ProposeTaskLabelResponse",
        "list_task_label_proposals" => "ListTaskLabelProposalsResponse",
        "record_label_ontology_observation" => "RecordLabelOntologyObservationResponse",
        "list_label_ontology_signals" => "LabelOntologySignalsResponse",
        "get_signal" => "GetSignalResponse",
        "get_label_ontology_signal" => "GetLabelOntologySignalResponse",
        "create_label_ontology_action"
        | "apply_label_ontology_atom"
        | "revert_label_ontology_mutation"
        | "validate_label_ontology_action" => "LabelOntologyActionResponse",
        "get_label_proposal" => "GetLabelProposalResponse",
        "accept_label_proposal" | "reject_label_proposal" => "LabelProposalDecisionResponse",
        "update_task" => "UpdateTaskResponse",
        _ => return None,
    })
}

fn exact_metadata_response_alias(function: &str) -> Option<&'static str> {
    Some(match function {
        "graph_neighbors" => "GraphNeighborsResponse",
        "search_tasks" => "SearchTasksResponse",
        "search_tasks_by_status" => "SearchTasksByStatusResponse",
        "list_signals" => "ListSignalsResponse",
        "review_signals" => "ReviewSignalsResponse",
        "review_label_ontology" => "ReviewLabelOntologyResponse",
        _ => return None,
    })
}

fn exact_optional_metadata_response_alias(function: &str) -> Option<&'static str> {
    Some(match function {
        "get_task" => "GetTaskResponse",
        _ => return None,
    })
}

fn envelope_kind(
    path: &syn::Path,
    has_qself: bool,
    shadowed: &std::collections::BTreeSet<String>,
) -> Option<EnvelopeKind> {
    envelope_path(path, has_qself, shadowed).map(|matched| matched.kind)
}

#[derive(Default)]
struct ReturnShape {
    data: usize,
    typed_data_alias: usize,
    typed_data_alias_names: Vec<String>,
    private: usize,
    metadata: usize,
    optional_metadata: usize,
    metadata_args: Vec<[syn::Type; 2]>,
    malformed_metadata: bool,
    optional_metadata_args: Vec<[syn::Type; 2]>,
    malformed_optional_metadata: bool,
    shadowed: std::collections::BTreeSet<String>,
}

impl ReturnShape {
    fn total(&self) -> usize {
        self.data + self.typed_data_alias + self.private + self.metadata + self.optional_metadata
    }
    fn is_exact_data(&self) -> bool {
        self.data == 1
            && self.typed_data_alias == 0
            && self.private == 0
            && self.metadata == 0
            && self.optional_metadata == 0
    }
    fn is_exact_typed_response(&self, name: &str) -> bool {
        self.data == 0
            && self.typed_data_alias == 1
            && self.typed_data_alias_names == [name]
            && self.private == 0
            && self.metadata == 0
            && self.optional_metadata == 0
    }
    fn is_exact_private(&self) -> bool {
        self.data == 0
            && self.typed_data_alias == 0
            && self.private == 1
            && self.metadata == 0
            && self.optional_metadata == 0
    }
    fn is_exact_metadata(&self) -> bool {
        self.data == 0
            && self.typed_data_alias == 0
            && self.private == 0
            && self.metadata == 1
            && self.optional_metadata == 0
    }
    fn is_exact_optional_metadata(&self) -> bool {
        self.data == 0
            && self.typed_data_alias == 0
            && self.private == 0
            && self.metadata == 0
            && self.optional_metadata == 1
    }
    fn is_exact_required(&self, spec: RequiredMetadataSpec) -> bool {
        if !self.is_exact_metadata() || self.malformed_metadata || self.metadata_args.len() != 1 {
            return false;
        }
        let [data, meta] = &self.metadata_args[0];
        type_matches_in_scope(data, spec.data, &self.shadowed)
            && type_matches_in_scope(meta, spec.meta, &self.shadowed)
    }
    fn is_exact_optional(&self, spec: OptionalMetadataSpec) -> bool {
        if !self.is_exact_optional_metadata()
            || self.malformed_optional_metadata
            || self.optional_metadata_args.len() != 1
        {
            return false;
        }
        let [data, meta] = &self.optional_metadata_args[0];
        type_matches_in_scope(data, spec.data, &self.shadowed)
            && type_matches_in_scope(meta, spec.meta, &self.shadowed)
    }
}

fn envelope_type_arguments(segment: &syn::PathSegment) -> Option<[syn::Type; 2]> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.colon2_token.is_some() || arguments.args.len() != 2 {
        return None;
    }
    let mut arguments = arguments.args.iter();
    let syn::GenericArgument::Type(data) = arguments.next()? else {
        return None;
    };
    let syn::GenericArgument::Type(meta) = arguments.next()? else {
        return None;
    };
    if arguments.next().is_some() {
        return None;
    }
    Some([data.clone(), meta.clone()])
}

impl<'ast> Visit<'ast> for ReturnShape {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        let matched = envelope_path(&node.path, node.qself.is_some(), &self.shadowed);
        match matched {
            Some(EnvelopePathMatch {
                kind: EnvelopeKind::Data,
                ..
            }) => self.data += 1,
            Some(EnvelopePathMatch {
                kind: EnvelopeKind::TypedDataAlias,
                segment,
                ..
            }) => {
                self.typed_data_alias += 1;
                self.typed_data_alias_names.push(segment.ident.to_string());
            }
            Some(EnvelopePathMatch {
                kind: EnvelopeKind::Private,
                ..
            }) => self.private += 1,
            Some(EnvelopePathMatch {
                kind: EnvelopeKind::Metadata,
                segment,
                ..
            }) => {
                self.metadata += 1;
                if let Some(arguments) = envelope_type_arguments(segment) {
                    self.metadata_args.push(arguments);
                } else {
                    self.malformed_metadata = true;
                }
            }
            Some(EnvelopePathMatch {
                kind: EnvelopeKind::OptionalMetadata,
                segment,
                ..
            }) => {
                self.optional_metadata += 1;
                if let Some(arguments) = envelope_type_arguments(segment) {
                    self.optional_metadata_args.push(arguments);
                } else {
                    self.malformed_optional_metadata = true;
                }
            }
            None => {}
        }
        syn::visit::visit_type_path(self, node);
    }
}

fn signature_type_shadows(function: &ItemFn) -> std::collections::BTreeSet<String> {
    function
        .sig
        .generics
        .params
        .iter()
        .filter_map(|parameter| match parameter {
            syn::GenericParam::Type(parameter) => Some(parameter.ident.to_string()),
            syn::GenericParam::Lifetime(_) | syn::GenericParam::Const(_) => None,
        })
        .collect()
}

fn return_shape_in_scope(
    function: &ItemFn,
    module_shadows: &std::collections::BTreeSet<String>,
) -> ReturnShape {
    let mut shadowed = module_shadows.clone();
    shadowed.extend(signature_type_shadows(function));
    let mut shape = ReturnShape {
        shadowed,
        ..ReturnShape::default()
    };
    if let ReturnType::Type(_, ty) = &function.sig.output {
        shape.visit_type(ty);
    }
    shape
}

#[derive(Clone)]
struct MetaStructShape {
    path: syn::Path,
    has_qself: bool,
    fields: Vec<String>,
    has_unnamed: bool,
    has_rest: bool,
}

impl std::fmt::Debug for MetaStructShape {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        formatter
            .debug_struct("MetaStructShape")
            .field("path", &path)
            .field("fields", &self.fields)
            .field("has_unnamed", &self.has_unnamed)
            .field("has_rest", &self.has_rest)
            .finish()
    }
}

#[derive(Debug, Clone)]
struct MetadataCallShape {
    arity: usize,
    meta_struct: Option<MetaStructShape>,
}

#[derive(Debug, Clone)]
struct BodyShape {
    data_new_calls: usize,
    metadata_calls: Vec<MetadataCallShape>,
    optional_metadata_calls: Vec<usize>,
    data_paths: usize,
    private_paths: usize,
    metadata_paths: usize,
    optional_metadata_paths: usize,
    data_literals: usize,
    private_literals: usize,
    metadata_literals: usize,
    optional_metadata_literals: usize,
    private_literal_fields_valid: bool,
    shadowed: std::collections::BTreeSet<String>,
}

impl Default for BodyShape {
    fn default() -> Self {
        Self {
            data_new_calls: 0,
            metadata_calls: Vec::new(),
            optional_metadata_calls: Vec::new(),
            data_paths: 0,
            private_paths: 0,
            metadata_paths: 0,
            optional_metadata_paths: 0,
            data_literals: 0,
            private_literals: 0,
            metadata_literals: 0,
            optional_metadata_literals: 0,
            private_literal_fields_valid: true,
            shadowed: std::collections::BTreeSet::new(),
        }
    }
}

impl BodyShape {
    fn uses_envelope(&self) -> bool {
        self.data_paths
            + self.private_paths
            + self.metadata_paths
            + self.optional_metadata_paths
            + self.data_literals
            + self.private_literals
            + self.metadata_literals
            + self.optional_metadata_literals
            + self.data_new_calls
            > 0
            || !self.metadata_calls.is_empty()
            || !self.optional_metadata_calls.is_empty()
    }

    fn record_path(&mut self, path: &syn::Path, has_qself: bool) {
        match envelope_kind(path, has_qself, &self.shadowed) {
            Some(EnvelopeKind::Data) => self.data_paths += 1,
            Some(EnvelopeKind::TypedDataAlias) => {}
            Some(EnvelopeKind::Private) => self.private_paths += 1,
            Some(EnvelopeKind::Metadata) => self.metadata_paths += 1,
            Some(EnvelopeKind::OptionalMetadata) => self.optional_metadata_paths += 1,
            None => {}
        }
    }
}

impl<'ast> Visit<'ast> for BodyShape {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let matched = envelope_path(&path.path, path.qself.is_some(), &self.shadowed);
            if matched
                .is_some_and(|matched| matched.kind == EnvelopeKind::Data && matched.constructor)
            {
                self.data_new_calls += 1;
            }
            if matched.is_some_and(|matched| {
                matched.kind == EnvelopeKind::Metadata && matched.constructor
            }) {
                let meta_struct = node.args.iter().nth(1).and_then(|argument| {
                    let syn::Expr::Struct(meta) = argument else {
                        return None;
                    };
                    let mut fields = Vec::new();
                    let mut has_unnamed = false;
                    for field in &meta.fields {
                        match &field.member {
                            syn::Member::Named(name) => fields.push(name.to_string()),
                            syn::Member::Unnamed(_) => has_unnamed = true,
                        }
                    }
                    Some(MetaStructShape {
                        path: meta.path.clone(),
                        has_qself: meta.qself.is_some(),
                        fields,
                        has_unnamed,
                        has_rest: meta.rest.is_some(),
                    })
                });
                self.metadata_calls.push(MetadataCallShape {
                    arity: node.args.len(),
                    meta_struct,
                });
            }
            if matched.is_some_and(|matched| {
                matched.kind == EnvelopeKind::OptionalMetadata && matched.constructor
            }) {
                self.optional_metadata_calls.push(node.args.len());
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        self.record_path(&node.path, node.qself.is_some());
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        self.record_path(&node.path, node.qself.is_some());
        syn::visit::visit_type_path(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let kind = envelope_kind(&node.path, node.qself.is_some(), &self.shadowed);
        self.record_path(&node.path, node.qself.is_some());
        if kind == Some(EnvelopeKind::Data) {
            self.data_literals += 1;
        }
        if kind == Some(EnvelopeKind::Private) {
            self.private_literals += 1;
            let mut names = node
                .fields
                .iter()
                .filter_map(|field| match &field.member {
                    syn::Member::Named(name) => Some(name.to_string()),
                    syn::Member::Unnamed(_) => None,
                })
                .collect::<Vec<_>>();
            names.sort();
            let fields_valid = node.rest.is_none()
                && node.fields.len() == 2
                && names.len() == 2
                && names[0] == "data"
                && names[1] == "meta";
            self.private_literal_fields_valid &= fields_valid;
        }
        if kind == Some(EnvelopeKind::Metadata) {
            self.metadata_literals += 1;
        }
        if kind == Some(EnvelopeKind::OptionalMetadata) {
            self.optional_metadata_literals += 1;
        }
        syn::visit::visit_expr_struct(self, node);
    }
}

fn collect_use_bindings(tree: &UseTree, shadowed: &mut std::collections::BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => collect_use_bindings(&path.tree, shadowed),
        UseTree::Name(name) => {
            shadowed.insert(name.ident.to_string());
        }
        UseTree::Rename(rename) => {
            shadowed.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_bindings(item, shadowed);
            }
        }
        UseTree::Glob(_) => {
            shadowed.insert("*".to_owned());
        }
    }
}

fn extern_crate_binding(item: &syn::ItemExternCrate) -> &syn::Ident {
    item.rename
        .as_ref()
        .map_or(&item.ident, |(_, rename)| rename)
}

#[derive(Default)]
struct LocalTypeShadows {
    names: std::collections::BTreeSet<String>,
}

impl LocalTypeShadows {
    fn record(&mut self, ident: &syn::Ident) {
        self.names.insert(ident.to_string());
    }
}

impl<'ast> Visit<'ast> for LocalTypeShadows {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        collect_use_bindings(&node.tree, &mut self.names);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.record(&node.ident);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.record(&node.ident);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.record(&node.ident);
    }

    fn visit_item_union(&mut self, node: &'ast syn::ItemUnion) {
        self.record(&node.ident);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.record(&node.ident);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.record(&node.ident);
    }

    fn visit_item_extern_crate(&mut self, node: &'ast syn::ItemExternCrate) {
        self.record(extern_crate_binding(node));
    }

    fn visit_item_fn(&mut self, _node: &'ast ItemFn) {}
}

fn body_shadows(
    function: &ItemFn,
    module_shadows: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut names = module_shadows.clone();
    names.extend(signature_type_shadows(function));
    let mut collector = LocalTypeShadows { names };
    collector.visit_block(&function.block);
    collector.names
}

fn body_shape_in_scope(
    function: &ItemFn,
    module_shadows: &std::collections::BTreeSet<String>,
) -> BodyShape {
    let mut shape = BodyShape {
        shadowed: body_shadows(function, module_shadows),
        ..BodyShape::default()
    };
    shape.visit_block(&function.block);
    shape
}

fn body_shape(function: &ItemFn) -> BodyShape {
    body_shape_in_scope(function, &std::collections::BTreeSet::new())
}

fn meta_path_matches(
    path: &syn::Path,
    has_qself: bool,
    expected: TypeSpec,
    shadowed: &std::collections::BTreeSet<String>,
) -> bool {
    if has_qself {
        return false;
    }
    match expected {
        TypeSpec::Named { owner, name } => {
            canonical_path_segment(path, owner, name, None, shadowed)
                .is_some_and(|segment| matches!(segment.arguments, syn::PathArguments::None))
        }
        TypeSpec::VecOf(_) | TypeSpec::OptionOf(_) | TypeSpec::Generic { .. } => false,
    }
}

fn validate_required_body(
    body: &BodyShape,
    spec: RequiredMetadataSpec,
    file: &str,
    function: &str,
    violations: &mut Vec<Violation>,
) {
    let constructor = if body.metadata_calls.len() == 1 {
        body.metadata_calls.first()
    } else {
        None
    };
    if constructor.is_none() || constructor.is_some_and(|call| call.arity != 2) {
        let arities = body
            .metadata_calls
            .iter()
            .map(|call| call.arity)
            .collect::<Vec<_>>();
        push_violation(
            violations,
            ViolationKind::WrongRequiredMetadataConstructor,
            file,
            Some(function),
            format!(
                "expected one two-argument MetadataEnvelope::new call, found {} with arities {arities:?}",
                body.metadata_calls.len()
            ),
        );
    }

    if body.data_paths != 0
        || body.private_paths != 0
        || body.optional_metadata_paths != 0
        || body.data_new_calls != 0
        || body.data_literals != 0
        || body.private_literals != 0
        || body.metadata_paths != 1
    {
        push_violation(
            violations,
            ViolationKind::ForbiddenRequiredMetadataFamily,
            file,
            Some(function),
            format!(
                "required metadata body paths: data={}, private={}, metadata={}, optional={}; data constructors={}, data literals={}, private literals={}",
                body.data_paths,
                body.private_paths,
                body.metadata_paths,
                body.optional_metadata_paths,
                body.data_new_calls,
                body.data_literals,
                body.private_literals
            ),
        );
    }
    if body.metadata_literals != 0 {
        push_violation(
            violations,
            ViolationKind::ForbiddenRequiredMetadataLiteral,
            file,
            Some(function),
            format!(
                "found {} MetadataEnvelope struct literals",
                body.metadata_literals
            ),
        );
    }

    let Some(call) = constructor else {
        return;
    };
    let Some(meta) = &call.meta_struct else {
        push_violation(
            violations,
            ViolationKind::WrongRequiredMetadataBodyType,
            file,
            Some(function),
            "metadata argument must be a direct struct expression",
        );
        return;
    };
    if !meta_path_matches(&meta.path, meta.has_qself, spec.meta, &body.shadowed) {
        push_violation(
            violations,
            ViolationKind::WrongRequiredMetadataBodyType,
            file,
            Some(function),
            format!("metadata struct path does not match {:?}", spec.meta),
        );
    }

    let mut actual_fields = meta.fields.clone();
    actual_fields.sort();
    let mut expected_fields = spec.fields.to_vec();
    expected_fields.sort();
    if meta.has_unnamed
        || meta.has_rest
        || actual_fields.len() != spec.fields.len()
        || actual_fields
            .iter()
            .map(String::as_str)
            .ne(expected_fields.iter().copied())
    {
        push_violation(
            violations,
            ViolationKind::WrongRequiredMetadataFields,
            file,
            Some(function),
            format!(
                "metadata fields={actual_fields:?}, expected={expected_fields:?}, unnamed={}, rest={}",
                meta.has_unnamed, meta.has_rest
            ),
        );
    }
}

fn validate_optional_body(
    body: &BodyShape,
    file: &str,
    function: &str,
    violations: &mut Vec<Violation>,
) {
    if body.optional_metadata_calls.len() != 1
        || body
            .optional_metadata_calls
            .first()
            .is_some_and(|arity| *arity != 2)
    {
        push_violation(
            violations,
            ViolationKind::WrongOptionalMetadataConstructor,
            file,
            Some(function),
            format!(
                "expected one two-argument OptionalMetadataEnvelope::new call, found {} with arities {:?}",
                body.optional_metadata_calls.len(),
                body.optional_metadata_calls
            ),
        );
    }

    if body.data_paths != 0
        || body.private_paths != 0
        || body.metadata_paths != 0
        || body.optional_metadata_paths != 1
        || body.data_new_calls != 0
        || body.data_literals != 0
        || body.private_literals != 0
        || !body.metadata_calls.is_empty()
    {
        push_violation(
            violations,
            ViolationKind::ForbiddenOptionalMetadataFamily,
            file,
            Some(function),
            format!(
                "optional metadata body paths: data={}, private={}, metadata={}, optional={}; data constructors={}, data literals={}, private literals={}, required constructors={}",
                body.data_paths,
                body.private_paths,
                body.metadata_paths,
                body.optional_metadata_paths,
                body.data_new_calls,
                body.data_literals,
                body.private_literals,
                body.metadata_calls.len()
            ),
        );
    }

    if body.optional_metadata_literals != 0 {
        push_violation(
            violations,
            ViolationKind::ForbiddenOptionalMetadataLiteral,
            file,
            Some(function),
            format!(
                "found {} OptionalMetadataEnvelope struct literals",
                body.optional_metadata_literals
            ),
        );
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ImportShape {
    data_direct: usize,
    data_alias: usize,
    private_direct: usize,
    private_alias: usize,
    metadata_direct: usize,
    metadata_alias: usize,
    optional_metadata_direct: usize,
    optional_metadata_alias: usize,
    required_meta_direct: std::collections::BTreeMap<String, usize>,
    required_meta_alias: std::collections::BTreeMap<String, usize>,
    owner_globs: Vec<Vec<String>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ImportExpectation {
    data: bool,
    private: bool,
    metadata: bool,
    optional_metadata: bool,
    meta_types: std::collections::BTreeSet<&'static str>,
}

fn import_path_is(path: &[String], expected: &[&str]) -> bool {
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn record_import_kind(kind: ImportKind, direct: &mut usize, alias: &mut usize) {
    match kind {
        ImportKind::Direct => *direct += 1,
        ImportKind::Alias => *alias += 1,
        ImportKind::Glob => {}
    }
}

fn owner_namespace_glob(path: &[String]) -> bool {
    path.first()
        .is_some_and(|segment| segment == "kanban_contract")
        || path
            .get(..2)
            .is_some_and(|prefix| prefix == ["crate", "dto"])
}

fn file_imports(file: &syn::File) -> Vec<ImportEntry> {
    let mut imports = Vec::new();
    for item in &file.items {
        if let syn::Item::Use(ItemUse {
            leading_colon,
            tree,
            ..
        }) = item
        {
            uses(tree, Vec::new(), leading_colon.is_some(), &mut imports);
        }
    }
    imports
}

fn import_shape(
    file: &syn::File,
    known_meta_types: &std::collections::BTreeSet<&'static str>,
) -> ImportShape {
    let mut shape = ImportShape::default();
    for entry in file_imports(file) {
        if entry.kind == ImportKind::Glob && owner_namespace_glob(&entry.path) {
            shape.owner_globs.push(entry.path.clone());
        }
        if import_path_is(&entry.path, &["kanban_contract", "DataEnvelope"]) {
            record_import_kind(entry.kind, &mut shape.data_direct, &mut shape.data_alias);
        }
        if import_path_is(&entry.path, &["crate", "dto", "Envelope"]) {
            record_import_kind(
                entry.kind,
                &mut shape.private_direct,
                &mut shape.private_alias,
            );
        }
        if import_path_is(&entry.path, &["kanban_contract", "MetadataEnvelope"]) {
            record_import_kind(
                entry.kind,
                &mut shape.metadata_direct,
                &mut shape.metadata_alias,
            );
        }
        if import_path_is(
            &entry.path,
            &["kanban_contract", "OptionalMetadataEnvelope"],
        ) {
            record_import_kind(
                entry.kind,
                &mut shape.optional_metadata_direct,
                &mut shape.optional_metadata_alias,
            );
        }
        if entry.path.len() == 2
            && entry
                .path
                .first()
                .is_some_and(|root| root == "kanban_contract")
            && entry
                .path
                .last()
                .is_some_and(|name| known_meta_types.contains(name.as_str()))
        {
            let name = entry.path[1].clone();
            let counts = match entry.kind {
                ImportKind::Direct => &mut shape.required_meta_direct,
                ImportKind::Alias => &mut shape.required_meta_alias,
                ImportKind::Glob => continue,
            };
            *counts.entry(name).or_default() += 1;
        }
    }
    shape
}

fn metadata_type_name(spec: TypeSpec) -> Option<&'static str> {
    match spec {
        TypeSpec::Named { name, .. } | TypeSpec::Generic { name, .. } => Some(name),
        TypeSpec::VecOf(_) | TypeSpec::OptionOf(_) => None,
    }
}

fn catalog_meta_types(catalog: &[HandlerOwnership]) -> std::collections::BTreeSet<&'static str> {
    catalog
        .iter()
        .flat_map(|entry| {
            entry
                .required_metadata
                .iter()
                .map(|spec| spec.meta)
                .chain(entry.optional_metadata.iter().map(|spec| spec.meta))
        })
        .filter_map(metadata_type_name)
        .collect()
}

fn record_type_owners(
    spec: TypeSpec,
    owners: &mut std::collections::BTreeMap<&'static str, CanonicalOwner>,
) {
    match spec {
        TypeSpec::Named { owner, name } => {
            owners.insert(name, owner);
        }
        TypeSpec::VecOf(inner) => {
            owners.insert("Vec", CanonicalOwner::PreludeVec);
            record_type_owners(*inner, owners);
        }
        TypeSpec::OptionOf(inner) => {
            owners.insert("Option", CanonicalOwner::PreludeOption);
            record_type_owners(*inner, owners);
        }
        TypeSpec::Generic { owner, name, args } => {
            owners.insert(name, owner);
            for argument in args {
                record_type_owners(*argument, owners);
            }
        }
    }
}

fn catalog_type_owners(
    catalog: &[HandlerOwnership],
) -> std::collections::BTreeMap<&'static str, CanonicalOwner> {
    let mut owners = std::collections::BTreeMap::from([
        ("DataEnvelope", CanonicalOwner::Contract),
        ("DeleteResponse", CanonicalOwner::Contract),
        ("CreateTaskResponse", CanonicalOwner::Contract),
        ("LabelOntologySignalsResponse", CanonicalOwner::Contract),
        ("Envelope", CanonicalOwner::CrateDto),
        ("MetadataEnvelope", CanonicalOwner::Contract),
        ("OptionalMetadataEnvelope", CanonicalOwner::Contract),
    ]);
    for entry in catalog {
        for spec in entry.required_metadata {
            record_type_owners(spec.data, &mut owners);
            record_type_owners(spec.meta, &mut owners);
        }
        for spec in entry.optional_metadata {
            record_type_owners(spec.data, &mut owners);
            record_type_owners(spec.meta, &mut owners);
        }
    }
    owners
}

fn import_source_matches_owner(path: &[String], owner: CanonicalOwner, leaf: &str) -> bool {
    let Some((actual_leaf, owner_segments)) = path.split_last() else {
        return false;
    };
    if actual_leaf != leaf {
        return false;
    }
    let expected = |segments: &[&str]| {
        owner_segments.len() == segments.len()
            && owner_segments
                .iter()
                .zip(segments)
                .all(|(actual, expected)| actual == expected)
    };
    match owner {
        CanonicalOwner::Local => false,
        CanonicalOwner::Contract => expected(&["kanban_contract"]),
        CanonicalOwner::CrateDto => expected(&["crate", "dto"]),
        CanonicalOwner::KanbanEntity => expected(&["kanban_entity"]),
        CanonicalOwner::KanbanSqliteApi => expected(&["kanban_sqlite", "api"]),
        CanonicalOwner::PreludeVec => expected(&["std", "vec"]) || expected(&["alloc", "vec"]),
        CanonicalOwner::PreludeOption => {
            expected(&["std", "option"]) || expected(&["core", "option"])
        }
    }
}

fn import_glob_namespace_is_known(path: &[String]) -> bool {
    import_path_is(path, &["kanban_contract"])
        || import_path_is(path, &["crate", "dto"])
        || import_path_is(path, &["kanban_entity"])
        || import_path_is(path, &["kanban_sqlite", "api"])
        || import_path_is(path, &["std", "vec"])
        || import_path_is(path, &["alloc", "vec"])
        || import_path_is(path, &["std", "option"])
        || import_path_is(path, &["core", "option"])
}

fn import_root_is_proven(
    entry: &ImportEntry,
    owner_root_shadows: &std::collections::BTreeSet<String>,
    has_glob_uncertainty: bool,
) -> bool {
    let Some(root) = entry.path.first() else {
        return false;
    };
    root == "crate"
        || entry.absolute
        || (!has_glob_uncertainty && !owner_root_shadows.contains(root))
}

fn import_source_is_proven(
    entry: &ImportEntry,
    owner: CanonicalOwner,
    leaf: &str,
    owner_root_shadows: &std::collections::BTreeSet<String>,
    has_glob_uncertainty: bool,
) -> bool {
    import_source_matches_owner(&entry.path, owner, leaf)
        && import_root_is_proven(entry, owner_root_shadows, has_glob_uncertainty)
}

fn module_type_shadows(
    file: &syn::File,
    owners: &std::collections::BTreeMap<&'static str, CanonicalOwner>,
) -> std::collections::BTreeSet<String> {
    const OWNER_ROOTS: &[&str] = &[
        "kanban_contract",
        "kanban_entity",
        "kanban_sqlite",
        "std",
        "core",
        "alloc",
    ];
    let imports = file_imports(file);
    let is_shadow_target = |name: &str| {
        OWNER_ROOTS.contains(&name)
            || owners
                .get(name)
                .is_some_and(|owner| *owner != CanonicalOwner::Local)
    };
    let mut item_shadows = std::collections::BTreeSet::new();
    for item in &file.items {
        let binding = match item {
            syn::Item::Enum(item) => Some(&item.ident),
            syn::Item::ExternCrate(item) => Some(extern_crate_binding(item)),
            syn::Item::Mod(item) => Some(&item.ident),
            syn::Item::Struct(item) => Some(&item.ident),
            syn::Item::Trait(item) => Some(&item.ident),
            syn::Item::Type(item) => Some(&item.ident),
            syn::Item::Union(item) => Some(&item.ident),
            _ => None,
        };
        let Some(binding) = binding else {
            continue;
        };
        let name = binding.to_string();
        let canonical_owner_extern = match item {
            syn::Item::ExternCrate(item) => {
                item.rename.is_none() && OWNER_ROOTS.iter().any(|root| item.ident == root)
            }
            _ => false,
        };
        if !canonical_owner_extern && is_shadow_target(&name) {
            item_shadows.insert(name);
        }
    }

    let mut owner_root_shadows = imports
        .iter()
        .filter_map(|entry| {
            let binding = entry.binding.as_deref()?;
            (OWNER_ROOTS.contains(&binding) && (entry.path.len() != 1 || entry.path[0] != binding))
                .then(|| binding.to_owned())
        })
        .collect::<std::collections::BTreeSet<_>>();
    owner_root_shadows.extend(
        item_shadows
            .iter()
            .filter(|name| OWNER_ROOTS.contains(&name.as_str()))
            .cloned(),
    );

    let has_textual_foreign_glob = imports.iter().any(|entry| {
        entry.kind == ImportKind::Glob && !import_glob_namespace_is_known(&entry.path)
    });
    let mut shadowed = item_shadows;
    shadowed.extend(owner_root_shadows.iter().cloned());
    for entry in &imports {
        let Some(binding) = entry.binding.as_deref() else {
            continue;
        };
        let Some(owner) = owners.get(binding).copied() else {
            continue;
        };
        if import_source_is_proven(
            entry,
            owner,
            binding,
            &owner_root_shadows,
            has_textual_foreign_glob,
        ) {
            continue;
        }
        if import_source_matches_owner(&entry.path, owner, binding) {
            shadowed.insert(binding.to_owned());
            continue;
        }
        let has_dedicated_import_gate = owner == CanonicalOwner::Contract
            || (owner == CanonicalOwner::CrateDto && binding == "Envelope");
        let has_proven_canonical_direct_binding = imports.iter().any(|candidate| {
            candidate.kind == ImportKind::Direct
                && candidate.binding.as_deref() == Some(binding)
                && import_source_is_proven(
                    candidate,
                    owner,
                    binding,
                    &owner_root_shadows,
                    has_textual_foreign_glob,
                )
        });
        if !has_dedicated_import_gate || has_proven_canonical_direct_binding {
            shadowed.insert(binding.to_owned());
        }
    }

    let has_untrusted_glob = imports.iter().any(|entry| {
        entry.kind == ImportKind::Glob
            && (!import_glob_namespace_is_known(&entry.path)
                || !import_root_is_proven(entry, &owner_root_shadows, has_textual_foreign_glob))
    });
    if has_untrusted_glob {
        shadowed.extend(OWNER_ROOTS.iter().map(|root| (*root).to_owned()));
        for (&leaf, &owner) in owners {
            if owner == CanonicalOwner::Local {
                continue;
            }
            let has_explicit_canonical_direct_binding = imports.iter().any(|entry| {
                entry.kind == ImportKind::Direct
                    && entry.binding.as_deref() == Some(leaf)
                    && import_source_is_proven(
                        entry,
                        owner,
                        leaf,
                        &owner_root_shadows,
                        has_untrusted_glob,
                    )
            });
            if !has_explicit_canonical_direct_binding {
                shadowed.insert(leaf.to_owned());
            }
        }
    }
    shadowed
}

fn import_expectation(
    handlers: &std::collections::BTreeMap<&'static str, HandlerKind>,
) -> ImportExpectation {
    let mut expectation = ImportExpectation::default();
    for kind in handlers.values() {
        match kind {
            HandlerKind::DataOnly => expectation.data = true,
            HandlerKind::PrivateMetadata => expectation.private = true,
            HandlerKind::RequiredMetadata(spec) => {
                expectation.metadata = true;
                if let Some(name) = metadata_type_name(spec.meta) {
                    expectation.meta_types.insert(name);
                }
            }
            HandlerKind::OptionalMetadata(spec) => {
                expectation.optional_metadata = true;
                let name = metadata_type_name(spec.meta).expect("optional metadata must be named");
                expectation.meta_types.insert(name);
            }
        }
    }
    expectation
}

fn expected_handlers(
    catalog: &[HandlerOwnership],
    violations: &mut Vec<Violation>,
) -> std::collections::BTreeMap<&'static str, std::collections::BTreeMap<&'static str, HandlerKind>>
{
    let mut expected = std::collections::BTreeMap::new();
    for entry in catalog {
        let handlers = expected
            .entry(entry.file)
            .or_insert_with(std::collections::BTreeMap::new);
        for (name, kind) in entry
            .data_only
            .iter()
            .map(|name| (*name, HandlerKind::DataOnly))
            .chain(
                entry
                    .private_metadata
                    .iter()
                    .map(|name| (*name, HandlerKind::PrivateMetadata))
                    .chain(
                        entry
                            .required_metadata
                            .iter()
                            .map(|spec| (spec.function, HandlerKind::RequiredMetadata(*spec))),
                    )
                    .chain(
                        entry
                            .optional_metadata
                            .iter()
                            .map(|spec| (spec.function, HandlerKind::OptionalMetadata(*spec))),
                    ),
            )
        {
            if handlers.insert(name, kind).is_some() {
                push_violation(
                    violations,
                    ViolationKind::DuplicateCatalog,
                    entry.file,
                    Some(name),
                    "handler appears more than once in the catalog",
                );
            }
        }
    }
    expected
}

fn validate_direct_import_count(
    violations: &mut Vec<Violation>,
    kind: ViolationKind,
    file: &str,
    path: &str,
    expected: bool,
    actual: usize,
) {
    let expected = usize::from(expected);
    if actual != expected {
        push_violation(
            violations,
            kind,
            file,
            None,
            format!("path={path} expected={expected} actual={actual} kind=direct"),
        );
    }
}

fn validate_import_alias(
    violations: &mut Vec<Violation>,
    kind: ViolationKind,
    file: &str,
    path: &str,
    actual: usize,
) {
    if actual != 0 {
        push_violation(
            violations,
            kind,
            file,
            None,
            format!("path={path} expected=0 actual={actual} kind=alias"),
        );
    }
}

fn validate_import_shape(
    file: &str,
    imports: &ImportShape,
    expectation: &ImportExpectation,
    known_meta_types: &std::collections::BTreeSet<&'static str>,
    violations: &mut Vec<Violation>,
) {
    validate_direct_import_count(
        violations,
        ViolationKind::DataImportCount,
        file,
        "kanban_contract::DataEnvelope",
        expectation.data,
        imports.data_direct,
    );
    validate_import_alias(
        violations,
        ViolationKind::DataImportAlias,
        file,
        "kanban_contract::DataEnvelope",
        imports.data_alias,
    );
    validate_direct_import_count(
        violations,
        ViolationKind::PrivateImportCount,
        file,
        "crate::dto::Envelope",
        expectation.private,
        imports.private_direct,
    );
    validate_import_alias(
        violations,
        ViolationKind::PrivateImportAlias,
        file,
        "crate::dto::Envelope",
        imports.private_alias,
    );
    validate_direct_import_count(
        violations,
        ViolationKind::MetadataEnvelopeImportCount,
        file,
        "kanban_contract::MetadataEnvelope",
        expectation.metadata,
        imports.metadata_direct,
    );
    validate_import_alias(
        violations,
        ViolationKind::MetadataEnvelopeImportAlias,
        file,
        "kanban_contract::MetadataEnvelope",
        imports.metadata_alias,
    );
    validate_direct_import_count(
        violations,
        ViolationKind::OptionalMetadataEnvelopeImportCount,
        file,
        "kanban_contract::OptionalMetadataEnvelope",
        expectation.optional_metadata,
        imports.optional_metadata_direct,
    );
    validate_import_alias(
        violations,
        ViolationKind::OptionalMetadataEnvelopeImportAlias,
        file,
        "kanban_contract::OptionalMetadataEnvelope",
        imports.optional_metadata_alias,
    );
    for &name in known_meta_types {
        let path = format!("kanban_contract::{name}");
        validate_direct_import_count(
            violations,
            ViolationKind::RequiredMetaImportCount,
            file,
            &path,
            expectation.meta_types.contains(name),
            imports.required_meta_direct.get(name).copied().unwrap_or(0),
        );
        validate_import_alias(
            violations,
            ViolationKind::RequiredMetaImportAlias,
            file,
            &path,
            imports.required_meta_alias.get(name).copied().unwrap_or(0),
        );
    }
    for path in &imports.owner_globs {
        push_violation(
            violations,
            ViolationKind::OwnerNamespaceGlob,
            file,
            None,
            format!("path={} expected=0 actual=1 kind=glob", path.join("::")),
        );
    }
}

fn validate_catalog(
    sources: &std::collections::BTreeMap<&str, String>,
    catalog: &[HandlerOwnership],
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let known_meta_types = catalog_meta_types(catalog);
    let known_type_owners = catalog_type_owners(catalog);
    let expected = expected_handlers(catalog, &mut violations);

    for (file_name, handlers) in expected {
        let Some(source) = sources.get(file_name) else {
            push_violation(
                &mut violations,
                ViolationKind::MissingSource,
                file_name,
                None,
                "source was not supplied to the validator",
            );
            continue;
        };
        let file = match syn::parse_file(source) {
            Ok(file) => file,
            Err(error) => {
                push_violation(
                    &mut violations,
                    ViolationKind::Parse,
                    file_name,
                    None,
                    error.to_string(),
                );
                continue;
            }
        };
        let module_shadows = module_type_shadows(&file, &known_type_owners);

        let expectation = import_expectation(&handlers);
        let imports = import_shape(&file, &known_meta_types);
        validate_import_shape(
            file_name,
            &imports,
            &expectation,
            &known_meta_types,
            &mut violations,
        );

        let mut functions = std::collections::BTreeMap::new();
        for item in &file.items {
            if let syn::Item::Fn(function) = item {
                let name = function.sig.ident.to_string();
                if functions.insert(name.clone(), function).is_some() {
                    push_violation(
                        &mut violations,
                        ViolationKind::DuplicateFunction,
                        file_name,
                        Some(&name),
                        "top-level function name is duplicated",
                    );
                }
            }
        }

        for (name, function) in &functions {
            let registered = handlers
                .keys()
                .any(|expected_name| *expected_name == name.as_str());
            let returned = return_shape_in_scope(function, &module_shadows);
            let body = body_shape_in_scope(function, &module_shadows);
            if (returned.total() != 0 || body.uses_envelope()) && !registered {
                push_violation(
                    &mut violations,
                    ViolationKind::UnregisteredEnvelopeResponse,
                    file_name,
                    Some(name),
                    "envelope response function is absent from the catalog",
                );
            }
        }

        for (name, expected_kind) in handlers {
            let Some(function) = functions.get(name) else {
                push_violation(
                    &mut violations,
                    ViolationKind::MissingFunction,
                    file_name,
                    Some(name),
                    "catalog function is missing from the source",
                );
                continue;
            };
            let returned = return_shape_in_scope(function, &module_shadows);
            if returned.total() == 0 {
                push_violation(
                    &mut violations,
                    ViolationKind::MissingEnvelopeResponse,
                    file_name,
                    Some(name),
                    "return signature has no recognized envelope",
                );
            }
            let body = body_shape_in_scope(function, &module_shadows);
            match expected_kind {
                HandlerKind::DataOnly => {
                    let exact_return = exact_data_response_alias(name).map_or_else(
                        || returned.is_exact_data(),
                        |alias| returned.is_exact_typed_response(alias),
                    );
                    if !exact_return {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongReturnEnvelope,
                            file_name,
                            Some(name),
                            format!(
                                "expected exactly one DataEnvelope, found {}",
                                returned.total()
                            ),
                        );
                    }
                    let expected_data_paths =
                        usize::from(name != "create_task" && name != "list_label_ontology_signals");
                    if body.data_new_calls != expected_data_paths {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongDataConstructorCount,
                            file_name,
                            Some(name),
                            format!(
                                "expected {expected_data_paths} DataEnvelope::new call(s), found {}",
                                body.data_new_calls
                            ),
                        );
                    }
                    if body.data_paths != expected_data_paths {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongDataBodyPathCount,
                            file_name,
                            Some(name),
                            format!(
                                "expected {expected_data_paths} DataEnvelope body path(s), found {}",
                                body.data_paths
                            ),
                        );
                    }
                    if body.data_literals != 0 {
                        push_violation(
                            &mut violations,
                            ViolationKind::DataStructLiteral,
                            file_name,
                            Some(name),
                            format!("found {} DataEnvelope struct literals", body.data_literals),
                        );
                    }
                    if body.private_paths != 0 {
                        push_violation(
                            &mut violations,
                            ViolationKind::ForbiddenPrivateBodyPath,
                            file_name,
                            Some(name),
                            format!("found {} private Envelope body paths", body.private_paths),
                        );
                    }
                    if body.metadata_paths != 0 || body.optional_metadata_paths != 0 {
                        push_violation(
                            &mut violations,
                            ViolationKind::ForbiddenContractMetadataBodyPath,
                            file_name,
                            Some(name),
                            format!(
                                "found metadata paths: required={}, optional={}",
                                body.metadata_paths, body.optional_metadata_paths
                            ),
                        );
                    }
                }
                HandlerKind::PrivateMetadata => {
                    if !returned.is_exact_private() {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongReturnEnvelope,
                            file_name,
                            Some(name),
                            format!(
                                "expected exactly one private Envelope, found {}",
                                returned.total()
                            ),
                        );
                    }
                    if body.private_literals != 1 {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongPrivateLiteralCount,
                            file_name,
                            Some(name),
                            format!(
                                "expected one private Envelope literal, found {}",
                                body.private_literals
                            ),
                        );
                    }
                    if !body.private_literal_fields_valid {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongPrivateLiteralFields,
                            file_name,
                            Some(name),
                            "private Envelope literal must have only data and meta without rest",
                        );
                    }
                    if body.private_paths != 1 {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongPrivateBodyPathCount,
                            file_name,
                            Some(name),
                            format!(
                                "expected one private Envelope body path, found {}",
                                body.private_paths
                            ),
                        );
                    }
                    if body.data_paths != 0 || body.data_new_calls != 0 || body.data_literals != 0 {
                        push_violation(
                            &mut violations,
                            ViolationKind::ForbiddenDataBodyPath,
                            file_name,
                            Some(name),
                            format!(
                                "found DataEnvelope body usage: paths={}, constructors={}, literals={}",
                                body.data_paths, body.data_new_calls, body.data_literals
                            ),
                        );
                    }
                    if body.metadata_paths != 0 || body.optional_metadata_paths != 0 {
                        push_violation(
                            &mut violations,
                            ViolationKind::ForbiddenContractMetadataBodyPath,
                            file_name,
                            Some(name),
                            format!(
                                "found metadata paths: required={}, optional={}",
                                body.metadata_paths, body.optional_metadata_paths
                            ),
                        );
                    }
                }
                HandlerKind::RequiredMetadata(spec) => {
                    let exact_return = exact_metadata_response_alias(name).map_or_else(
                        || returned.is_exact_metadata() && returned.is_exact_required(spec),
                        |alias| returned.is_exact_typed_response(alias),
                    );
                    if !exact_return {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongReturnEnvelope,
                            file_name,
                            Some(name),
                            format!("required metadata return mismatch: {}", returned.total()),
                        );
                    }
                    validate_required_body(&body, spec, file_name, name, &mut violations);
                }
                HandlerKind::OptionalMetadata(spec) => {
                    let exact_return = exact_optional_metadata_response_alias(name).map_or_else(
                        || {
                            returned.is_exact_optional_metadata()
                                && returned.is_exact_optional(spec)
                        },
                        |alias| returned.is_exact_typed_response(alias),
                    );
                    if !exact_return {
                        push_violation(
                            &mut violations,
                            ViolationKind::WrongReturnEnvelope,
                            file_name,
                            Some(name),
                            format!("optional metadata return mismatch: {}", returned.total()),
                        );
                    }
                    validate_optional_body(&body, file_name, name, &mut violations);
                }
            }
        }
    }

    violations
}

fn production_sources(files: &[&'static str]) -> std::collections::BTreeMap<&'static str, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
    files
        .iter()
        .copied()
        .map(|file| {
            let source = std::fs::read_to_string(root.join(file))
                .unwrap_or_else(|error| panic!("failed to read {file}: {error}"));
            (file, source)
        })
        .collect()
}

fn production_g4_sources() -> std::collections::BTreeMap<&'static str, String> {
    production_sources(&["graph.rs", "search.rs", "tasks.rs", "events.rs"])
}

#[test]
fn private_server_envelope_type_is_removed() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dto.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let file = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let definitions = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "Envelope" => Some("struct"),
            syn::Item::Type(item) if item.ident == "Envelope" => Some("type"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        definitions.is_empty(),
        "src/dto.rs still defines private Envelope items: {definitions:?}"
    );
}

fn is_frontier_handler_violation(kind: ViolationKind) -> bool {
    matches!(
        kind,
        ViolationKind::WrongReturnEnvelope
            | ViolationKind::WrongRequiredMetadataConstructor
            | ViolationKind::WrongRequiredMetadataBodyType
            | ViolationKind::WrongRequiredMetadataFields
            | ViolationKind::ForbiddenRequiredMetadataFamily
            | ViolationKind::ForbiddenRequiredMetadataLiteral
            | ViolationKind::WrongOptionalMetadataConstructor
            | ViolationKind::ForbiddenOptionalMetadataFamily
            | ViolationKind::ForbiddenOptionalMetadataLiteral
    )
}

fn assert_only_frontier_violations(
    violations: &[Violation],
    frontier_handlers: &[&str],
    frontier_imports: &[(&str, ViolationKind, &str)],
    label: &str,
) {
    for violation in violations {
        let allowed = if let Some(function) = violation.function.as_deref() {
            frontier_handlers.contains(&function) && is_frontier_handler_violation(violation.kind)
        } else {
            frontier_imports.iter().any(|(file, kind, path)| {
                violation.file == *file
                    && violation.kind == *kind
                    && violation.detail.starts_with(&format!("path={path} "))
            })
        };
        assert!(allowed, "{label}: {violation:#?}");
    }
}

#[test]
fn g4_delete_response_alias_is_exactly_scoped_to_delete_label_semantics() {
    let mut sources = production_g4_sources();
    let tasks = sources.get_mut("tasks.rs").expect("tasks source");
    let original = ") -> Result<Json<DeleteResponse>, ApiError> {";
    assert!(tasks.contains(original));
    *tasks = tasks.replace(
        original,
        ") -> Result<Json<DataEnvelope<DeleteResult>>, ApiError> {",
    );
    let violations = validate_catalog(&sources, G4_CATALOG);
    assert!(contains_file_violation(
        &violations,
        "tasks.rs",
        ViolationKind::WrongReturnEnvelope,
        Some("delete_label_semantics"),
    ));
}

#[test]
fn g4_delete_response_alias_is_rejected_for_other_data_only_handlers() {
    let mut sources = production_g4_sources();
    let tasks = sources.get_mut("tasks.rs").expect("tasks source");
    let original = ") -> Result<Json<kanban_contract::ListLabelAtomsResponse>, ApiError> {";
    assert!(tasks.contains(original));
    *tasks = tasks.replace(original, ") -> Result<Json<DeleteResponse>, ApiError> {");
    let violations = validate_catalog(&sources, G4_CATALOG);
    assert!(contains_file_violation(
        &violations,
        "tasks.rs",
        ViolationKind::WrongReturnEnvelope,
        Some("list_label_atoms"),
    ));
}

#[test]
fn g4_all_handlers_use_contract_owned_envelopes() {
    assert_eq!(
        G4_CATALOG
            .iter()
            .map(|entry| entry.data_only.len())
            .sum::<usize>(),
        32
    );
    assert_eq!(
        G4_CATALOG
            .iter()
            .map(|entry| entry.required_metadata.len())
            .sum::<usize>(),
        6
    );
    assert_eq!(
        G4_CATALOG
            .iter()
            .map(|entry| entry.optional_metadata.len())
            .sum::<usize>(),
        2
    );
    assert_eq!(
        G4_CATALOG
            .iter()
            .map(|entry| entry.private_metadata.len())
            .sum::<usize>(),
        0
    );

    let violations = validate_catalog(&production_g4_sources(), G4_CATALOG);
    assert!(
        violations.is_empty(),
        "G4 response ownership violations:\n{violations:#?}"
    );
}

static SYNTHETIC_REQUIRED: &[RequiredMetadataSpec] = &[RequiredMetadataSpec {
    function: "required_handler",
    data: VEC_RELATION,
    meta: OFFSET_META,
    fields: &["limit", "offset"],
}];

static SYNTHETIC_OPTIONAL: &[OptionalMetadataSpec] = &[
    OptionalMetadataSpec {
        function: "optional_handler",
        data: TASK_DTO,
        meta: CREATED_LABELS_META,
    },
    OptionalMetadataSpec {
        function: "nested_optional_handler",
        data: TASK_DTO,
        meta: TASK_ONTOLOGY_DETAILS_META,
    },
];

static A3_FAMILY_REQUIRED: &[RequiredMetadataSpec] = &[RequiredMetadataSpec {
    function: "required_handler",
    data: VEC_RELATION,
    meta: OFFSET_META,
    fields: &["limit", "offset"],
}];

const A3_FAMILY_CATALOG: &[HandlerOwnership] = &[
    HandlerOwnership {
        file: "data.rs",
        data_only: &["data_handler"],
        private_metadata: &[],
        required_metadata: &[],
        optional_metadata: &[],
    },
    HandlerOwnership {
        file: "required.rs",
        data_only: &[],
        private_metadata: &[],
        required_metadata: A3_FAMILY_REQUIRED,
        optional_metadata: &[],
    },
];

static A3_MULTI_META_REQUIRED: &[RequiredMetadataSpec] = &[
    RequiredMetadataSpec {
        function: "required_handler",
        data: VEC_RELATION,
        meta: OFFSET_META,
        fields: &["limit", "offset"],
    },
    RequiredMetadataSpec {
        function: "repeated_meta_handler",
        data: VEC_RELATION,
        meta: OFFSET_META,
        fields: &["limit", "offset"],
    },
    RequiredMetadataSpec {
        function: "limit_meta_handler",
        data: VEC_RELATION,
        meta: LIMIT_META,
        fields: &["limit"],
    },
];

const A3_MULTI_META_CATALOG: &[HandlerOwnership] = &[HandlerOwnership {
    file: "multi.rs",
    data_only: &[],
    private_metadata: &[],
    required_metadata: A3_MULTI_META_REQUIRED,
    optional_metadata: &[],
}];

const SYNTHETIC_CATALOG: &[HandlerOwnership] = &[
    HandlerOwnership {
        file: "fixture.rs",
        data_only: &["data_handler"],
        private_metadata: &["meta_handler"],
        required_metadata: SYNTHETIC_REQUIRED,
        optional_metadata: &[],
    },
    HandlerOwnership {
        file: "optional.rs",
        data_only: &[],
        private_metadata: &[],
        required_metadata: &[],
        optional_metadata: SYNTHETIC_OPTIONAL,
    },
];

const VALID_SYNTHETIC_SOURCE: &str = r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};

fn data_handler() -> DataEnvelope<i32> {
    DataEnvelope::new(1)
}

fn meta_handler() -> Envelope<i32> {
    Envelope { data: 1, meta: Some(2) }
}

fn required_handler(
) -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {
    MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta { limit: 10, offset: 0 },
    )
}
"#;

const SYNTHETIC_IMPORTS: &str = r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#;
const VALID_DATA_HANDLER: &str = "fn data_handler() -> DataEnvelope<i32> { DataEnvelope::new(1) }";
const VALID_PRIVATE_HANDLER: &str =
    "fn meta_handler() -> Envelope<i32> { Envelope { data: 1, meta: Some(2) } }";
const VALID_REQUIRED_HANDLER: &str = r#"
fn required_handler(
) -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {
    MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta { limit: 10, offset: 0 },
    )
}
"#;

const SYNTHETIC_OPTIONAL_IMPORTS: &str = r#"
use kanban_contract::{
    ApiLabel, ApiTask, CreatedLabelsMeta, OptionalMetadataEnvelope,
    TaskOntologyDetailsMeta,
};
"#;
const VALID_OPTIONAL_HANDLER: &str = r#"
fn optional_handler(
) -> OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>> {
    let meta = None;
    OptionalMetadataEnvelope::new(task, meta)
}
"#;
const VALID_NESTED_OPTIONAL_HANDLER: &str = r#"
fn nested_optional_handler(
) -> OptionalMetadataEnvelope<
    ApiTask,
    TaskOntologyDetailsMeta<Option<TaskOntologySummary>>,
> {
    OptionalMetadataEnvelope::new(task, None)
}
"#;

fn optional_synthetic_source(
    imports: &str,
    optional_handler: &str,
    nested_optional_handler: &str,
    extra: &str,
) -> String {
    format!("{imports}\n{optional_handler}\n{nested_optional_handler}\n{extra}\n")
}

fn valid_optional_synthetic_source() -> String {
    optional_synthetic_source(
        SYNTHETIC_OPTIONAL_IMPORTS,
        VALID_OPTIONAL_HANDLER,
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    )
}

fn optional_handler_fixture(return_type: &str, body: &str) -> String {
    format!("fn optional_handler() -> {return_type} {{\nlet meta = None;\n{body}\n}}")
}

fn nested_optional_handler_fixture(return_type: &str, body: &str) -> String {
    format!("fn nested_optional_handler() -> {return_type} {{\nlet meta = None;\n{body}\n}}")
}

fn synthetic_fixture(
    imports: &str,
    data_handler: &str,
    private_handler: &str,
    required_handler: &str,
    extra: &str,
) -> String {
    format!("{imports}\n{data_handler}\n{private_handler}\n{required_handler}\n{extra}\n")
}

const SYNTHETIC_REQUIRED_RETURN: &str = "MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta>";
const SYNTHETIC_REQUIRED_BODY: &str = r#"
MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
    vec![],
    OffsetPaginationMeta { limit: 10, offset: 0 },
)
"#;

fn required_synthetic_fixture(return_type: &str, body: &str, extra: &str) -> String {
    let required_handler = format!("fn required_handler() -> {return_type} {{\n{body}\n}}");
    synthetic_fixture(
        SYNTHETIC_IMPORTS,
        VALID_DATA_HANDLER,
        VALID_PRIVATE_HANDLER,
        &required_handler,
        extra,
    )
}

fn a3_family_sources(
    data_imports: &str,
    required_imports: &str,
) -> std::collections::BTreeMap<&'static str, String> {
    [
        (
            "data.rs",
            format!(
                r#"
{data_imports}
fn data_handler() -> DataEnvelope<i32> {{ DataEnvelope::new(1) }}
"#
            ),
        ),
        (
            "required.rs",
            format!(
                r#"
{required_imports}
fn required_handler() -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {{
    MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta {{ limit: 10, offset: 0 }},
    )
}}
"#
            ),
        ),
    ]
    .into_iter()
    .collect()
}

fn a3_multi_meta_sources(imports: &str) -> std::collections::BTreeMap<&'static str, String> {
    [(
        "multi.rs",
        format!(
            r#"
{imports}
fn required_handler() -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {{
    MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta {{ limit: 10, offset: 0 }},
    )
}}
fn repeated_meta_handler() -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {{
    MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta {{ limit: 20, offset: 5 }},
    )
}}
fn limit_meta_handler() -> MetadataEnvelope<Vec<Relation>, LimitMeta> {{
    MetadataEnvelope::<Vec<Relation>, LimitMeta>::new(
        vec![],
        LimitMeta {{ limit: 10 }},
    )
}}
"#
        ),
    )]
    .into_iter()
    .collect()
}

fn synthetic_sources_with_optional(
    source: &str,
    optional_source: &str,
) -> std::collections::BTreeMap<&'static str, String> {
    [
        ("fixture.rs", source.to_owned()),
        ("optional.rs", optional_source.to_owned()),
    ]
    .into_iter()
    .collect()
}

fn synthetic_sources(source: &str) -> std::collections::BTreeMap<&'static str, String> {
    synthetic_sources_with_optional(source, &valid_optional_synthetic_source())
}

#[test]
fn g4_frontier_regression_is_stable_before_and_after_typed_migration() {
    static REQUIRED: &[RequiredMetadataSpec] = &[
        RequiredMetadataSpec {
            function: "stable_required",
            data: VEC_RELATION,
            meta: OFFSET_META,
            fields: &["limit", "offset"],
        },
        RequiredMetadataSpec {
            function: "frontier_required",
            data: VEC_RELATION,
            meta: LIMIT_META,
            fields: &["limit"],
        },
    ];
    static OPTIONAL: &[OptionalMetadataSpec] = &[OptionalMetadataSpec {
        function: "frontier_optional",
        data: TASK_DTO,
        meta: CREATED_LABELS_META,
    }];
    const CATALOG: &[HandlerOwnership] = &[HandlerOwnership {
        file: "lifecycle.rs",
        data_only: &["stable_data"],
        private_metadata: &[],
        required_metadata: REQUIRED,
        optional_metadata: OPTIONAL,
    }];
    const FRONTIER_HANDLERS: &[&str] = &["frontier_required", "frontier_optional"];
    const FRONTIER_IMPORTS: &[(&str, ViolationKind, &str)] = &[
        (
            "lifecycle.rs",
            ViolationKind::PrivateImportCount,
            "crate::dto::Envelope",
        ),
        (
            "lifecycle.rs",
            ViolationKind::OptionalMetadataEnvelopeImportCount,
            "kanban_contract::OptionalMetadataEnvelope",
        ),
        (
            "lifecycle.rs",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::CreatedLabelsMeta",
        ),
        (
            "lifecycle.rs",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::LimitMeta",
        ),
    ];

    let private_sources = [(
        "lifecycle.rs",
        r#"
use kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta};
use crate::dto::Envelope;

fn stable_data() -> DataEnvelope<i32> {
    DataEnvelope::new(1)
}

fn stable_required() -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {
    MetadataEnvelope::new(
        vec![],
        OffsetPaginationMeta { limit: 10, offset: 0 },
    )
}

fn frontier_required() -> Envelope<Vec<Relation>> {
    Envelope { data: vec![], meta: Some(1) }
}

fn frontier_optional() -> Envelope<ApiTask> {
    Envelope { data: task, meta: Some(1) }
}
"#
        .to_owned(),
    )]
    .into_iter()
    .collect();
    let private_violations = validate_catalog(&private_sources, CATALOG);
    assert!(
        !private_violations.is_empty(),
        "private frontier must remain visible to the full G4 gate"
    );
    assert_only_frontier_violations(
        &private_violations,
        FRONTIER_HANDLERS,
        FRONTIER_IMPORTS,
        "private migration frontier",
    );

    let typed_sources = [(
        "lifecycle.rs",
        r#"
use kanban_contract::{
    CreatedLabelsMeta, DataEnvelope, LimitMeta, MetadataEnvelope,
    OffsetPaginationMeta, OptionalMetadataEnvelope,
};

fn stable_data() -> DataEnvelope<i32> {
    DataEnvelope::new(1)
}

fn stable_required() -> MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> {
    MetadataEnvelope::new(
        vec![],
        OffsetPaginationMeta { limit: 10, offset: 0 },
    )
}

fn frontier_required() -> MetadataEnvelope<Vec<Relation>, LimitMeta> {
    MetadataEnvelope::new(vec![], LimitMeta { limit: 10 })
}

fn frontier_optional() -> OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>> {
    OptionalMetadataEnvelope::new(task, None)
}
"#
        .to_owned(),
    )]
    .into_iter()
    .collect();
    let typed_violations = validate_catalog(&typed_sources, CATALOG);
    assert_only_frontier_violations(
        &typed_violations,
        FRONTIER_HANDLERS,
        FRONTIER_IMPORTS,
        "typed migration frontier",
    );
    assert!(
        typed_violations.is_empty(),
        "typed G4 lifecycle fixture: {typed_violations:#?}"
    );
}

fn contains_file_violation(
    violations: &[Violation],
    file: &str,
    kind: ViolationKind,
    function: Option<&str>,
) -> bool {
    violations.iter().any(|violation| {
        violation.kind == kind
            && violation.file == file
            && violation.function.as_deref() == function
    })
}

fn contains_violation(
    violations: &[Violation],
    kind: ViolationKind,
    function: Option<&str>,
) -> bool {
    contains_file_violation(violations, "fixture.rs", kind, function)
}

fn assert_import_rejection(
    label: &str,
    sources: &std::collections::BTreeMap<&str, String>,
    catalog: &[HandlerOwnership],
    file: &str,
    kind: ViolationKind,
    path: &str,
) {
    let violations = validate_catalog(sources, catalog);
    assert!(
        violations
            .iter()
            .all(|violation| violation.function.is_none()),
        "{label} produced handler-level noise: {violations:#?}"
    );
    assert!(
        violations.iter().any(|violation| {
            violation.kind == kind
                && violation.file == file
                && violation.function.is_none()
                && violation.detail.contains(path)
        }),
        "{label} did not produce {kind:?} for {file} path={path}: {violations:#?}"
    );
}

fn assert_synthetic_rejection(
    label: &str,
    source: &str,
    kind: ViolationKind,
    function: Option<&str>,
) {
    let violations = validate_catalog(&synthetic_sources(source), SYNTHETIC_CATALOG);
    assert!(
        !violations
            .iter()
            .any(|violation| violation.kind == ViolationKind::MissingFunction),
        "{label} produced unrelated MissingFunction noise: {violations:#?}"
    );
    assert!(
        contains_violation(&violations, kind, function),
        "{label} did not produce {kind:?} for {function:?}: {violations:#?}"
    );
}

fn assert_optional_rejection(
    label: &str,
    optional_source: &str,
    kind: ViolationKind,
    function: Option<&str>,
) {
    let sources = synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, optional_source);
    let violations = validate_catalog(&sources, SYNTHETIC_CATALOG);
    assert!(
        !violations
            .iter()
            .any(|violation| violation.kind == ViolationKind::MissingFunction),
        "{label} produced unrelated MissingFunction noise: {violations:#?}"
    );
    assert!(
        contains_file_violation(&violations, "optional.rs", kind, function),
        "{label} did not produce {kind:?} for {function:?}: {violations:#?}"
    );
}

#[test]
fn g2_validator_self_tests_reject_all_supported_bypasses() {
    let parsed: syn::Type = syn::parse_str("Vec<Relation>").expect("type fixture");
    assert!(type_matches(&parsed, TypeSpec::VecOf(&RELATION)));
    let baseline = validate_catalog(
        &synthetic_sources(VALID_SYNTHETIC_SOURCE),
        SYNTHETIC_CATALOG,
    );
    assert!(baseline.is_empty(), "valid synthetic source: {baseline:#?}");

    let cases = vec![
        (
            "data function remains private",
            ViolationKind::WrongReturnEnvelope,
            Some("data_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                "fn data_handler() -> Envelope<i32> { Envelope { data: 1, meta: None } }",
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "",
            ),
        ),
        (
            "metadata function changes to data envelope",
            ViolationKind::WrongReturnEnvelope,
            Some("meta_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                VALID_DATA_HANDLER,
                "fn meta_handler() -> DataEnvelope<i32> { DataEnvelope::new(1) }",
                VALID_REQUIRED_HANDLER,
                "",
            ),
        ),
        (
            "unregistered response",
            ViolationKind::UnregisteredEnvelopeResponse,
            Some("extra_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                VALID_DATA_HANDLER,
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "fn extra_handler() -> Envelope<i32> { Envelope { data: 1, meta: None } }",
            ),
        ),
        (
            "data struct literal",
            ViolationKind::DataStructLiteral,
            Some("data_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                "fn data_handler() -> DataEnvelope<i32> { DataEnvelope { data: 1 } }",
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "",
            ),
        ),
        (
            "data import alias",
            ViolationKind::DataImportAlias,
            None,
            synthetic_fixture(
                r#"
use {
    kanban_contract::{DataEnvelope as D, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
                "fn data_handler() -> D<i32> { D::new(1) }",
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "",
            ),
        ),
        (
            "unregistered DataEnvelope impl IntoResponse body",
            ViolationKind::UnregisteredEnvelopeResponse,
            Some("extra_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                VALID_DATA_HANDLER,
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "fn extra_handler() -> impl IntoResponse { DataEnvelope::new(1) }",
            ),
        ),
        (
            "private import alias",
            ViolationKind::PrivateImportAlias,
            None,
            synthetic_fixture(
                r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope as E,
};
"#,
                VALID_DATA_HANDLER,
                "fn meta_handler() -> E<i32> { E { data: 1, meta: Some(2) } }",
                VALID_REQUIRED_HANDLER,
                "",
            ),
        ),
        (
            "unregistered private Envelope impl IntoResponse body",
            ViolationKind::UnregisteredEnvelopeResponse,
            Some("extra_handler"),
            synthetic_fixture(
                SYNTHETIC_IMPORTS,
                VALID_DATA_HANDLER,
                VALID_PRIVATE_HANDLER,
                VALID_REQUIRED_HANDLER,
                "fn extra_handler() -> impl IntoResponse { Envelope { data: 1, meta: None } }",
            ),
        ),
    ];

    for (label, kind, function, source) in cases {
        assert_synthetic_rejection(label, &source, kind, function);
    }
}

#[test]
fn g3_family_aware_recursive_import_closure_is_exact() {
    let baseline = validate_catalog(
        &synthetic_sources(VALID_SYNTHETIC_SOURCE),
        SYNTHETIC_CATALOG,
    );
    assert!(
        baseline.is_empty(),
        "grouped recursive import baseline: {baseline:#?}"
    );

    let cases = [
        (
            "DataEnvelope alias",
            ViolationKind::DataImportAlias,
            "kanban_contract::DataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope as D, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "private Envelope alias",
            ViolationKind::PrivateImportAlias,
            "crate::dto::Envelope",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope as E,
};
"#,
        ),
        (
            "MetadataEnvelope alias",
            ViolationKind::MetadataEnvelopeImportAlias,
            "kanban_contract::MetadataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope as M, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "required meta alias",
            ViolationKind::RequiredMetaImportAlias,
            "kanban_contract::OffsetPaginationMeta",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta as PageMeta},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "duplicate DataEnvelope direct import",
            ViolationKind::DataImportCount,
            "kanban_contract::DataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
use kanban_contract::DataEnvelope;
"#,
        ),
        (
            "duplicate private Envelope direct import",
            ViolationKind::PrivateImportCount,
            "crate::dto::Envelope",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
use crate::dto::Envelope;
"#,
        ),
        (
            "duplicate MetadataEnvelope direct import",
            ViolationKind::MetadataEnvelopeImportCount,
            "kanban_contract::MetadataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
use kanban_contract::MetadataEnvelope;
"#,
        ),
        (
            "duplicate required meta direct import",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::OffsetPaginationMeta",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
use kanban_contract::OffsetPaginationMeta;
"#,
        ),
        (
            "contract namespace glob",
            ViolationKind::OwnerNamespaceGlob,
            "kanban_contract",
            r#"
use kanban_contract::*;
use crate::dto::Envelope;
"#,
        ),
        (
            "private namespace glob",
            ViolationKind::OwnerNamespaceGlob,
            "crate::dto",
            r#"
use kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta};
use crate::dto::*;
"#,
        ),
        (
            "missing MetadataEnvelope direct import",
            ViolationKind::MetadataEnvelopeImportCount,
            "kanban_contract::MetadataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "missing required meta direct import",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::OffsetPaginationMeta",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "wrong deep MetadataEnvelope path",
            ViolationKind::MetadataEnvelopeImportCount,
            "kanban_contract::MetadataEnvelope",
            r#"
use {
    kanban_contract::{DataEnvelope, nested::MetadataEnvelope, OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
        ),
        (
            "wrong deep required meta path",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::OffsetPaginationMeta",
            r#"
use {
    kanban_contract::{DataEnvelope, MetadataEnvelope, nested::OffsetPaginationMeta},
    crate::dto::Envelope,
};
"#,
        ),
    ];
    for (label, kind, path, imports) in cases {
        let source = synthetic_fixture(
            imports,
            VALID_DATA_HANDLER,
            VALID_PRIVATE_HANDLER,
            VALID_REQUIRED_HANDLER,
            "",
        );
        assert_import_rejection(
            label,
            &synthetic_sources(&source),
            SYNTHETIC_CATALOG,
            "fixture.rs",
            kind,
            path,
        );
    }

    const DATA_IMPORTS: &str = "use kanban_contract::DataEnvelope;";
    const REQUIRED_IMPORTS: &str = "use kanban_contract::{MetadataEnvelope, OffsetPaginationMeta};";
    let family_baseline = a3_family_sources(DATA_IMPORTS, REQUIRED_IMPORTS);
    let family_violations = validate_catalog(&family_baseline, A3_FAMILY_CATALOG);
    assert!(
        family_violations.is_empty(),
        "family-absent baseline: {family_violations:#?}"
    );

    let dead_private = a3_family_sources(
        r#"
use kanban_contract::DataEnvelope;
use crate::dto::Envelope;
"#,
        REQUIRED_IMPORTS,
    );
    assert_import_rejection(
        "dead private import without private family",
        &dead_private,
        A3_FAMILY_CATALOG,
        "data.rs",
        ViolationKind::PrivateImportCount,
        "crate::dto::Envelope",
    );

    let dead_metadata = a3_family_sources(
        "use kanban_contract::{DataEnvelope, MetadataEnvelope};",
        REQUIRED_IMPORTS,
    );
    assert_import_rejection(
        "dead MetadataEnvelope import without Required family",
        &dead_metadata,
        A3_FAMILY_CATALOG,
        "data.rs",
        ViolationKind::MetadataEnvelopeImportCount,
        "kanban_contract::MetadataEnvelope",
    );

    let dead_meta_type = a3_family_sources(
        "use kanban_contract::{DataEnvelope, OffsetPaginationMeta};",
        REQUIRED_IMPORTS,
    );
    assert_import_rejection(
        "dead meta type import without Required family",
        &dead_meta_type,
        A3_FAMILY_CATALOG,
        "data.rs",
        ViolationKind::RequiredMetaImportCount,
        "kanban_contract::OffsetPaginationMeta",
    );

    let dead_data = a3_family_sources(
        DATA_IMPORTS,
        "use kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta};",
    );
    assert_import_rejection(
        "dead DataEnvelope import without Data family",
        &dead_data,
        A3_FAMILY_CATALOG,
        "required.rs",
        ViolationKind::DataImportCount,
        "kanban_contract::DataEnvelope",
    );

    const MULTI_META_IMPORTS: &str =
        "use kanban_contract::{MetadataEnvelope, LimitMeta, OffsetPaginationMeta};";
    let multi_meta_baseline = a3_multi_meta_sources(MULTI_META_IMPORTS);
    let multi_meta_violations = validate_catalog(&multi_meta_baseline, A3_MULTI_META_CATALOG);
    assert!(
        multi_meta_violations.is_empty(),
        "distinct and repeated meta imports: {multi_meta_violations:#?}"
    );

    let missing_distinct_meta =
        a3_multi_meta_sources("use kanban_contract::{MetadataEnvelope, OffsetPaginationMeta};");
    assert_import_rejection(
        "each distinct meta type is required once",
        &missing_distinct_meta,
        A3_MULTI_META_CATALOG,
        "multi.rs",
        ViolationKind::RequiredMetaImportCount,
        "kanban_contract::LimitMeta",
    );
}

#[test]
fn g3_unified_synthetic_validator_rejects_required_metadata_bypasses() {
    let baseline = validate_catalog(
        &synthetic_sources(VALID_SYNTHETIC_SOURCE),
        SYNTHETIC_CATALOG,
    );
    assert!(
        baseline.is_empty(),
        "three-kind synthetic baseline: {baseline:#?}"
    );

    let unrelated_middle_segments = required_synthetic_fixture(
        SYNTHETIC_REQUIRED_RETURN,
        r#"
foo::Envelope::helper();
foo::MetadataEnvelope::helper();
kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
    vec![],
    OffsetPaginationMeta { limit: 10, offset: 0 },
)
"#,
        "",
    );
    let unrelated_violations = validate_catalog(
        &synthetic_sources(&unrelated_middle_segments),
        SYNTHETIC_CATALOG,
    );
    assert!(
        unrelated_violations.is_empty(),
        "unrelated middle path segments: {unrelated_violations:#?}"
    );

    let cases = vec![
        (
            "wrong return data type",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope<Vec<ApiTask>, OffsetPaginationMeta>",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "wrong return metadata type",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope<Vec<Relation>, LimitMeta>",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "missing return generic",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope<Vec<Relation>>",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "extra return generic",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta, Extra>",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "non-type return generic",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope<Vec<Relation>, 'static>",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "missing return generic list",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "kanban_contract::MetadataEnvelope",
                SYNTHETIC_REQUIRED_BODY,
                "",
            ),
        ),
        (
            "wrong body metadata type",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                r#"
kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
    vec![],
    OtherMeta { limit: 10, offset: 0 },
)
"#,
                "",
            ),
        ),
        (
            "zero constructor arity",
            ViolationKind::WrongRequiredMetadataConstructor,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new()",
                "",
            ),
        ),
        (
            "one constructor arity",
            ViolationKind::WrongRequiredMetadataConstructor,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![])",
                "",
            ),
        ),
        (
            "three constructor arguments",
            ViolationKind::WrongRequiredMetadataConstructor,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], OffsetPaginationMeta { limit: 10, offset: 0 }, extra)",
                "",
            ),
        ),
        (
            "multiple constructors",
            ViolationKind::WrongRequiredMetadataConstructor,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                r#"
kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
    vec![],
    OffsetPaginationMeta { limit: 10, offset: 0 },
);
kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
    vec![],
    OffsetPaginationMeta { limit: 10, offset: 0 },
)
"#,
                "",
            ),
        ),
        (
            "parenthesized metadata argument",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], (OffsetPaginationMeta { limit: 10, offset: 0 }))",
                "",
            ),
        ),
        (
            "block metadata argument",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], { OffsetPaginationMeta { limit: 10, offset: 0 } })",
                "",
            ),
        ),
        (
            "macro metadata argument",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], meta!())",
                "",
            ),
        ),
        (
            "call metadata argument",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], make_meta())",
                "",
            ),
        ),
        (
            "path metadata argument",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], meta)",
                "",
            ),
        ),
        (
            "missing metadata field",
            ViolationKind::WrongRequiredMetadataFields,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], OffsetPaginationMeta { limit: 10 })",
                "",
            ),
        ),
        (
            "extra metadata field",
            ViolationKind::WrongRequiredMetadataFields,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], OffsetPaginationMeta { limit: 10, offset: 0, extra: 1 })",
                "",
            ),
        ),
        (
            "unnamed metadata field",
            ViolationKind::WrongRequiredMetadataFields,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], OffsetPaginationMeta { 0: 10, limit: 10, offset: 0 })",
                "",
            ),
        ),
        (
            "metadata rest",
            ViolationKind::WrongRequiredMetadataFields,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(vec![], OffsetPaginationMeta { limit: 10, offset: 0, ..base })",
                "",
            ),
        ),
        (
            "required handler changed to DataEnvelope",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "DataEnvelope<Vec<Relation>>",
                "DataEnvelope::new(vec![])",
                "",
            ),
        ),
        (
            "required handler changed to private Envelope",
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
            required_synthetic_fixture(
                "Envelope<Vec<Relation>>",
                "Envelope { data: vec![], meta: None }",
                "",
            ),
        ),
        (
            "MetadataEnvelope literal",
            ViolationKind::ForbiddenRequiredMetadataLiteral,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                "kanban_contract::MetadataEnvelope { data: vec![], meta: OffsetPaginationMeta { limit: 10, offset: 0 } }",
                "",
            ),
        ),
        (
            "mixed DataEnvelope family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                &format!("DataEnvelope::new(1);\n{SYNTHETIC_REQUIRED_BODY}"),
                "",
            ),
        ),
        (
            "mixed private Envelope family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                &format!("Envelope {{ data: 1, meta: None }};\n{SYNTHETIC_REQUIRED_BODY}"),
                "",
            ),
        ),
        (
            "mixed OptionalMetadataEnvelope family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                &format!(
                    "kanban_contract::OptionalMetadataEnvelope::new(vec![], None);\n{SYNTHETIC_REQUIRED_BODY}"
                ),
                "",
            ),
        ),
        (
            "extra MetadataEnvelope path",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            Some("required_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                &format!(
                    "let _: kanban_contract::MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta>;\n{SYNTHETIC_REQUIRED_BODY}"
                ),
                "",
            ),
        ),
        (
            "unregistered MetadataEnvelope impl IntoResponse body",
            ViolationKind::UnregisteredEnvelopeResponse,
            Some("extra_handler"),
            required_synthetic_fixture(
                SYNTHETIC_REQUIRED_RETURN,
                SYNTHETIC_REQUIRED_BODY,
                r#"
fn extra_handler() -> impl IntoResponse {
    kanban_contract::MetadataEnvelope::<Vec<Relation>, OffsetPaginationMeta>::new(
        vec![],
        OffsetPaginationMeta { limit: 10, offset: 0 },
    )
}
"#,
            ),
        ),
    ];

    for (label, kind, function, source) in cases {
        assert_synthetic_rejection(label, &source, kind, function);
    }
}

#[test]
fn g4_optional_metadata_synthetic_matrix_is_closed() {
    const OPTIONAL_RETURN: &str = "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>";
    const OPTIONAL_BODY: &str = "OptionalMetadataEnvelope::new(task, meta)";
    const NESTED_OPTIONAL_RETURN: &str =
        "OptionalMetadataEnvelope<ApiTask, TaskOntologyDetailsMeta<Option<TaskOntologySummary>>>";
    const NESTED_OPTIONAL_BODY: &str = "OptionalMetadataEnvelope::new(task, meta)";

    let baseline = validate_catalog(
        &synthetic_sources(VALID_SYNTHETIC_SOURCE),
        SYNTHETIC_CATALOG,
    );
    assert!(
        baseline.is_empty(),
        "data/required/optional synthetic baseline: {baseline:#?}"
    );

    let nested: syn::Type = syn::parse_str("TaskOntologyDetailsMeta<Option<TaskOntologySummary>>")
        .expect("nested generic fixture");
    assert!(type_matches(&nested, TASK_ONTOLOGY_DETAILS_META));
    for wrong in [
        "TaskOntologyDetailsMeta<TaskOntologySummary>",
        "TaskOntologyDetailsMeta<Option<Option<TaskOntologySummary>>>",
        "TaskOntologyDetailsMeta<Option<TaskOntologySummary>, Extra>",
    ] {
        let parsed = syn::parse_str(wrong).expect("wrong nested generic fixture");
        assert!(
            !type_matches(&parsed, TASK_ONTOLOGY_DETAILS_META),
            "nested generic mismatch accepted: {wrong}"
        );
    }
    let vec_mismatch: syn::Type =
        syn::parse_str("Vec<Vec<Relation>>").expect("Vec mismatch fixture");
    assert!(!type_matches(&vec_mismatch, VEC_RELATION));

    let required_to_optional = required_synthetic_fixture(
        "kanban_contract::OptionalMetadataEnvelope<Vec<Relation>, OffsetPaginationMeta>",
        r#"
kanban_contract::OptionalMetadataEnvelope::new(
    vec![],
    Some(OffsetPaginationMeta { limit: 10, offset: 0 }),
)
"#,
        "",
    );
    assert_synthetic_rejection(
        "required handler changed to optional family",
        &required_to_optional,
        ViolationKind::WrongReturnEnvelope,
        Some("required_handler"),
    );
    assert_synthetic_rejection(
        "required body changed to optional family",
        &required_to_optional,
        ViolationKind::ForbiddenRequiredMetadataFamily,
        Some("required_handler"),
    );

    let cases = vec![
        (
            "optional return missing metadata argument",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture("OptionalMetadataEnvelope<ApiTask>", OPTIONAL_BODY),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional return has extra argument",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>, Extra>",
                    OPTIONAL_BODY,
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "CreatedLabelsMeta generic argument mismatch",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<Wrong>>",
                    OPTIONAL_BODY,
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "TaskOntologyDetailsMeta missing Option",
            ViolationKind::WrongReturnEnvelope,
            "nested_optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                VALID_OPTIONAL_HANDLER,
                &nested_optional_handler_fixture(
                    "OptionalMetadataEnvelope<ApiTask, TaskOntologyDetailsMeta<TaskOntologySummary>>",
                    NESTED_OPTIONAL_BODY,
                ),
                "",
            ),
        ),
        (
            "TaskOntologyDetailsMeta double Option",
            ViolationKind::WrongReturnEnvelope,
            "nested_optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                VALID_OPTIONAL_HANDLER,
                &nested_optional_handler_fixture(
                    "OptionalMetadataEnvelope<ApiTask, TaskOntologyDetailsMeta<Option<Option<TaskOntologySummary>>>>",
                    NESTED_OPTIONAL_BODY,
                ),
                "",
            ),
        ),
        (
            "optional constructor one argument",
            ViolationKind::WrongOptionalMetadataConstructor,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(OPTIONAL_RETURN, "OptionalMetadataEnvelope::new(task)"),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional constructor three arguments",
            ViolationKind::WrongOptionalMetadataConstructor,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "OptionalMetadataEnvelope::new(task, meta, extra)",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "multiple optional constructors",
            ViolationKind::WrongOptionalMetadataConstructor,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "OptionalMetadataEnvelope::new(task, meta); OptionalMetadataEnvelope::new(task, None)",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional struct literal",
            ViolationKind::ForbiddenOptionalMetadataLiteral,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "OptionalMetadataEnvelope { data: task, meta }",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional body changed to private family",
            ViolationKind::ForbiddenOptionalMetadataFamily,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "crate::dto::Envelope { data: task, meta }",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional body changed to data family",
            ViolationKind::ForbiddenOptionalMetadataFamily,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "kanban_contract::DataEnvelope::new(task)",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional body changed to required family",
            ViolationKind::ForbiddenOptionalMetadataFamily,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    OPTIONAL_RETURN,
                    "kanban_contract::MetadataEnvelope::new(
                        task,
                        CreatedLabelsMeta { created_labels: vec![] },
                    )",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional handler changed to required family",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    "kanban_contract::MetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                    "kanban_contract::MetadataEnvelope::new(task, CreatedLabelsMeta { created_labels: vec![] })",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional handler changed to private family",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    "crate::dto::Envelope<ApiTask>",
                    "crate::dto::Envelope { data: task, meta }",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
        (
            "optional handler changed to data family",
            ViolationKind::WrongReturnEnvelope,
            "optional_handler",
            optional_synthetic_source(
                SYNTHETIC_OPTIONAL_IMPORTS,
                &optional_handler_fixture(
                    "kanban_contract::DataEnvelope<ApiTask>",
                    "kanban_contract::DataEnvelope::new(task)",
                ),
                VALID_NESTED_OPTIONAL_HANDLER,
                "",
            ),
        ),
    ];
    for (label, kind, function, source) in cases {
        assert_optional_rejection(label, &source, kind, Some(function));
    }

    let body_only = optional_synthetic_source(
        SYNTHETIC_OPTIONAL_IMPORTS,
        VALID_OPTIONAL_HANDLER,
        VALID_NESTED_OPTIONAL_HANDLER,
        r#"
fn extra_handler() -> impl IntoResponse {
    OptionalMetadataEnvelope::new(task, None::<CreatedLabelsMeta<ApiLabel>>)
}
"#,
    );
    assert_optional_rejection(
        "body-only unregistered optional response",
        &body_only,
        ViolationKind::UnregisteredEnvelopeResponse,
        Some("extra_handler"),
    );

    let import_cases = [
        (
            "OptionalMetadataEnvelope alias",
            ViolationKind::OptionalMetadataEnvelopeImportAlias,
            "kanban_contract::OptionalMetadataEnvelope",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                OptionalMetadataEnvelope as OptionalEnvelope,
                TaskOntologyDetailsMeta,
            };"#,
        ),
        (
            "CreatedLabelsMeta alias",
            ViolationKind::RequiredMetaImportAlias,
            "kanban_contract::CreatedLabelsMeta",
            r#"use kanban_contract::{
                CreatedLabelsMeta as CreatedMeta,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta,
            };"#,
        ),
        (
            "TaskOntologyDetailsMeta alias",
            ViolationKind::RequiredMetaImportAlias,
            "kanban_contract::TaskOntologyDetailsMeta",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta as DetailsMeta,
            };"#,
        ),
        (
            "optional owner namespace glob",
            ViolationKind::OwnerNamespaceGlob,
            "kanban_contract",
            "use kanban_contract::*;",
        ),
        (
            "duplicate OptionalMetadataEnvelope import",
            ViolationKind::OptionalMetadataEnvelopeImportCount,
            "kanban_contract::OptionalMetadataEnvelope",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta,
            };
            use kanban_contract::OptionalMetadataEnvelope;"#,
        ),
        (
            "duplicate CreatedLabelsMeta import",
            ViolationKind::RequiredMetaImportCount,
            "kanban_contract::CreatedLabelsMeta",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta,
            };
            use kanban_contract::CreatedLabelsMeta;"#,
        ),
        (
            "dead required family import in optional-only file",
            ViolationKind::MetadataEnvelopeImportCount,
            "kanban_contract::MetadataEnvelope",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                MetadataEnvelope,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta,
            };"#,
        ),
        (
            "dead data family import in optional-only file",
            ViolationKind::DataImportCount,
            "kanban_contract::DataEnvelope",
            r#"use kanban_contract::{
                CreatedLabelsMeta,
                DataEnvelope,
                OptionalMetadataEnvelope,
                TaskOntologyDetailsMeta,
            };"#,
        ),
    ];
    for (label, kind, path, imports) in import_cases {
        let optional_source = optional_synthetic_source(
            imports,
            VALID_OPTIONAL_HANDLER,
            VALID_NESTED_OPTIONAL_HANDLER,
            "",
        );
        let sources = synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &optional_source);
        assert_import_rejection(
            label,
            &sources,
            SYNTHETIC_CATALOG,
            "optional.rs",
            kind,
            path,
        );
    }

    let nested_return: syn::Type =
        syn::parse_str(NESTED_OPTIONAL_RETURN).expect("nested optional return fixture");
    let mut shape = ReturnShape::default();
    shape.visit_type(&nested_return);
    assert!(shape.is_exact_optional(SYNTHETIC_OPTIONAL[1]));
}

#[test]
fn g4_canonical_owners_reject_foreign_qself_and_shadow_bypasses() {
    let qualified_type: syn::Type =
        syn::parse_str("::kanban_contract::CreatedLabelsMeta<::kanban_contract::ApiLabel>")
            .expect("qualified canonical type");
    assert!(type_matches(&qualified_type, CREATED_LABELS_META));
    for wrong in [
        "evil::CreatedLabelsMeta<ApiLabel>",
        "CreatedLabelsMeta<'static>",
        "CreatedLabelsMeta<3>",
        "CreatedLabelsMeta<<evil::Holder as Trait>::ApiLabel>",
    ] {
        let parsed = syn::parse_str(wrong).expect("foreign/generic type mutation");
        assert!(
            !type_matches(&parsed, CREATED_LABELS_META),
            "owner/generic bypass accepted: {wrong}"
        );
    }
    let signal_meta: syn::Type =
        syn::parse_str("kanban_contract::SignalFilterMeta").expect("canonical signal meta");
    let evil_signal_meta: syn::Type =
        syn::parse_str("evil::SignalFilterMeta").expect("foreign signal meta");
    assert!(type_matches(&signal_meta, SIGNAL_FILTER_META));
    assert!(!type_matches(&evil_signal_meta, SIGNAL_FILTER_META));

    let qualified_optional = optional_synthetic_source(
        SYNTHETIC_OPTIONAL_IMPORTS,
        r#"
fn optional_handler(
) -> kanban_contract::OptionalMetadataEnvelope<
    kanban_contract::ApiTask,
    kanban_contract::CreatedLabelsMeta<kanban_contract::ApiLabel>,
> {
    let meta = None;
    kanban_contract::OptionalMetadataEnvelope::new(task, meta)
}
"#,
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let qualified_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &qualified_optional),
        SYNTHETIC_CATALOG,
    );
    assert!(
        qualified_violations.is_empty(),
        "canonical fully-qualified paths: {qualified_violations:#?}"
    );

    let evil_envelope = optional_synthetic_source(
        SYNTHETIC_OPTIONAL_IMPORTS,
        &optional_handler_fixture(
            "evil::OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
            "evil::OptionalMetadataEnvelope::new(task, meta)",
        ),
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let evil_envelope_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &evil_envelope),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &evil_envelope_violations,
            "optional.rs",
            ViolationKind::MissingEnvelopeResponse,
            Some("optional_handler"),
        ),
        "foreign optional envelope: {evil_envelope_violations:#?}"
    );
    assert!(
        !evil_envelope_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "canonical sibling was polluted: {evil_envelope_violations:#?}"
    );

    let root_alias_imports = r#"
use evil as kanban_contract;
use kanban_contract::{
    CreatedLabelsMeta, OptionalMetadataEnvelope, TaskOntologyDetailsMeta,
};
"#;
    let root_alias_target = optional_synthetic_source(
        root_alias_imports,
        r#"
fn optional_handler(
) -> OptionalMetadataEnvelope<
    crate::dto::ApiTask,
    CreatedLabelsMeta<crate::dto::ApiLabel>,
> {
    OptionalMetadataEnvelope::new(task, None)
}
"#,
        r#"
fn nested_optional_handler(
) -> ::kanban_contract::OptionalMetadataEnvelope<
    ::kanban_contract::ApiTask,
    ::kanban_contract::TaskOntologyDetailsMeta<
        core::option::Option<kanban_sqlite::api::TaskOntologySummary>,
    >,
> {
    ::kanban_contract::OptionalMetadataEnvelope::new(task, None)
}
"#,
        "",
    );
    let root_alias_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &root_alias_target),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &root_alias_violations,
            "optional.rs",
            ViolationKind::MissingEnvelopeResponse,
            Some("optional_handler"),
        ),
        "owner root alias bypass: {root_alias_violations:#?}"
    );
    assert!(
        !root_alias_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "absolute canonical sibling was polluted: {root_alias_violations:#?}"
    );

    let foreign_label_imports = format!("{SYNTHETIC_OPTIONAL_IMPORTS}\nuse evil::ApiLabel;");
    let foreign_label = optional_synthetic_source(
        &foreign_label_imports,
        VALID_OPTIONAL_HANDLER,
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let foreign_label_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &foreign_label),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &foreign_label_violations,
            "optional.rs",
            ViolationKind::WrongReturnEnvelope,
            Some("optional_handler"),
        ),
        "foreign nested data binding: {foreign_label_violations:#?}"
    );
    assert!(
        !foreign_label_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "canonical nested sibling was polluted: {foreign_label_violations:#?}"
    );
    let optional_cases = [
        (
            "foreign CreatedLabelsMeta owner",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, evil::CreatedLabelsMeta<ApiLabel>>",
                "OptionalMetadataEnvelope::new(task, meta)",
            ),
            ViolationKind::WrongReturnEnvelope,
        ),
        (
            "lifetime generic argument",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<'static>>",
                "OptionalMetadataEnvelope::new(task, meta)",
            ),
            ViolationKind::WrongReturnEnvelope,
        ),
        (
            "const generic argument",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<3>>",
                "OptionalMetadataEnvelope::new(task, meta)",
            ),
            ViolationKind::WrongReturnEnvelope,
        ),
        (
            "nested qself generic argument",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<<evil::Holder as Trait>::ApiLabel>>",
                "OptionalMetadataEnvelope::new(task, meta)",
            ),
            ViolationKind::WrongReturnEnvelope,
        ),
        (
            "qself envelope return",
            optional_handler_fixture(
                "<evil::Holder as Trait>::OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                "OptionalMetadataEnvelope::new(task, meta)",
            ),
            ViolationKind::WrongReturnEnvelope,
        ),
        (
            "qself envelope constructor",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                "<evil::OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>> as Trait>::new(task, meta)",
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "block-local envelope module shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    mod OptionalMetadataEnvelope {}
                    OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "block-local owner module shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    mod kanban_contract {}
                    kanban_contract::OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "block-local extern crate original shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    extern crate OptionalMetadataEnvelope;
                    OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "block-local extern crate alias shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    extern crate evil as kanban_contract;
                    kanban_contract::OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "local envelope alias shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    use evil::ForeignEnvelope as OptionalMetadataEnvelope;
                    OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
        (
            "local canonical owner root shadow",
            optional_handler_fixture(
                "OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>",
                r#"{
                    use evil as kanban_contract;
                    kanban_contract::OptionalMetadataEnvelope::new(task, meta)
                }"#,
            ),
            ViolationKind::WrongOptionalMetadataConstructor,
        ),
    ];
    for (label, handler, kind) in optional_cases {
        let source = optional_synthetic_source(
            SYNTHETIC_OPTIONAL_IMPORTS,
            &handler,
            VALID_NESTED_OPTIONAL_HANDLER,
            "",
        );
        assert_optional_rejection(label, &source, kind, Some("optional_handler"));
        let violations = validate_catalog(
            &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &source),
            SYNTHETIC_CATALOG,
        );
        assert!(
            !violations
                .iter()
                .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
            "{label} polluted canonical sibling: {violations:#?}"
        );
    }

    let required_cases = [
        (
            "foreign MetadataEnvelope constructor",
            "evil::MetadataEnvelope::new(
                vec![],
                OffsetPaginationMeta { limit: 10, offset: 0 },
            )",
            ViolationKind::WrongRequiredMetadataConstructor,
        ),
        (
            "foreign metadata literal owner",
            "MetadataEnvelope::new(
                vec![],
                evil::OffsetPaginationMeta { limit: 10, offset: 0 },
            )",
            ViolationKind::WrongRequiredMetadataBodyType,
        ),
        (
            "qself MetadataEnvelope constructor",
            "<evil::MetadataEnvelope<Vec<Relation>, OffsetPaginationMeta> as Trait>::new(
                vec![],
                OffsetPaginationMeta { limit: 10, offset: 0 },
            )",
            ViolationKind::WrongRequiredMetadataConstructor,
        ),
        (
            "qself metadata literal",
            "MetadataEnvelope::new(
                vec![],
                <evil::Holder as Trait>::OffsetPaginationMeta { limit: 10, offset: 0 },
            )",
            ViolationKind::WrongRequiredMetadataBodyType,
        ),
        (
            "local metadata owner shadow",
            r#"{
                use evil::OffsetPaginationMeta;
                MetadataEnvelope::new(
                    vec![],
                    OffsetPaginationMeta { limit: 10, offset: 0 },
                )
            }"#,
            ViolationKind::WrongRequiredMetadataBodyType,
        ),
        (
            "local required envelope alias shadow",
            r#"{
                use evil::ForeignEnvelope as MetadataEnvelope;
                MetadataEnvelope::new(
                    vec![],
                    OffsetPaginationMeta { limit: 10, offset: 0 },
                )
            }"#,
            ViolationKind::WrongRequiredMetadataConstructor,
        ),
    ];
    for (label, body, kind) in required_cases {
        assert_synthetic_rejection(
            label,
            &required_synthetic_fixture(SYNTHETIC_REQUIRED_RETURN, body, ""),
            kind,
            Some("required_handler"),
        );
    }

    static SIGNAL_REQUIRED: &[RequiredMetadataSpec] = &[RequiredMetadataSpec {
        function: "signal_handler",
        data: VEC_SIGNAL_RECORD,
        meta: SIGNAL_FILTER_META,
        fields: &["include_all", "limit"],
    }];
    const SIGNAL_CATALOG: &[HandlerOwnership] = &[HandlerOwnership {
        file: "signal.rs",
        data_only: &[],
        private_metadata: &[],
        required_metadata: SIGNAL_REQUIRED,
        optional_metadata: &[],
    }];
    let signal_source = |return_meta: &str, body_meta: &str| {
        [(
            "signal.rs",
            format!(
                r#"
use kanban_contract::{{MetadataEnvelope, SignalFilterMeta}};
fn signal_handler(
) -> MetadataEnvelope<Vec<kanban_sqlite::api::SignalRecord>, {return_meta}> {{
    MetadataEnvelope::new(
        vec![],
        {body_meta} {{ include_all: false, limit: 10 }},
    )
}}
"#
            ),
        )]
        .into_iter()
        .collect()
    };
    let signal_baseline = validate_catalog(
        &signal_source("SignalFilterMeta", "SignalFilterMeta"),
        SIGNAL_CATALOG,
    );
    assert!(
        signal_baseline.is_empty(),
        "signal owner baseline: {signal_baseline:#?}"
    );
    let foreign_return = validate_catalog(
        &signal_source("evil::SignalFilterMeta", "SignalFilterMeta"),
        SIGNAL_CATALOG,
    );
    assert!(
        contains_file_violation(
            &foreign_return,
            "signal.rs",
            ViolationKind::WrongReturnEnvelope,
            Some("signal_handler"),
        ),
        "foreign SignalFilterMeta return: {foreign_return:#?}"
    );
    let foreign_body = validate_catalog(
        &signal_source("SignalFilterMeta", "evil::SignalFilterMeta"),
        SIGNAL_CATALOG,
    );
    assert!(
        contains_file_violation(
            &foreign_body,
            "signal.rs",
            ViolationKind::WrongRequiredMetadataBodyType,
            Some("signal_handler"),
        ),
        "foreign SignalFilterMeta body: {foreign_body:#?}"
    );
}

#[test]
fn g4_module_items_and_foreign_globs_are_owner_aware() {
    const ABSOLUTE_NESTED_OPTIONAL_HANDLER: &str = r#"
fn nested_optional_handler(
) -> ::kanban_contract::OptionalMetadataEnvelope<
    ::kanban_contract::ApiTask,
    ::kanban_contract::TaskOntologyDetailsMeta<
        ::core::option::Option<::kanban_sqlite::api::TaskOntologySummary>,
    >,
> {
    ::kanban_contract::OptionalMetadataEnvelope::new(task, None)
}
"#;

    for (label, owner_item) in [
        ("module owner shadow", "mod kanban_contract {}"),
        (
            "extern crate owner alias",
            "extern crate evil as kanban_contract;",
        ),
    ] {
        let imports = format!("{owner_item}\n{SYNTHETIC_OPTIONAL_IMPORTS}");
        let source = optional_synthetic_source(
            &imports,
            VALID_OPTIONAL_HANDLER,
            ABSOLUTE_NESTED_OPTIONAL_HANDLER,
            "",
        );
        let violations = validate_catalog(
            &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &source),
            SYNTHETIC_CATALOG,
        );
        assert!(
            contains_file_violation(
                &violations,
                "optional.rs",
                ViolationKind::MissingEnvelopeResponse,
                Some("optional_handler"),
            ),
            "{label}: {violations:#?}"
        );
        assert!(
            !violations
                .iter()
                .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
            "{label} polluted absolute canonical sibling: {violations:#?}"
        );
    }

    let relation_glob_source = synthetic_fixture(
        r#"
use ::kanban_contract::{DataEnvelope, MetadataEnvelope, OffsetPaginationMeta};
use ::std::vec::Vec;
use crate::dto::Envelope;
use evil as kanban_entity;
use kanban_entity::*;
"#,
        VALID_DATA_HANDLER,
        VALID_PRIVATE_HANDLER,
        VALID_REQUIRED_HANDLER,
        "",
    );
    let relation_glob_violations =
        validate_catalog(&synthetic_sources(&relation_glob_source), SYNTHETIC_CATALOG);
    assert!(
        contains_violation(
            &relation_glob_violations,
            ViolationKind::WrongReturnEnvelope,
            Some("required_handler"),
        ),
        "shadowed kanban_entity glob must not prove Relation: {relation_glob_violations:#?}"
    );
    assert!(
        !relation_glob_violations.iter().any(|violation| {
            matches!(
                violation.function.as_deref(),
                Some("data_handler" | "meta_handler")
            )
        }),
        "shadowed kanban_entity glob polluted stable handlers: {relation_glob_violations:#?}"
    );

    static GLOB_SIGNAL_REQUIRED: &[RequiredMetadataSpec] = &[RequiredMetadataSpec {
        function: "signal_handler",
        data: VEC_SIGNAL_RECORD,
        meta: SIGNAL_FILTER_META,
        fields: &["include_all", "limit"],
    }];
    const GLOB_SIGNAL_CATALOG: &[HandlerOwnership] = &[HandlerOwnership {
        file: "signal.rs",
        data_only: &[],
        private_metadata: &[],
        required_metadata: GLOB_SIGNAL_REQUIRED,
        optional_metadata: &[],
    }];
    let signal_glob_sources = [(
        "signal.rs",
        r#"
use ::kanban_contract::{MetadataEnvelope, SignalFilterMeta};
use ::std::vec::Vec;
use evil as kanban_sqlite;
use kanban_sqlite::api::*;
fn signal_handler(
) -> MetadataEnvelope<Vec<SignalRecord>, SignalFilterMeta> {
    MetadataEnvelope::new(
        vec![],
        SignalFilterMeta { include_all: false, limit: 10 },
    )
}
"#
        .to_owned(),
    )]
    .into_iter()
    .collect();
    let signal_glob_violations = validate_catalog(&signal_glob_sources, GLOB_SIGNAL_CATALOG);
    assert!(
        contains_file_violation(
            &signal_glob_violations,
            "signal.rs",
            ViolationKind::WrongReturnEnvelope,
            Some("signal_handler"),
        ),
        "shadowed kanban_sqlite glob must not prove SignalRecord: {signal_glob_violations:#?}"
    );

    const FOREIGN_GLOB_IMPORTS: &str = r#"
use evil::*;
use ::kanban_contract::{
    ApiTask, CreatedLabelsMeta, OptionalMetadataEnvelope,
    TaskOntologyDetailsMeta,
};
use ::core::option::Option;
use ::kanban_sqlite::api::TaskOntologySummary;
"#;
    let foreign_glob_source = optional_synthetic_source(
        FOREIGN_GLOB_IMPORTS,
        VALID_OPTIONAL_HANDLER,
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let foreign_glob_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &foreign_glob_source),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &foreign_glob_violations,
            "optional.rs",
            ViolationKind::WrongReturnEnvelope,
            Some("optional_handler"),
        ),
        "foreign glob ApiLabel ambiguity: {foreign_glob_violations:#?}"
    );
    assert!(
        !foreign_glob_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "explicit canonical sibling imports were polluted: {foreign_glob_violations:#?}"
    );

    const RELATIVE_DIRECT_WITH_FOREIGN_GLOB: &str = r#"
use evil::*;
use kanban_contract::{
    ApiLabel, ApiTask, CreatedLabelsMeta, OptionalMetadataEnvelope, TaskOntologyDetailsMeta,
};
use ::core::option::Option;
use ::kanban_sqlite::api::TaskOntologySummary;
"#;
    let relative_direct_source = optional_synthetic_source(
        RELATIVE_DIRECT_WITH_FOREIGN_GLOB,
        VALID_OPTIONAL_HANDLER,
        ABSOLUTE_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let relative_direct_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &relative_direct_source),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &relative_direct_violations,
            "optional.rs",
            ViolationKind::MissingEnvelopeResponse,
            Some("optional_handler"),
        ),
        "relative contract direct import is not proof under foreign glob: {relative_direct_violations:#?}"
    );
    assert!(
        !relative_direct_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "relative import ambiguity polluted absolute sibling: {relative_direct_violations:#?}"
    );

    const ABSOLUTE_CANONICAL_GLOB_IMPORTS: &str = r#"
use evil::*;
use ::kanban_contract::{
    ApiLabel, ApiTask, CreatedLabelsMeta, OptionalMetadataEnvelope, TaskOntologyDetailsMeta,
};
use ::core::option::Option;
use ::kanban_sqlite::api::TaskOntologySummary;
"#;
    let absolute_canonical_source = optional_synthetic_source(
        ABSOLUTE_CANONICAL_GLOB_IMPORTS,
        VALID_OPTIONAL_HANDLER,
        VALID_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let absolute_canonical_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &absolute_canonical_source),
        SYNTHETIC_CATALOG,
    );
    assert!(
        absolute_canonical_violations.is_empty(),
        "absolute external and crate::dto direct imports must outrank foreign glob uncertainty: {absolute_canonical_violations:#?}"
    );

    let glob_root_source = optional_synthetic_source(
        r#"
mod kanban_contract {}
use evil::*;
use kanban_contract::{
    CreatedLabelsMeta, OptionalMetadataEnvelope, TaskOntologyDetailsMeta,
};
"#,
        VALID_OPTIONAL_HANDLER,
        ABSOLUTE_NESTED_OPTIONAL_HANDLER,
        "",
    );
    let glob_root_violations = validate_catalog(
        &synthetic_sources_with_optional(VALID_SYNTHETIC_SOURCE, &glob_root_source),
        SYNTHETIC_CATALOG,
    );
    assert!(
        contains_file_violation(
            &glob_root_violations,
            "optional.rs",
            ViolationKind::MissingEnvelopeResponse,
            Some("optional_handler"),
        ),
        "glob plus module root shadow: {glob_root_violations:#?}"
    );
    assert!(
        !glob_root_violations
            .iter()
            .any(|violation| violation.function.as_deref() == Some("nested_optional_handler")),
        "glob/root shadow polluted absolute canonical sibling: {glob_root_violations:#?}"
    );
}

fn required_body_fixture(source: &str, spec: RequiredMetadataSpec) -> (BodyShape, Vec<Violation>) {
    let function = syn::parse_str::<ItemFn>(source).expect("required body fixture");
    let body = body_shape(&function);
    let mut violations = Vec::new();
    validate_required_body(&body, spec, "fixture.rs", "handler", &mut violations);
    (body, violations)
}

#[test]
fn g3_required_metadata_body_shape_records_canonical_ast_once() {
    let (qualified, violations) = required_body_fixture(
        r#"
fn handler() {
    kanban_contract::MetadataEnvelope::<Vec<Relation>, LimitMeta>::new(
        data,
        kanban_contract::LimitMeta { limit: 7 },
    );
}
"#,
        G3_GRAPH[0],
    );
    assert!(
        violations.is_empty(),
        "qualified turbofish call: {violations:#?}"
    );
    assert_eq!(qualified.metadata_calls.len(), 1);
    assert_eq!(qualified.metadata_paths, 1);
    assert_eq!(qualified.metadata_literals, 0);
    let call = &qualified.metadata_calls[0];
    assert_eq!(call.arity, 2);
    let meta = call.meta_struct.as_ref().expect("direct struct metadata");
    assert!(meta_path_matches(
        &meta.path,
        meta.has_qself,
        LIMIT_META,
        &qualified.shadowed,
    ));
    assert_eq!(meta.fields, ["limit"]);
    assert!(!meta.has_unnamed);
    assert!(!meta.has_rest);

    let direct = body_shape(
        &syn::parse_str::<ItemFn>(
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { limit: 7 }); }",
        )
        .expect("direct constructor fixture"),
    );
    assert_eq!(direct.metadata_calls.len(), 1);
    assert_eq!(direct.metadata_paths, 1);

    let repeated = body_shape(
        &syn::parse_str::<ItemFn>(
            r#"
fn handler() {
    MetadataEnvelope::new(data, LimitMeta { limit: 7 });
    MetadataEnvelope::new(data, LimitMeta { limit: 8 });
}
"#,
        )
        .expect("repeated constructor fixture"),
    );
    assert_eq!(repeated.metadata_calls.len(), 2);
    assert_eq!(repeated.metadata_paths, 2);

    let literal = body_shape(
        &syn::parse_str::<ItemFn>(
            "fn handler() { MetadataEnvelope { data, meta: LimitMeta { limit: 7 } }; }",
        )
        .expect("metadata literal fixture"),
    );
    assert!(literal.metadata_calls.is_empty());
    assert_eq!(literal.metadata_paths, 1);
    assert_eq!(literal.metadata_literals, 1);

    let middle = syn::parse_str::<syn::Path>("foo::Envelope::helper").expect("path fixture");
    assert_eq!(
        envelope_kind(&middle, false, &std::collections::BTreeSet::new()),
        None
    );
}

#[test]
fn g3_required_metadata_body_shape_is_spec_driven_and_closed() {
    let valid = [
        (
            G3_GRAPH[0],
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { limit: 7 }); }",
        ),
        (
            G3_SEARCH[0],
            "fn handler() { MetadataEnvelope::new(data, OffsetPaginationMeta { offset: 2, limit: 7 }); }",
        ),
        (
            G3_TASKS[0],
            "fn handler() { MetadataEnvelope::new(data, TotalPaginationMeta { total: 9, offset: 2, limit: 7 }); }",
        ),
    ];
    for (spec, source) in valid {
        let (_, violations) = required_body_fixture(source, spec);
        assert!(
            violations.is_empty(),
            "{} valid body: {violations:#?}",
            spec.function
        );
    }

    let cases = [
        (
            "multiple constructors",
            ViolationKind::WrongRequiredMetadataConstructor,
            r#"
fn handler() {
    MetadataEnvelope::new(data, LimitMeta { limit: 7 });
    MetadataEnvelope::new(data, LimitMeta { limit: 8 });
}
"#,
        ),
        (
            "zero arity",
            ViolationKind::WrongRequiredMetadataConstructor,
            "fn handler() { MetadataEnvelope::new(); }",
        ),
        (
            "one arity",
            ViolationKind::WrongRequiredMetadataConstructor,
            "fn handler() { MetadataEnvelope::new(data); }",
        ),
        (
            "three arity",
            ViolationKind::WrongRequiredMetadataConstructor,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { limit: 7 }, extra); }",
        ),
        (
            "wrong metadata type",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, OtherMeta { limit: 7 }); }",
        ),
        (
            "generic metadata type",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta::<u8> { limit: 7 }); }",
        ),
        (
            "missing field",
            ViolationKind::WrongRequiredMetadataFields,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta {}); }",
        ),
        (
            "extra field",
            ViolationKind::WrongRequiredMetadataFields,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { limit: 7, extra: 1 }); }",
        ),
        (
            "unnamed field",
            ViolationKind::WrongRequiredMetadataFields,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { 0: 7, limit: 7 }); }",
        ),
        (
            "rest field",
            ViolationKind::WrongRequiredMetadataFields,
            "fn handler() { MetadataEnvelope::new(data, LimitMeta { limit: 7, ..base }); }",
        ),
        (
            "parenthesized metadata",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, (LimitMeta { limit: 7 })); }",
        ),
        (
            "block metadata",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, { LimitMeta { limit: 7 } }); }",
        ),
        (
            "macro metadata",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, meta!()); }",
        ),
        (
            "call metadata",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, make_meta()); }",
        ),
        (
            "path metadata",
            ViolationKind::WrongRequiredMetadataBodyType,
            "fn handler() { MetadataEnvelope::new(data, meta); }",
        ),
        (
            "metadata literal",
            ViolationKind::ForbiddenRequiredMetadataLiteral,
            "fn handler() { MetadataEnvelope { data, meta: LimitMeta { limit: 7 } }; }",
        ),
        (
            "extra metadata path",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            r#"
fn handler() {
    let _: MetadataEnvelope<i32, LimitMeta>;
    MetadataEnvelope::new(data, LimitMeta { limit: 7 });
}
"#,
        ),
        (
            "data family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            "fn handler() { DataEnvelope::new(data); }",
        ),
        (
            "private family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            "fn handler() { Envelope { data, meta: None }; }",
        ),
        (
            "optional family",
            ViolationKind::ForbiddenRequiredMetadataFamily,
            "fn handler() { OptionalMetadataEnvelope::new(data, None); }",
        ),
    ];
    for (label, kind, source) in cases {
        let (_, violations) = required_body_fixture(source, G3_GRAPH[0]);
        assert!(
            contains_violation(&violations, kind, Some("handler")),
            "{label} did not produce {kind:?}: {violations:#?}"
        );
    }
}
