#![cfg(feature = "schema")]
#![recursion_limit = "256"]

use std::collections::BTreeSet;

use kanban_contract::{
    ApiHeaderProfile, CliMachineOutput, ContractBinding, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, ContractTransport, EndpointDescriptor, EndpointObligation,
    EndpointObligations, HttpMethod, HttpTransportLocation, MigrationState, OperationContract,
    WireParameter, WireParameterCardinality, api_header_contract_specs, cli_operation_catalog,
    endpoint_catalog, endpoint_descriptor, endpoint_obligation_todo_count, generated_artifacts,
    generated_schema_ids, operation_inventory, surface_operation_catalog,
    validate_contract_topology, validate_endpoint_catalog,
};

#[test]
fn exact_request_dtos_reject_legacy_wire_aliases() {
    assert!(
        serde_json::from_value::<kanban_contract::SignalQuery>(serde_json::json!({
            "task": "default#1"
        }))
        .is_err()
    );
    for alias in [
        serde_json::json!({"task": "default#1"}),
        serde_json::json!({"label": "cli"}),
        serde_json::json!({"proposed_label": "database"}),
    ] {
        assert!(
            serde_json::from_value::<kanban_contract::LabelOntologySignalQuery>(alias).is_err()
        );
    }
    assert!(
        serde_json::from_value::<kanban_contract::LabelOntologyReviewGroupByWire>(
            serde_json::json!("candidate-atom")
        )
        .is_err()
    );
}

#[test]
fn portable_contract_catalog_freezes_all_jsonl_discriminators() {
    use kanban_contract::{PortableContractLane, portable_contract_catalog, schema_registry};

    let catalog = portable_contract_catalog();
    let discriminators = catalog
        .iter()
        .map(|descriptor| descriptor.discriminator)
        .collect::<BTreeSet<_>>();

    assert_eq!(catalog.len(), 21);
    assert_eq!(discriminators.len(), catalog.len());
    assert_eq!(
        catalog
            .iter()
            .map(|descriptor| descriptor.discriminator)
            .collect::<Vec<_>>(),
        vec![
            "board",
            "column",
            "task",
            "dependency",
            "run",
            "comment",
            "signal_observation",
            "signal",
            "event",
            "attachment",
            "label",
            "label_semantics",
            "label_atom",
            "label_semantic_proposal",
            "label_ontology_observation",
            "label_ontology_signal",
            "label_ontology_action",
            "label_ontology_action_atom_effect",
            "label_ontology_action_signal",
            "task_label",
            "setting",
        ],
        "portable catalog order is the dependency-safe export/import order"
    );
    assert_eq!(
        discriminators,
        BTreeSet::from([
            "attachment",
            "board",
            "column",
            "comment",
            "dependency",
            "event",
            "label",
            "label_atom",
            "label_ontology_action",
            "label_ontology_action_atom_effect",
            "label_ontology_action_signal",
            "label_ontology_observation",
            "label_ontology_signal",
            "label_semantic_proposal",
            "label_semantics",
            "run",
            "setting",
            "signal",
            "signal_observation",
            "task",
            "task_label",
        ])
    );

    let contract_ids = catalog
        .iter()
        .flat_map(|descriptor| [descriptor.input.contract_id, descriptor.output.contract_id])
        .collect::<BTreeSet<_>>();
    let schema_ids = catalog
        .iter()
        .flat_map(|descriptor| [descriptor.input.schema_id, descriptor.output.schema_id])
        .collect::<BTreeSet<_>>();
    let fixtures = catalog
        .iter()
        .flat_map(|descriptor| [descriptor.input.fixture, descriptor.output.fixture])
        .collect::<BTreeSet<_>>();
    let invalid_fixtures = catalog
        .iter()
        .flat_map(|descriptor| {
            [
                descriptor.input.invalid_fixture,
                descriptor.output.invalid_fixture,
            ]
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(contract_ids.len(), 42);
    assert_eq!(schema_ids.len(), 42);
    assert_eq!(fixtures.len(), 42);
    assert_eq!(invalid_fixtures.len(), 42);

    let inventory = operation_inventory();
    let roots = schema_registry();
    let surfaces = surface_operation_catalog();
    for descriptor in catalog {
        let surface = surfaces
            .iter()
            .find(|surface| {
                surface.surface == ContractSurface::Jsonl && surface.key == descriptor.operation_key
            })
            .unwrap_or_else(|| panic!("missing JSONL surface {}", descriptor.operation_key));
        assert_eq!(surface.migration, MigrationState::Adopted);
        assert_eq!(
            surface.contracts,
            vec![descriptor.input.contract_id, descriptor.output.contract_id]
        );

        for (side, direction) in [
            (&descriptor.input, ContractDirection::Deserialize),
            (&descriptor.output, ContractDirection::Serialize),
        ] {
            let contract = inventory
                .iter()
                .find(|contract| contract.id == side.contract_id)
                .unwrap_or_else(|| panic!("missing JSONL contract {}", side.contract_id));
            assert_eq!(contract.migration, MigrationState::Adopted);
            assert_eq!(contract.direction, direction);
            assert_eq!(contract.granularity, ContractGranularity::Exact);
            assert_eq!(contract.binding, ContractBinding::ExactSurface);
            assert_eq!(contract.transport, ContractTransport::NoTransport);
            assert_eq!(contract.schema_id, Some(side.schema_id));
            assert_eq!(contract.fixture, Some(side.fixture));
            let adoption = contract.adoption.expect("adopted JSONL witness");
            assert_eq!(adoption.producer_fixture, side.fixture);
            assert_eq!(adoption.producer.test_target, side.test_target);
            assert_eq!(adoption.producer.exact_test, side.producer_test);
            assert_eq!(adoption.consumer.test_target, side.test_target);
            assert_eq!(adoption.consumer.exact_test, side.consumer_test);

            let root = roots
                .iter()
                .find(|root| root.contract_id == side.contract_id)
                .unwrap_or_else(|| panic!("missing JSONL schema root {}", side.contract_id));
            assert_eq!(root.id, side.schema_id);
            assert_eq!(root.valid_fixture, side.fixture);
            assert_eq!(root.invalid_fixture, side.invalid_fixture);
        }
    }

    for descriptor in catalog {
        assert_eq!(
            descriptor.operation_key,
            format!("type={}", descriptor.discriminator)
        );
        assert_eq!(
            descriptor.input.contract_id,
            format!("jsonl.{}.input", descriptor.discriminator)
        );
        assert_eq!(
            descriptor.output.contract_id,
            format!("jsonl.{}.output", descriptor.discriminator)
        );
        assert_eq!(
            descriptor.input.schema_id,
            format!(
                "urn:kanban-tool:schema:jsonl:{}-input:v1",
                descriptor.discriminator
            )
        );
        assert_eq!(
            descriptor.output.schema_id,
            format!(
                "urn:kanban-tool:schema:jsonl:{}-output:v1",
                descriptor.discriminator
            )
        );
        assert!(descriptor.input.fixture.ends_with("-input.v1.valid.json"));
        assert!(descriptor.output.fixture.ends_with("-output.v1.valid.json"));
        assert!(
            descriptor
                .input
                .invalid_fixture
                .ends_with("-input.v1.invalid.json")
        );
        assert!(
            descriptor
                .output
                .invalid_fixture
                .ends_with("-output.v1.invalid.json")
        );
        assert_eq!(descriptor.input.test_target, descriptor.output.test_target);
    }

    assert_eq!(
        catalog
            .iter()
            .filter(|descriptor| descriptor.lane == PortableContractLane::Core)
            .count(),
        9
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|descriptor| descriptor.lane == PortableContractLane::Ledger)
            .count(),
        12
    );

    let jsonl_surfaces = surface_operation_catalog()
        .into_iter()
        .filter(|operation| operation.surface == ContractSurface::Jsonl)
        .collect::<Vec<_>>();
    assert_eq!(jsonl_surfaces.len(), catalog.len());
    assert_eq!(
        jsonl_surfaces
            .iter()
            .map(|operation| operation.key.as_str())
            .collect::<BTreeSet<_>>(),
        catalog
            .iter()
            .map(|descriptor| descriptor.operation_key)
            .collect::<BTreeSet<_>>()
    );
    assert!(jsonl_surfaces.iter().all(|operation| {
        operation.migration == MigrationState::Adopted && operation.contracts.len() == 2
    }));
}

const B4_C2_LABEL_OPERATION_IDS: &[&str] = &[
    "api.list-board-labels",
    "api.create-board-label",
    "api.list-label-semantics",
    "api.get-label-semantics",
    "api.upsert-label-semantics",
    "api.delete-label-semantics",
    "api.list-label-atoms",
    "api.explain-label-atom",
    "api.label-atom-index-status",
    "api.rebuild-label-atom-index",
    "api.query-label-atom-index",
    "api.list-task-labels",
    "api.add-task-label",
    "api.bootstrap-task-label",
    "api.suggest-task-labels",
    "api.list-task-label-proposals",
    "api.propose-task-label",
    "api.record-label-ontology-observation",
    "api.list-label-ontology-signals",
    "api.review-label-ontology",
    "api.create-label-ontology-action",
    "api.apply-label-ontology-atom",
    "api.revert-label-ontology-mutation",
    "api.validate-label-ontology-action",
    "api.get-label-ontology-signal",
    "api.get-label-proposal",
    "api.accept-label-proposal",
    "api.reject-label-proposal",
    "api.remove-task-label",
];

#[test]
fn b7_exact_header_contracts_cover_every_non_sse_endpoint() {
    let endpoints = endpoint_catalog()
        .iter()
        .filter(|endpoint| endpoint.operation_id != "sse.stream-events")
        .collect::<Vec<_>>();
    // 当前基线已有 98 个 JSON endpoint，本提交相对 graph 基线新增 6 个。
    assert_eq!(endpoints.len(), 104);

    for endpoint in endpoints {
        let EndpointObligation::Contract(expected_id) = endpoint.obligations.headers else {
            panic!(
                "{} must declare an exact header contract",
                endpoint.operation_id
            );
        };
        assert_eq!(
            endpoint.obligations.headers,
            EndpointObligation::Contract(expected_id),
            "{}",
            endpoint.operation_id
        );
        let contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == expected_id)
            .unwrap_or_else(|| panic!("missing exact header contract: {expected_id}"));
        assert_eq!(contract.binding, ContractBinding::ExactSurface);
        assert_eq!(contract.granularity, ContractGranularity::Exact);
        assert_eq!(contract.direction, ContractDirection::Deserialize);
        assert_eq!(contract.migration, MigrationState::Adopted);
        match contract.transport {
            ContractTransport::Http {
                operation_key: Some(_),
                location: HttpTransportLocation::Headers,
                parameters,
            } => assert!(
                parameters
                    .iter()
                    .any(|parameter| parameter.name == "Accept-Language"),
                "{}",
                endpoint.operation_id
            ),
            other => panic!(
                "invalid header transport for {}: {other:?}",
                endpoint.operation_id
            ),
        }
    }
}

#[test]
fn b7_header_profiles_fail_closed_over_actor_and_body_cardinality() {
    let specs = api_header_contract_specs();
    let actor_operations = specs
        .iter()
        .filter(|spec| {
            spec.profile
                .parameters()
                .iter()
                .any(|parameter| parameter.name == "X-KB-Actor")
        })
        .map(|spec| spec.endpoint.operation_id)
        .collect::<BTreeSet<_>>();
    let expected_actor_operations = [
        "api.accept-label-proposal",
        "api.add-dependency",
        "api.add-task-label",
        "api.archive-board",
        "api.archive-task",
        "api.block-task",
        "api.bootstrap-task-label",
        "api.claim-task",
        "api.complete-step",
        "api.complete-task",
        "api.create-board",
        "api.create-comment",
        "api.create-step",
        "api.create-task",
        "api.delete-label-semantics",
        "api.heartbeat-task",
        "api.mark-execution-plan-not-required",
        "api.promote-task",
        "api.propose-task-label",
        "api.reclaim-task",
        "api.release-task",
        "api.remove-dependency",
        "api.remove-step",
        "api.remove-task-label",
        "api.reopen-step",
        "api.reopen-task",
        "api.reject-label-proposal",
        "api.skip-step",
        "api.specify-task",
        "api.submit-review-task",
        "api.unblock-task",
        "api.update-step",
        "api.update-task",
        "api.upsert-label-semantics",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(actor_operations, expected_actor_operations);

    let optional_body_operations = specs
        .iter()
        .filter(|spec| matches!(spec.profile, ApiHeaderProfile::LocaleActorOptionalJson))
        .map(|spec| spec.endpoint.operation_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        optional_body_operations,
        [
            "api.accept-label-proposal",
            "api.archive-board",
            "api.archive-task",
            "api.promote-task",
            "api.propose-task-label",
            "api.reclaim-task",
            "api.reject-label-proposal",
            "api.unblock-task",
        ]
        .into_iter()
        .collect()
    );

    for spec in specs {
        let content_type = spec
            .profile
            .parameters()
            .iter()
            .find(|parameter| parameter.name == "Content-Type");
        let has_body = matches!(
            spec.endpoint.obligations.body,
            EndpointObligation::Contract(_)
        );
        assert_eq!(
            content_type.is_some(),
            has_body,
            "{}",
            spec.endpoint.operation_id
        );
        if let Some(content_type) = content_type {
            let expected = if optional_body_operations.contains(spec.endpoint.operation_id) {
                WireParameterCardinality::OptionalOne
            } else {
                WireParameterCardinality::RequiredOne
            };
            assert_eq!(
                content_type.cardinality,
                Some(expected),
                "{}",
                spec.endpoint.operation_id
            );
        }
    }
}

#[test]
fn b4_c2_label_operations_exactly_own_all_non_header_dimensions() {
    assert_eq!(B4_C2_LABEL_OPERATION_IDS.len(), 29);
    for operation_id in B4_C2_LABEL_OPERATION_IDS {
        let endpoint = endpoint_descriptor(operation_id).expect("B4-C2 endpoint descriptor");
        assert_eq!(
            endpoint.migration,
            MigrationState::Adopted,
            "{operation_id}"
        );
        assert_eq!(
            endpoint.obligations.headers,
            EndpointObligation::Contract(Box::leak(
                format!("{operation_id}.headers").into_boxed_str()
            )),
            "{operation_id}"
        );
        assert_eq!(
            endpoint.obligations.sse,
            EndpointObligation::NotApplicable,
            "{operation_id}"
        );
        for (dimension, obligation) in [
            ("path", endpoint.obligations.path),
            ("query", endpoint.obligations.query),
            ("body", endpoint.obligations.body),
            ("success", endpoint.obligations.success),
        ] {
            assert_ne!(
                obligation,
                EndpointObligation::Todo,
                "{operation_id} {dimension}"
            );
        }
    }
}

#[test]
fn structured_metadata_contracts_have_exact_roots_surfaces_and_adopter_witnesses() {
    use kanban_contract::schema_registry;

    let cases = [
        (
            "metadata.decision.input",
            "structured decision comment metadata input",
            ContractDirection::Deserialize,
            "urn:kanban-tool:schema:metadata:decision:v1",
            "schemas/fixtures/metadata/decision.v1.valid.json",
            "metadata_contract_adoption",
            "metadata_decision_input_fixture_is_produced_by_cli_contract_dto",
            "comments",
            "metadata_decision_input_fixture_is_consumed_by_real_cli",
        ),
        (
            "metadata.signal-record.input",
            "generic signal record input",
            ContractDirection::Deserialize,
            "urn:kanban-tool:schema:metadata:signal-record-input:v1",
            "schemas/fixtures/metadata/signal-record-input.v1.valid.json",
            "metadata_contract_adoption",
            "metadata_signal_record_input_fixture_is_produced_by_cli_contract_dto",
            "signal",
            "metadata_signal_record_input_fixture_is_consumed_by_real_cli",
        ),
        (
            "metadata.signal-link.output",
            "signal backlink comment metadata output",
            ContractDirection::Serialize,
            "urn:kanban-tool:schema:metadata:signal-link-output:v1",
            "schemas/fixtures/metadata/signal-link-output.v1.valid.json",
            "signal",
            "metadata_signal_link_output_fixture_is_produced_by_real_service_adapter",
            "metadata_contract_adoption",
            "metadata_signal_link_output_fixture_is_consumed_by_cli_contract_dto",
        ),
        (
            "metadata.label-proposal-candidate.input",
            "label proposal candidate input",
            ContractDirection::Deserialize,
            "urn:kanban-tool:schema:metadata:label-proposal-candidate-input:v1",
            "schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json",
            "metadata_contract_adoption",
            "metadata_label_proposal_candidate_input_fixture_is_produced_by_cli_contract_dto",
            "task",
            "metadata_label_proposal_candidate_input_fixture_is_consumed_by_real_cli",
        ),
        (
            "metadata.ontology-record.input",
            "label ontology observation input",
            ContractDirection::Deserialize,
            "urn:kanban-tool:schema:metadata:ontology-record-input:v1",
            "schemas/fixtures/metadata/ontology-record-input.v1.valid.json",
            "metadata_contract_adoption",
            "metadata_ontology_record_input_fixture_is_produced_by_cli_contract_dto",
            "task",
            "metadata_ontology_record_input_fixture_is_consumed_by_real_cli",
        ),
        (
            "metadata.ontology-validation-evidence.input",
            "label ontology external validation evidence",
            ContractDirection::Deserialize,
            "urn:kanban-tool:schema:metadata:ontology-validation-evidence-input:v1",
            "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json",
            "metadata_contract_adoption",
            "metadata_ontology_validation_evidence_input_fixture_is_produced_by_cli_contract_dto",
            "cli_label_contract_adoption",
            "metadata_ontology_validation_evidence_input_fixture_is_consumed_by_real_cli",
        ),
    ];

    for (
        contract_id,
        operation,
        direction,
        schema_id,
        fixture,
        producer_target,
        producer_test,
        consumer_target,
        consumer_test,
    ) in cases
    {
        let contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == contract_id)
            .unwrap_or_else(|| panic!("missing metadata contract {contract_id}"));
        assert_eq!(contract.operation, operation);
        assert_eq!(contract.surface, ContractSurface::Metadata);
        assert_eq!(contract.direction, direction);
        assert_eq!(contract.migration, MigrationState::Adopted);
        assert_eq!(contract.granularity, ContractGranularity::Exact);
        assert_eq!(contract.binding, ContractBinding::ExactSurface);
        assert_eq!(contract.transport, ContractTransport::NoTransport);
        assert_eq!(contract.schema_id, Some(schema_id));
        assert_eq!(contract.fixture, Some(fixture));
        let adoption = contract.adoption.expect("adopted metadata witness");
        assert_eq!(adoption.producer_fixture, fixture);
        assert_eq!(adoption.producer.package, "kanban-cli");
        assert_eq!(adoption.producer.test_target, producer_target);
        assert_eq!(adoption.producer.exact_test, producer_test);
        assert_eq!(adoption.consumer.package, "kanban-cli");
        assert_eq!(adoption.consumer.test_target, consumer_target);
        assert_eq!(adoption.consumer.exact_test, consumer_test);

        let root = schema_registry()
            .iter()
            .find(|root| root.contract_id == contract_id)
            .unwrap_or_else(|| panic!("missing metadata root {contract_id}"));
        assert_eq!(root.id, schema_id);
        assert_eq!(root.valid_fixture, fixture);

        let surface = surface_operation_catalog()
            .into_iter()
            .find(|surface| {
                surface.surface == ContractSurface::Metadata && surface.key == operation
            })
            .unwrap_or_else(|| panic!("missing metadata surface {operation}"));
        assert_eq!(surface.migration, MigrationState::Adopted);
        assert_eq!(surface.contracts, vec![contract_id]);
    }
}

#[test]
fn public_operation_inventory_covers_every_public_surface() {
    let actual = operation_inventory()
        .iter()
        .map(|entry| entry.surface)
        .collect::<BTreeSet<_>>();
    let actual = actual
        .into_iter()
        .chain(
            surface_operation_catalog()
                .iter()
                .map(|entry| entry.surface),
        )
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        ContractSurface::Api,
        ContractSurface::Cli,
        ContractSurface::Jsonl,
        ContractSurface::Sse,
        ContractSurface::Metadata,
        ContractSurface::Config,
        ContractSurface::Helper,
    ]);
    assert_eq!(
        actual, expected,
        "inventory 必须显式覆盖全部公开 JSON surface"
    );
}

#[test]
fn cli_machine_output_obligations_cover_every_cli_leaf_exactly() {
    let operations = cli_operation_catalog();
    let actual = operations
        .iter()
        .map(|operation| operation.key.clone())
        .collect::<BTreeSet<_>>();
    let expected = surface_operation_catalog()
        .into_iter()
        .filter(|operation| operation.surface == ContractSurface::Cli)
        .map(|operation| operation.key)
        .collect::<BTreeSet<_>>();

    assert_eq!(actual.len(), operations.len(), "CLI operation key 不得重复");
    assert_eq!(
        actual, expected,
        "CLI machine obligation 必须精确覆盖 clap leaf catalog"
    );
    for operation in operations {
        match operation.machine_output {
            CliMachineOutput::Excluded { reason } => {
                assert!(
                    !reason.trim().is_empty(),
                    "{} exclusion 缺少理由",
                    operation.key
                );
            }
            CliMachineOutput::Todo | CliMachineOutput::Contract { .. } => {}
        }
    }
}

#[test]
fn foundation_registry_contains_generated_roots() {
    let mut actual = generated_schema_ids()
        .iter()
        .copied()
        .filter(|id| !id.contains(":schema:cli:"))
        .collect::<BTreeSet<_>>();
    let header_roots = actual
        .iter()
        .filter(|id| id.contains(":api:") && id.ends_with("-headers:v1"))
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(header_roots.len(), 104);
    actual.retain(|id| !header_roots.contains(id));
    let mut expected = BTreeSet::from([
        "urn:kanban-tool:schema:api:accept-label-proposal-body:v1",
        "urn:kanban-tool:schema:api:accept-label-proposal-path:v1",
        "urn:kanban-tool:schema:api:accept-label-proposal-response:v1",
        "urn:kanban-tool:schema:api:add-dependency-path:v1",
        "urn:kanban-tool:schema:api:add-dependency-request:v1",
        "urn:kanban-tool:schema:api:add-dependency-response:v1",
        "urn:kanban-tool:schema:api:add-task-label-path:v1",
        "urn:kanban-tool:schema:api:add-task-label-request:v1",
        "urn:kanban-tool:schema:api:add-task-label-response:v1",
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-path:v1",
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-request:v1",
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-response:v1",
        "urn:kanban-tool:schema:api:archive-board-path:v1",
        "urn:kanban-tool:schema:api:archive-board-request:v1",
        "urn:kanban-tool:schema:api:archive-board-response:v1",
        "urn:kanban-tool:schema:api:archive-task-path:v1",
        "urn:kanban-tool:schema:api:archive-task-request:v1",
        "urn:kanban-tool:schema:api:archive-task-response:v1",
        "urn:kanban-tool:schema:api:block-task-path:v1",
        "urn:kanban-tool:schema:api:block-task-request:v1",
        "urn:kanban-tool:schema:api:block-task-response:v1",
        "urn:kanban-tool:schema:api:board-task-map-path:v1",
        "urn:kanban-tool:schema:api:board-task-map-query:v1",
        "urn:kanban-tool:schema:api:board-task-map-response:v1",
        "urn:kanban-tool:schema:api:bootstrap-task-label-path:v1",
        "urn:kanban-tool:schema:api:bootstrap-task-label-request:v1",
        "urn:kanban-tool:schema:api:bootstrap-task-label-response:v1",
        "urn:kanban-tool:schema:api:checkpoint-response:v1",
        "urn:kanban-tool:schema:api:maintenance-path-request:v1",
        "urn:kanban-tool:schema:api:maintenance-backup-request:v1",
        "urn:kanban-tool:schema:api:maintenance-export-request:v1",
        "urn:kanban-tool:schema:api:maintenance-import-request:v1",
        "urn:kanban-tool:schema:api:maintenance-import-v30-request:v1",
        "urn:kanban-tool:schema:api:maintenance-run-request:v1",
        "urn:kanban-tool:schema:api:maintenance-rebuild-request:v1",
        "urn:kanban-tool:schema:api:maintenance-cleanup-request:v1",
        "urn:kanban-tool:schema:api:maintenance-backup-response:v1",
        "urn:kanban-tool:schema:api:maintenance-export-response:v1",
        "urn:kanban-tool:schema:api:maintenance-import-response:v1",
        "urn:kanban-tool:schema:api:maintenance-import-v30-response:v1",
        "urn:kanban-tool:schema:api:maintenance-vacuum-response:v1",
        "urn:kanban-tool:schema:api:maintenance-status-response:v1",
        "urn:kanban-tool:schema:api:maintenance-run-response:v1",
        "urn:kanban-tool:schema:api:maintenance-rebuild-response:v1",
        "urn:kanban-tool:schema:api:maintenance-cleanup-response:v1",
        "urn:kanban-tool:schema:api:claim-task-path:v1",
        "urn:kanban-tool:schema:api:claim-task-request:v1",
        "urn:kanban-tool:schema:api:claim-task-response:v1",
        "urn:kanban-tool:schema:api:complete-step-path:v1",
        "urn:kanban-tool:schema:api:complete-step-request:v1",
        "urn:kanban-tool:schema:api:complete-step-response:v1",
        "urn:kanban-tool:schema:api:complete-task-path:v1",
        "urn:kanban-tool:schema:api:complete-task-request:v1",
        "urn:kanban-tool:schema:api:complete-task-response:v1",
        "urn:kanban-tool:schema:api:create-board-label-path:v1",
        "urn:kanban-tool:schema:api:create-board-label-request:v1",
        "urn:kanban-tool:schema:api:create-board-label-response:v1",
        "urn:kanban-tool:schema:api:create-board-request:v1",
        "urn:kanban-tool:schema:api:create-board-response:v1",
        "urn:kanban-tool:schema:api:create-comment-path:v1",
        "urn:kanban-tool:schema:api:create-comment-request:v1",
        "urn:kanban-tool:schema:api:create-comment-response:v1",
        "urn:kanban-tool:schema:api:create-label-ontology-action-path:v1",
        "urn:kanban-tool:schema:api:create-label-ontology-action-request:v1",
        "urn:kanban-tool:schema:api:create-label-ontology-action-response:v1",
        "urn:kanban-tool:schema:api:create-step-path:v1",
        "urn:kanban-tool:schema:api:create-step-request:v1",
        "urn:kanban-tool:schema:api:create-step-response:v1",
        "urn:kanban-tool:schema:api:create-task-path:v1",
        "urn:kanban-tool:schema:api:create-task-request:v1",
        "urn:kanban-tool:schema:api:create-task-response:v1",
        "urn:kanban-tool:schema:api:delete-label-semantics-path:v1",
        "urn:kanban-tool:schema:api:delete-label-semantics-query:v1",
        "urn:kanban-tool:schema:api:delete-response:v1",
        "urn:kanban-tool:schema:api:doctor-response:v1",
        "urn:kanban-tool:schema:api:error-response:v1",
        "urn:kanban-tool:schema:api:explain-label-atom-response:v1",
        "urn:kanban-tool:schema:api:get-board-path:v1",
        "urn:kanban-tool:schema:api:get-board-response:v1",
        "urn:kanban-tool:schema:api:get-label-ontology-signal-path:v1",
        "urn:kanban-tool:schema:api:get-label-ontology-signal-response:v1",
        "urn:kanban-tool:schema:api:get-label-proposal-path:v1",
        "urn:kanban-tool:schema:api:get-label-proposal-response:v1",
        "urn:kanban-tool:schema:api:get-label-semantics-path:v1",
        "urn:kanban-tool:schema:api:get-label-semantics-response:v1",
        "urn:kanban-tool:schema:api:get-run-log-path:v1",
        "urn:kanban-tool:schema:api:get-run-log-response:v1",
        "urn:kanban-tool:schema:api:get-run-path:v1",
        "urn:kanban-tool:schema:api:get-run-response:v1",
        "urn:kanban-tool:schema:api:get-signal-path:v1",
        "urn:kanban-tool:schema:api:get-signal-response:v1",
        "urn:kanban-tool:schema:api:record-signal-path:v1",
        "urn:kanban-tool:schema:api:record-signal-request:v1",
        "urn:kanban-tool:schema:api:record-signal-response:v1",
        "urn:kanban-tool:schema:api:confirm-signals-path:v1",
        "urn:kanban-tool:schema:api:confirm-signals-response:v1",
        "urn:kanban-tool:schema:api:reject-signals-path:v1",
        "urn:kanban-tool:schema:api:reject-signals-response:v1",
        "urn:kanban-tool:schema:api:resolve-signals-path:v1",
        "urn:kanban-tool:schema:api:resolve-signals-response:v1",
        "urn:kanban-tool:schema:api:supersede-signals-path:v1",
        "urn:kanban-tool:schema:api:supersede-signals-response:v1",
        "urn:kanban-tool:schema:api:review-signals-request:v1",
        "urn:kanban-tool:schema:api:get-task-path:v1",
        "urn:kanban-tool:schema:api:get-task-query:v1",
        "urn:kanban-tool:schema:api:get-task-response:v1",
        "urn:kanban-tool:schema:api:update-task-path:v1",
        "urn:kanban-tool:schema:api:update-task-request:v1",
        "urn:kanban-tool:schema:api:update-task-response:v1",
        "urn:kanban-tool:schema:api:health-response:v1",
        "urn:kanban-tool:schema:api:heartbeat-task-path:v1",
        "urn:kanban-tool:schema:api:heartbeat-task-request:v1",
        "urn:kanban-tool:schema:api:heartbeat-task-response:v1",
        "urn:kanban-tool:schema:api:release-task-path:v1",
        "urn:kanban-tool:schema:api:release-task-request:v1",
        "urn:kanban-tool:schema:api:release-task-response:v1",
        "urn:kanban-tool:schema:api:label-atom-index-status-path:v1",
        "urn:kanban-tool:schema:api:label-atom-index-status-response:v1",
        "urn:kanban-tool:schema:api:label-atom-path:v1",
        "urn:kanban-tool:schema:api:label-ontology-review-query:v1",
        "urn:kanban-tool:schema:api:label-ontology-signal-query:v1",
        "urn:kanban-tool:schema:api:label-suggestion-query:v1",
        "urn:kanban-tool:schema:api:list-board-columns-path:v1",
        "urn:kanban-tool:schema:api:list-board-columns-response:v1",
        "urn:kanban-tool:schema:api:list-board-labels-path:v1",
        "urn:kanban-tool:schema:api:list-board-labels-response:v1",
        "urn:kanban-tool:schema:api:list-boards-query:v1",
        "urn:kanban-tool:schema:api:list-boards-response:v1",
        "urn:kanban-tool:schema:api:list-comments-path:v1",
        "urn:kanban-tool:schema:api:list-comments-response:v1",
        "urn:kanban-tool:schema:api:list-dependencies-path:v1",
        "urn:kanban-tool:schema:api:list-dependencies-response:v1",
        "urn:kanban-tool:schema:api:list-events-response:v1",
        "urn:kanban-tool:schema:api:list-label-atoms-path:v1",
        "urn:kanban-tool:schema:api:list-label-atoms-response:v1",
        "urn:kanban-tool:schema:api:list-label-ontology-signals-path:v1",
        "urn:kanban-tool:schema:api:list-label-ontology-signals-response:v1",
        "urn:kanban-tool:schema:api:list-label-semantics-path:v1",
        "urn:kanban-tool:schema:api:list-label-semantics-response:v1",
        "urn:kanban-tool:schema:api:list-runs-path:v1",
        "urn:kanban-tool:schema:api:list-runs-response:v1",
        "urn:kanban-tool:schema:api:list-signals-path:v1",
        "urn:kanban-tool:schema:api:list-signals-query:v1",
        "urn:kanban-tool:schema:api:list-signals-response:v1",
        "urn:kanban-tool:schema:api:list-steps-path:v1",
        "urn:kanban-tool:schema:api:list-steps-response:v1",
        "urn:kanban-tool:schema:api:list-task-label-proposals-path:v1",
        "urn:kanban-tool:schema:api:list-task-label-proposals-response:v1",
        "urn:kanban-tool:schema:api:list-task-labels-path:v1",
        "urn:kanban-tool:schema:api:list-task-labels-response:v1",
        "urn:kanban-tool:schema:api:list-tasks-by-status-path:v1",
        "urn:kanban-tool:schema:api:list-tasks-by-status-query:v1",
        "urn:kanban-tool:schema:api:list-tasks-by-status-response:v1",
        "urn:kanban-tool:schema:api:list-tasks-path:v1",
        "urn:kanban-tool:schema:api:list-tasks-query:v1",
        "urn:kanban-tool:schema:api:list-tasks-response:v1",
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-path:v1",
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-request:v1",
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-response:v1",
        "urn:kanban-tool:schema:api:promote-task-path:v1",
        "urn:kanban-tool:schema:api:promote-task-request:v1",
        "urn:kanban-tool:schema:api:promote-task-response:v1",
        "urn:kanban-tool:schema:api:propose-task-label-path:v1",
        "urn:kanban-tool:schema:api:propose-task-label-query:v1",
        "urn:kanban-tool:schema:api:propose-task-label-request:v1",
        "urn:kanban-tool:schema:api:propose-task-label-response:v1",
        "urn:kanban-tool:schema:api:query-label-atom-index-path:v1",
        "urn:kanban-tool:schema:api:query-label-atom-index-query:v1",
        "urn:kanban-tool:schema:api:query-label-atom-index-response:v1",
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-path:v1",
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-response:v1",
        "urn:kanban-tool:schema:api:reclaim-task-path:v1",
        "urn:kanban-tool:schema:api:reclaim-task-request:v1",
        "urn:kanban-tool:schema:api:reclaim-task-response:v1",
        "urn:kanban-tool:schema:api:record-label-ontology-observation-body:v1",
        "urn:kanban-tool:schema:api:record-label-ontology-observation-path:v1",
        "urn:kanban-tool:schema:api:record-label-ontology-observation-response:v1",
        "urn:kanban-tool:schema:api:reject-label-proposal-body:v1",
        "urn:kanban-tool:schema:api:reject-label-proposal-path:v1",
        "urn:kanban-tool:schema:api:reject-label-proposal-response:v1",
        "urn:kanban-tool:schema:api:remove-dependency-path:v1",
        "urn:kanban-tool:schema:api:remove-dependency-response:v1",
        "urn:kanban-tool:schema:api:remove-step-path:v1",
        "urn:kanban-tool:schema:api:remove-step-response:v1",
        "urn:kanban-tool:schema:api:remove-task-label-path:v1",
        "urn:kanban-tool:schema:api:remove-task-label-response:v1",
        "urn:kanban-tool:schema:api:reopen-step-path:v1",
        "urn:kanban-tool:schema:api:reopen-step-request:v1",
        "urn:kanban-tool:schema:api:reopen-step-response:v1",
        "urn:kanban-tool:schema:api:reopen-task-path:v1",
        "urn:kanban-tool:schema:api:reopen-task-request:v1",
        "urn:kanban-tool:schema:api:reopen-task-response:v1",
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-path:v1",
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-request:v1",
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-response:v1",
        "urn:kanban-tool:schema:api:review-label-ontology-path:v1",
        "urn:kanban-tool:schema:api:review-label-ontology-response:v1",
        "urn:kanban-tool:schema:api:review-signals-path:v1",
        "urn:kanban-tool:schema:api:review-signals-query:v1",
        "urn:kanban-tool:schema:api:review-signals-response:v1",
        "urn:kanban-tool:schema:api:skip-step-path:v1",
        "urn:kanban-tool:schema:api:skip-step-request:v1",
        "urn:kanban-tool:schema:api:skip-step-response:v1",
        "urn:kanban-tool:schema:api:specify-task-path:v1",
        "urn:kanban-tool:schema:api:specify-task-request:v1",
        "urn:kanban-tool:schema:api:specify-task-response:v1",
        "urn:kanban-tool:schema:api:submit-review-task-path:v1",
        "urn:kanban-tool:schema:api:submit-review-task-request:v1",
        "urn:kanban-tool:schema:api:submit-review-task-response:v1",
        "urn:kanban-tool:schema:api:suggest-task-labels-path:v1",
        "urn:kanban-tool:schema:api:suggest-task-labels-response:v1",
        "urn:kanban-tool:schema:api:task-neighborhood-path:v1",
        "urn:kanban-tool:schema:api:task-neighborhood-query:v1",
        "urn:kanban-tool:schema:api:task-neighborhood-response:v1",
        "urn:kanban-tool:schema:api:unblock-task-path:v1",
        "urn:kanban-tool:schema:api:unblock-task-request:v1",
        "urn:kanban-tool:schema:api:unblock-task-response:v1",
        "urn:kanban-tool:schema:api:update-step-path:v1",
        "urn:kanban-tool:schema:api:update-step-request:v1",
        "urn:kanban-tool:schema:api:update-step-response:v1",
        "urn:kanban-tool:schema:api:upsert-label-semantics-path:v1",
        "urn:kanban-tool:schema:api:upsert-label-semantics-request:v1",
        "urn:kanban-tool:schema:api:upsert-label-semantics-response:v1",
        "urn:kanban-tool:schema:api:validate-label-ontology-action-path:v1",
        "urn:kanban-tool:schema:api:validate-label-ontology-action-request:v1",
        "urn:kanban-tool:schema:api:validate-label-ontology-action-response:v1",
        "urn:kanban-tool:schema:metadata:decision:v1",
        "urn:kanban-tool:schema:metadata:label-proposal-candidate-input:v1",
        "urn:kanban-tool:schema:metadata:ontology-record-input:v1",
        "urn:kanban-tool:schema:metadata:ontology-validation-evidence-input:v1",
        "urn:kanban-tool:schema:metadata:signal-link-output:v1",
        "urn:kanban-tool:schema:metadata:signal-record-input:v1",
        "urn:kanban-tool:schema:sse:stream-event-data:v1",
        "urn:kanban-tool:schema:api:get-stats-query:v1",
        "urn:kanban-tool:schema:api:get-stats-response:v1",
        "urn:kanban-tool:schema:api:search-tasks-query:v1",
        "urn:kanban-tool:schema:api:search-tasks-response:v1",
        "urn:kanban-tool:schema:api:search-tasks-by-status-query:v1",
        "urn:kanban-tool:schema:api:search-tasks-by-status-response:v1",
        "urn:kanban-tool:schema:api:search-status-query:v1",
        "urn:kanban-tool:schema:api:search-status-response:v1",
        "urn:kanban-tool:schema:api:build-context-path:v1",
        "urn:kanban-tool:schema:api:build-context-query:v1",
        "urn:kanban-tool:schema:api:build-context-response:v1",
        "urn:kanban-tool:schema:api:entity-list-query:v1",
        "urn:kanban-tool:schema:api:entity-list-response:v1",
        "urn:kanban-tool:schema:api:entity-path:v1",
        "urn:kanban-tool:schema:api:entity-response:v1",
        "urn:kanban-tool:schema:api:entity-upsert-request:v1",
        "urn:kanban-tool:schema:api:entity-upsert-response:v1",
        "urn:kanban-tool:schema:api:graph-status-query:v1",
        "urn:kanban-tool:schema:api:graph-status-response:v1",
        "urn:kanban-tool:schema:api:graph-neighbors-query:v1",
        "urn:kanban-tool:schema:api:graph-neighbors-response:v1",
        "urn:kanban-tool:schema:api:graph-query-query:v1",
        "urn:kanban-tool:schema:api:graph-query-response:v1",
        "urn:kanban-tool:schema:api:graph-rebuild-query:v1",
        "urn:kanban-tool:schema:api:graph-rebuild-response:v1",
        "urn:kanban-tool:schema:api:graph-sync-query:v1",
        "urn:kanban-tool:schema:api:graph-sync-response:v1",
        "urn:kanban-tool:schema:api:vector-status-query:v1",
        "urn:kanban-tool:schema:api:vector-status-response:v1",
        "urn:kanban-tool:schema:api:list-events-query:v1",
        "urn:kanban-tool:schema:sse:stream-events-query:v1",
    ]);
    expected.extend(
        kanban_contract::portable_contract_catalog()
            .iter()
            .flat_map(|descriptor| [descriptor.input.schema_id, descriptor.output.schema_id]),
    );
    expected.extend(
        operation_inventory()
            .iter()
            .filter(|contract| {
                matches!(
                    contract.surface,
                    ContractSurface::Config | ContractSurface::Helper
                )
            })
            .map(|contract| contract.schema_id.expect("adopted protocol schema id")),
    );

    assert_eq!(
        actual, expected,
        "foundation root 必须来自 schema generation DTO registry"
    );
}

#[test]
fn generated_schema_artifacts_are_non_empty_and_deterministic() {
    let first = generated_artifacts();
    let second = generated_artifacts();

    assert!(
        !first.is_empty(),
        "schema registry 必须生成 committed artifact"
    );
    assert_eq!(first, second, "同一 registry 连续生成必须 byte-identical");
}

#[test]
fn decision_wire_type_preserves_existing_extension_fields() {
    let value = serde_json::json!({
        "options": [{
            "slug": "typed-open",
            "title": "Typed open contract",
            "detail": "Keep known fields typed and preserve extensions.",
            "owner": "adapter"
        }],
        "selected": "typed-open",
        "reason": "当前 service validator 允许未知字段。",
        "ticket": "default#123"
    });

    let contract: kanban_contract::DecisionMetadata =
        serde_json::from_value(value.clone()).expect("decision contract should deserialize");

    assert_eq!(
        serde_json::to_value(contract).expect("decision contract should serialize"),
        value
    );
}

#[test]
fn decision_optional_strings_accept_missing_but_reject_explicit_null() {
    let missing = serde_json::json!({
        "options": [{
            "slug": "typed-open",
            "title": "Typed open contract",
            "detail": "Keep known fields typed and preserve extensions."
        }],
        "selected": "typed-open",
        "reason": "missing optional values remain valid"
    });
    serde_json::from_value::<kanban_contract::DecisionMetadata>(missing)
        .expect("missing risk/verification must remain valid");

    for field in ["risk", "verification"] {
        let mut explicit_null = serde_json::json!({
            "options": [{
                "slug": "typed-open",
                "title": "Typed open contract",
                "detail": "Keep known fields typed and preserve extensions."
            }],
            "selected": "typed-open",
            "reason": "explicit null is not the same as missing"
        });
        explicit_null
            .as_object_mut()
            .expect("fixture is an object")
            .insert(field.to_owned(), serde_json::Value::Null);

        serde_json::from_value::<kanban_contract::DecisionMetadata>(explicit_null)
            .expect_err("explicit null must be rejected by the real Serde DTO");
    }
}

#[test]
fn endpoint_descriptor_catalog_is_complete_and_explicit() {
    let endpoints = endpoint_catalog();
    assert_eq!(
        endpoints.len(),
        105,
        "104 JSON API + 1 SSE 必须全部有 descriptor"
    );
    assert_eq!(
        endpoints
            .iter()
            .filter(|endpoint| endpoint.surface == ContractSurface::Sse)
            .count(),
        1,
        "SSE 必须作为独立 transport descriptor"
    );
    assert!(endpoints.iter().all(|endpoint| {
        !endpoint.operation_id.is_empty()
            && !endpoint.path.is_empty()
            && endpoint.obligations.entries().len() == 6
    }));
}

#[test]
fn endpoint_descriptor_validator_rejects_duplicate_operation_and_method_path() {
    let baseline = endpoint_catalog()[0];
    let duplicate_operation = [
        baseline,
        EndpointDescriptor {
            path: "/other",
            ..baseline
        },
    ];
    assert!(validate_endpoint_catalog(&duplicate_operation, false).is_err());
    let duplicate_method_path = [
        baseline,
        EndpointDescriptor {
            operation_id: "api.test-other",
            ..baseline
        },
    ];
    assert!(validate_endpoint_catalog(&duplicate_method_path, false).is_err());
    let wrong_surface = [EndpointDescriptor {
        surface: ContractSurface::Cli,
        operation_id: "api.test-cli",
        method: HttpMethod::Get,
        path: "/test",
        migration: MigrationState::Planned,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Todo,
            query: EndpointObligation::Todo,
            headers: EndpointObligation::Todo,
            body: EndpointObligation::Todo,
            success: EndpointObligation::Todo,
            sse: EndpointObligation::NotApplicable,
        },
    }];
    assert!(validate_endpoint_catalog(&wrong_surface, false).is_err());
}

#[test]
fn endpoint_closure_rejects_residual_todo_and_adopted_todo() {
    assert!(kanban_contract::validate_endpoint_catalog(endpoint_catalog(), true).is_ok());
    let baseline = endpoint_catalog()[0];
    let adopted_with_todo = [EndpointDescriptor {
        migration: MigrationState::Adopted,
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Todo,
            headers: EndpointObligation::NotApplicable,
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.health.response"),
            sse: EndpointObligation::NotApplicable,
        },
        ..baseline
    }];
    assert!(kanban_contract::validate_endpoint_catalog(&adopted_with_todo, false).is_err());
}

#[test]
fn endpoint_obligation_validator_rejects_empty_exclusion_and_contract_identity_drift() {
    let baseline = endpoint_catalog()[0];
    let empty_exclusion = [EndpointDescriptor {
        obligations: EndpointObligations {
            query: EndpointObligation::Excluded { reason: "  " },
            ..baseline.obligations
        },
        ..baseline
    }];
    assert!(validate_endpoint_catalog(&empty_exclusion, false).is_err());

    let unknown_contract = [EndpointDescriptor {
        obligations: EndpointObligations {
            query: EndpointObligation::Contract("api.unknown.input"),
            ..baseline.obligations
        },
        ..baseline
    }];
    assert!(validate_endpoint_catalog(&unknown_contract, false).is_err());

    let wrong_surface = [EndpointDescriptor {
        obligations: EndpointObligations {
            query: EndpointObligation::Contract("metadata.decision.input"),
            ..baseline.obligations
        },
        ..baseline
    }];
    assert!(validate_endpoint_catalog(&wrong_surface, false).is_err());

    let wrong_direction = [EndpointDescriptor {
        obligations: EndpointObligations {
            query: EndpointObligation::Contract("api.health.response"),
            ..baseline.obligations
        },
        ..baseline
    }];
    assert!(validate_endpoint_catalog(&wrong_direction, false).is_err());
}

const MISSING_QUERY_CARDINALITY: &[WireParameter] = &[WireParameter {
    name: "status",
    cardinality: None,
}];
const OPTIONAL_PATH_CARDINALITY: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
const REQUIRED_PATH_CARDINALITY: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const OPTIONAL_QUERY_CARDINALITY: &[WireParameter] = &[WireParameter {
    name: "status",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
const DUPLICATE_QUERY_CARDINALITY: &[WireParameter] = &[
    WireParameter {
        name: "status",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "status",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
];
const ORDERED_REPEATED_QUERY: &[WireParameter] = &[WireParameter {
    name: "status",
    cardinality: Some(WireParameterCardinality::RepeatedOrdered),
}];

fn contract_mut<'a>(
    inventory: &'a mut [OperationContract],
    contract_id: &str,
) -> &'a mut OperationContract {
    inventory
        .iter_mut()
        .find(|contract| contract.id == contract_id)
        .unwrap_or_else(|| panic!("missing test contract {contract_id}"))
}

fn claim_endpoint_with_query_contract() -> EndpointDescriptor {
    let mut endpoint = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "api.claim-task")
        .expect("claim endpoint descriptor");
    endpoint.obligations.body = EndpointObligation::Todo;
    endpoint.obligations.query = EndpointObligation::Contract("api.claim-task.request");
    endpoint.migration = MigrationState::Generated;
    endpoint
}

#[test]
fn operation_inventory_declares_http_or_explicit_no_transport() {
    for contract in operation_inventory() {
        match contract.surface {
            ContractSurface::Api | ContractSurface::Sse => assert!(
                matches!(contract.transport, ContractTransport::Http { .. }),
                "{} 必须显式声明 HTTP transport",
                contract.id
            ),
            _ => assert_eq!(
                contract.transport,
                ContractTransport::NoTransport,
                "{} 必须显式声明 no_transport",
                contract.id
            ),
        }
    }

    let mut missing_http = operation_inventory().to_vec();
    contract_mut(&mut missing_http, "api.health.response").transport =
        ContractTransport::NoTransport;
    let error = validate_contract_topology(&[], &missing_http, false)
        .expect_err("API contract 缺 HTTP transport 必须失败");
    assert!(error.contains("must declare transport metadata"), "{error}");

    let mut false_http = operation_inventory().to_vec();
    contract_mut(&mut false_http, "metadata.decision.input").transport = ContractTransport::Http {
        operation_key: Some("metadata decision"),
        location: HttpTransportLocation::Body,
        parameters: &[],
    };
    let error = validate_contract_topology(&[], &false_http, false)
        .expect_err("非 HTTP contract 伪装 HTTP transport 必须失败");
    assert!(error.contains("must declare no_transport"), "{error}");

    let mut missing_operation = operation_inventory().to_vec();
    contract_mut(&mut missing_operation, "api.health.response").transport =
        ContractTransport::Http {
            operation_key: None,
            location: HttpTransportLocation::Success,
            parameters: &[],
        };
    let error = validate_contract_topology(&[], &missing_operation, false)
        .expect_err("ExactSurface HTTP contract 缺 operation_key 必须失败");
    assert!(error.contains("must name an operation_key"), "{error}");
}

#[test]
fn topology_rejects_body_contract_in_query_with_location_diagnostic() {
    let endpoint = claim_endpoint_with_query_contract();
    let error = validate_contract_topology(&[endpoint], operation_inventory(), false)
        .expect_err("body contract 放入 query obligation 必须失败");
    assert!(
        error.contains("contract location body does not match obligation query"),
        "{error}"
    );
}

#[test]
fn topology_rejects_success_deserialize_and_surface_location_drift() {
    let health = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "api.health")
        .expect("health endpoint descriptor");

    let mut wrong_direction = operation_inventory().to_vec();
    contract_mut(&mut wrong_direction, "api.health.response").direction =
        ContractDirection::Deserialize;
    let error = validate_contract_topology(&[health], &wrong_direction, false)
        .expect_err("success contract 指向 Deserialize 必须失败");
    assert!(
        error.contains("transport direction does not match location success"),
        "{error}"
    );

    let mut wrong_location = operation_inventory().to_vec();
    contract_mut(&mut wrong_location, "api.health.response").transport = ContractTransport::Http {
        operation_key: Some("GET /health"),
        location: HttpTransportLocation::Sse,
        parameters: &[],
    };
    let error = validate_contract_topology(&[health], &wrong_location, false)
        .expect_err("API contract 声明 SSE location 必须失败");
    assert!(
        error.contains("transport location sse is incompatible with api surface"),
        "{error}"
    );
}

#[test]
fn topology_rejects_operation_drift_and_exact_as_shared_conflict() {
    let claim = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "api.claim-task")
        .expect("claim endpoint descriptor");
    let mut wrong_operation = operation_inventory().to_vec();
    contract_mut(&mut wrong_operation, "api.claim-task.request").transport =
        ContractTransport::Http {
            operation_key: Some("POST /api/v1/tasks/:task_id/transitions/other"),
            location: HttpTransportLocation::Body,
            parameters: &[],
        };
    let error = validate_contract_topology(&[claim], &wrong_operation, false)
        .expect_err("exact contract operation 漂移必须失败");
    assert!(
        error.contains("contract operation does not match endpoint"),
        "{error}"
    );

    let mut duplicated = claim;
    duplicated.shared_components = &["api.claim-task.request"];
    let error = validate_contract_topology(&[duplicated], operation_inventory(), false)
        .expect_err("ExactSurface 不得进入 shared_components");
    assert!(
        error.contains("shared component link requires SharedComponent contract"),
        "{error}"
    );
}

#[test]
fn topology_rejects_missing_wrong_and_conflicting_parameter_cardinality() {
    let endpoint = claim_endpoint_with_query_contract();

    let mut missing = operation_inventory().to_vec();
    contract_mut(&mut missing, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Query,
        parameters: MISSING_QUERY_CARDINALITY,
    };
    let error = validate_contract_topology(&[endpoint], &missing, false)
        .expect_err("query parameter 缺 cardinality 必须失败");
    assert!(
        error.contains("wire parameter missing cardinality"),
        "{error}"
    );

    let mut path_endpoint = endpoint;
    path_endpoint.obligations.query = EndpointObligation::Todo;
    path_endpoint.obligations.path = EndpointObligation::Contract("api.claim-task.request");
    let mut wrong = operation_inventory().to_vec();
    contract_mut(&mut wrong, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Path,
        parameters: OPTIONAL_PATH_CARDINALITY,
    };
    let error = validate_contract_topology(&[path_endpoint], &wrong, false)
        .expect_err("path parameter 非 RequiredOne 必须失败");
    assert!(
        error.contains("path parameter cardinality must be required_one"),
        "{error}"
    );

    let mut required = operation_inventory().to_vec();
    contract_mut(&mut required, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Path,
        parameters: REQUIRED_PATH_CARDINALITY,
    };
    validate_contract_topology(&[path_endpoint], &required, false)
        .expect("path 的 required_one cardinality 必须精确匹配 placeholder");

    let mut optional = operation_inventory().to_vec();
    contract_mut(&mut optional, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Query,
        parameters: OPTIONAL_QUERY_CARDINALITY,
    };
    validate_contract_topology(&[endpoint], &optional, false)
        .expect("query 的 optional_one cardinality 必须可精确表达");

    let mut conflicting = operation_inventory().to_vec();
    contract_mut(&mut conflicting, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Query,
        parameters: DUPLICATE_QUERY_CARDINALITY,
    };
    let error = validate_contract_topology(&[endpoint], &conflicting, false)
        .expect_err("同一 component 重复 parameter name 必须失败");
    assert!(error.contains("wire parameter name conflict"), "{error}");

    let mut repeated = operation_inventory().to_vec();
    contract_mut(&mut repeated, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Query,
        parameters: ORDERED_REPEATED_QUERY,
    };
    validate_contract_topology(&[endpoint], &repeated, false)
        .expect("query 的 repeated_ordered cardinality 必须可精确表达");
}

const SHARED_API_ERROR: &[&str] = &["api.error.response"];
const UNKNOWN_SHARED: &[&str] = &["api.unknown.response"];

#[test]
fn topology_allows_shared_reuse_but_rejects_invalid_shared_references() {
    let list_tasks = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "api.list-tasks")
        .expect("list tasks endpoint descriptor");
    let mut list_boards = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "api.list-boards")
        .expect("list boards endpoint descriptor");
    list_boards.shared_components = SHARED_API_ERROR;
    validate_contract_topology(&[list_tasks, list_boards], operation_inventory(), false)
        .expect("同一个 SharedComponent 必须允许被多个 endpoint 显式复用");

    let mut unknown = list_boards;
    unknown.shared_components = UNKNOWN_SHARED;
    let error = validate_contract_topology(&[unknown], operation_inventory(), false)
        .expect_err("unknown shared reference 必须失败");
    assert!(error.contains("references unknown contract"), "{error}");

    for state in [MigrationState::Planned, MigrationState::Excluded] {
        let mut inventory = operation_inventory().to_vec();
        contract_mut(&mut inventory, "api.error.response").migration = state;
        let error = validate_contract_topology(&[list_boards], &inventory, false)
            .expect_err("Planned/Excluded shared reference 必须失败");
        assert!(
            error.contains(match state {
                MigrationState::Planned => "references planned contract",
                MigrationState::Excluded => "references excluded contract",
                _ => unreachable!(),
            }),
            "{error}"
        );
    }
}

#[test]
fn topology_rejects_unknown_planned_and_excluded_contract_references() {
    let mut sse = *endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == "sse.stream-events")
        .expect("SSE endpoint descriptor");

    sse.obligations.sse = EndpointObligation::Contract("sse.unknown.data");
    let error = validate_contract_topology(&[sse], operation_inventory(), false)
        .expect_err("unknown contract reference 必须失败");
    assert!(error.contains("references unknown contract"), "{error}");

    sse.obligations.sse = EndpointObligation::Contract("sse.event.data");
    let mut planned = operation_inventory().to_vec();
    let contract = contract_mut(&mut planned, "sse.event.data");
    contract.migration = MigrationState::Planned;
    contract.schema_id = None;
    contract.fixture = None;
    contract.adoption = None;
    let error = validate_contract_topology(&[sse], &planned, false)
        .expect_err("Planned contract reference 必须失败");
    assert!(error.contains("references planned contract"), "{error}");

    let mut excluded = operation_inventory().to_vec();
    let contract = contract_mut(&mut excluded, "sse.event.data");
    contract.migration = MigrationState::Excluded;
    contract.exclusion = Some("test-only exclusion");
    let error = validate_contract_topology(&[sse], &excluded, false)
        .expect_err("Excluded contract reference 必须失败");
    assert!(error.contains("references excluded contract"), "{error}");
}

const WRONG_PATH_NAME: &[WireParameter] = &[WireParameter {
    name: "task",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const EXTRA_PATH_NAME: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "extra",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];
const CASE_DRIFT_PATH_NAME: &[WireParameter] = &[WireParameter {
    name: "TASK_ID",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const REVERSED_LABEL_PATH_NAMES: &[WireParameter] = &[
    WireParameter {
        name: "label_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];
const HEADER_CASE_CONFLICT: &[WireParameter] = &[
    WireParameter {
        name: "X-KB-Actor",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "x-kb-actor",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];
const EMPTY_PARAMETER_NAME: &[WireParameter] = &[WireParameter {
    name: "",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
const WHITESPACE_PARAMETER_NAME: &[WireParameter] = &[WireParameter {
    name: " status ",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
const FORBIDDEN_WIRE_PARAMETER: &[WireParameter] = &[WireParameter {
    name: "unexpected",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const SHARED_HEADER_PARAMETER: &[WireParameter] = &[WireParameter {
    name: "X-Request-Id",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
const SHARED_HEADER_WITHOUT_CARDINALITY: &[WireParameter] = &[WireParameter {
    name: "X-Request-Id",
    cardinality: None,
}];
const DUPLICATE_SHARED_API_ERROR: &[&str] = &["api.error.response", "api.error.response"];

fn assert_diagnostic(error: &str, expected: &[&str]) {
    for token in expected {
        assert!(error.contains(token), "diagnostic 缺少 {token:?}: {error}");
    }
}

fn claim_endpoint_with_path_contract() -> EndpointDescriptor {
    let mut endpoint = *endpoint_descriptor("api.claim-task").expect("claim endpoint descriptor");
    endpoint.obligations.body = EndpointObligation::Todo;
    endpoint.obligations.path = EndpointObligation::Contract("api.claim-task.request");
    endpoint
}

fn claim_path_mapping_error(parameters: &'static [WireParameter]) -> String {
    let endpoint = claim_endpoint_with_path_contract();
    let mut inventory = operation_inventory().to_vec();
    contract_mut(&mut inventory, "api.claim-task.request").transport = ContractTransport::Http {
        operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        location: HttpTransportLocation::Path,
        parameters,
    };
    validate_contract_topology(&[endpoint], &inventory, false)
        .expect_err("path placeholder 漂移必须失败")
}

#[test]
fn b1_c1_task_read_schema_and_runtime_budgets_are_exact() {
    assert_eq!(kanban_contract::MAX_TASK_READ_QUERY_BYTES, 8_192);
    assert_eq!(kanban_contract::MAX_TASK_READ_QUERY_PAIRS, 54);
    assert_eq!(kanban_contract::MAX_TASK_READ_STATUSES, 9);
    assert_eq!(kanban_contract::MAX_TASK_READ_PRIORITIES, 4);
    assert_eq!(kanban_contract::MAX_TASK_READ_PLAN_FILTERS, 3);
    assert_eq!(kanban_contract::MAX_TASK_READ_LABELS, 32);
    assert_eq!(kanban_contract::MAX_TASK_READ_Q_CHARS, 1_024);
    assert_eq!(kanban_contract::MAX_TASK_READ_ASSIGNEE_CHARS, 128);
    assert_eq!(kanban_contract::MAX_TASK_READ_LABEL_CHARS, 128);
    assert_eq!(kanban_contract::MAX_TASK_READ_LIMIT, 1_000);

    let artifacts = generated_artifacts();
    for artifact in [
        "api/list-tasks-query.v1.schema.json",
        "api/list-tasks-by-status-query.v1.schema.json",
    ] {
        let schema: serde_json::Value =
            serde_json::from_slice(artifacts.get(artifact).expect("task-read schema artifact"))
                .expect("valid generated schema");
        let properties = &schema["properties"];
        for (field, maximum) in [
            ("status", 9),
            ("priority", 4),
            ("plan_filter", 3),
            ("label", 32),
        ] {
            assert_eq!(properties[field]["maxItems"], maximum, "{artifact}:{field}");
            assert_eq!(properties[field]["uniqueItems"], true, "{artifact}:{field}");
        }
        assert_eq!(properties["q"]["maxLength"], 1_024, "{artifact}:q");
        assert_eq!(
            properties["assignee"]["maxLength"], 128,
            "{artifact}:assignee"
        );
        assert_eq!(properties["limit"]["maximum"], 1_000, "{artifact}:limit");
        assert_eq!(schema["$defs"]["TaskReadLabel"]["minLength"], 1);
        assert_eq!(schema["$defs"]["TaskReadLabel"]["maxLength"], 128);
        assert_eq!(
            schema["$defs"]["TaskReadLabel"]["pattern"],
            r"\P{White_Space}"
        );
    }
}

#[test]
fn b1_c1_task_read_label_normalizes_edges_and_rejects_unicode_whitespace() {
    let normalized = kanban_contract::TaskReadLabel::new("\u{3000}后端 API\u{2003}")
        .expect("含非空白字符的 label 应可规范化");
    assert_eq!(normalized.as_str(), "后端 API");

    let body_at_raw_limit = "界".repeat(126);
    let raw_at_limit = format!("\u{3000}{body_at_raw_limit}\u{2003}");
    assert_eq!(
        raw_at_limit.chars().count(),
        kanban_contract::MAX_TASK_READ_LABEL_CHARS
    );
    let constructed = kanban_contract::TaskReadLabel::new(raw_at_limit.clone())
        .expect("raw 128 字符必须接受并移除 Unicode 边缘空白");
    assert_eq!(constructed.as_str(), body_at_raw_limit.as_str());
    let encoded = serde_json::to_string(&raw_at_limit).expect("encode raw-limit label");
    let deserialized = serde_json::from_str::<kanban_contract::TaskReadLabel>(&encoded)
        .expect("Serde 必须接受 raw 128 字符并规范化");
    assert_eq!(deserialized.as_str(), body_at_raw_limit.as_str());

    let raw_over_limit = format!("\u{3000}{}\u{2003}", "界".repeat(127));
    assert_eq!(
        raw_over_limit.chars().count(),
        kanban_contract::MAX_TASK_READ_LABEL_CHARS + 1
    );
    assert!(
        kanban_contract::TaskReadLabel::new(raw_over_limit.clone()).is_none(),
        "随后会被 trim 的 Unicode 边缘空白也必须计入 raw 字符预算"
    );
    let encoded = serde_json::to_string(&raw_over_limit).expect("encode over-limit label");
    assert!(
        serde_json::from_str::<kanban_contract::TaskReadLabel>(&encoded).is_err(),
        "Serde 必须拒绝 raw 129 字符，即使 trim 后正文未超限"
    );

    for whitespace in [" ", "\t", "\n", "\u{00a0}", "\u{2003}", "\u{3000}"] {
        assert!(
            kanban_contract::TaskReadLabel::new(whitespace).is_none(),
            "纯 Unicode 空白 label 必须被构造器拒绝: {whitespace:?}"
        );
        let encoded = serde_json::to_string(whitespace).expect("encode whitespace fixture");
        assert!(
            serde_json::from_str::<kanban_contract::TaskReadLabel>(&encoded).is_err(),
            "纯 Unicode 空白 label 必须被 deserialize 拒绝: {whitespace:?}"
        );
    }
}

#[test]
fn b1_c1_task_read_contracts_are_endpoint_specific_and_exact() {
    const REPEATED: WireParameterCardinality = WireParameterCardinality::RepeatedOrdered;
    const OPTIONAL: WireParameterCardinality = WireParameterCardinality::OptionalOne;
    let expected_query_parameters = [
        ("status", REPEATED),
        ("priority", REPEATED),
        ("label", REPEATED),
        ("plan_filter", REPEATED),
        ("assignee", OPTIONAL),
        ("q", OPTIONAL),
        ("include_archived", OPTIONAL),
        ("limit", OPTIONAL),
        ("offset", OPTIONAL),
        ("sort", OPTIONAL),
    ];

    for (operation_id, path_contract_id, query_contract_id, response_contract_id, operation_key) in [
        (
            "api.list-tasks",
            "api.list-tasks.path",
            "api.list-tasks.query",
            "api.list-tasks.response",
            "GET /api/v1/boards/:board/tasks",
        ),
        (
            "api.list-tasks-by-status",
            "api.list-tasks-by-status.path",
            "api.list-tasks-by-status.query",
            "api.list-tasks-by-status.response",
            "GET /api/v1/boards/:board/tasks/by-status",
        ),
    ] {
        let endpoint = endpoint_descriptor(operation_id).expect("task-read endpoint descriptor");
        assert_eq!(endpoint.migration, MigrationState::Adopted);
        assert_eq!(
            endpoint.obligations.path,
            EndpointObligation::Contract(path_contract_id)
        );
        assert_eq!(
            endpoint.obligations.query,
            EndpointObligation::Contract(query_contract_id)
        );
        assert_eq!(
            endpoint.obligations.headers,
            EndpointObligation::Contract(Box::leak(
                format!("{operation_id}.headers").into_boxed_str()
            ))
        );
        assert_eq!(endpoint.obligations.body, EndpointObligation::NotApplicable);
        assert_eq!(
            endpoint.obligations.success,
            EndpointObligation::Contract(response_contract_id)
        );

        let path_contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == path_contract_id)
            .expect("endpoint-specific path contract");
        assert_eq!(path_contract.migration, MigrationState::Adopted);
        assert_eq!(path_contract.granularity, ContractGranularity::Exact);
        assert_eq!(path_contract.binding, ContractBinding::ExactSurface);
        let ContractTransport::Http {
            operation_key: actual_operation,
            location,
            parameters,
        } = path_contract.transport
        else {
            panic!("task-read path contract must declare HTTP transport");
        };
        assert_eq!(actual_operation, Some(operation_key));
        assert_eq!(location, HttpTransportLocation::Path);
        assert_eq!(
            parameters,
            &[WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne),
            }]
        );

        let query_contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == query_contract_id)
            .expect("endpoint-specific query contract");
        assert_eq!(query_contract.migration, MigrationState::Adopted);
        assert_eq!(query_contract.granularity, ContractGranularity::Exact);
        assert_eq!(query_contract.binding, ContractBinding::ExactSurface);
        let ContractTransport::Http {
            operation_key: actual_operation,
            location,
            parameters,
        } = query_contract.transport
        else {
            panic!("task-read query contract must declare HTTP transport");
        };
        assert_eq!(actual_operation, Some(operation_key));
        assert_eq!(location, HttpTransportLocation::Query);
        assert_eq!(
            parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.name,
                        parameter.cardinality.expect("explicit cardinality"),
                    )
                })
                .collect::<Vec<_>>(),
            expected_query_parameters
        );

        let response_contract = operation_inventory()
            .iter()
            .find(|contract| contract.id == response_contract_id)
            .expect("endpoint-specific exact success response contract");
        assert_eq!(response_contract.migration, MigrationState::Adopted);
        assert_eq!(response_contract.direction, ContractDirection::Serialize);
        assert_eq!(response_contract.granularity, ContractGranularity::Exact);
        assert_eq!(response_contract.binding, ContractBinding::ExactSurface);
        assert_eq!(
            response_contract.strictness,
            kanban_contract::ContractStrictness::DenyUnknownFields
        );
        assert!(response_contract.schema_id.is_some());
        assert!(response_contract.fixture.is_some());
        assert!(response_contract.adoption.is_some());
        let ContractTransport::Http {
            operation_key: actual_operation,
            location,
            parameters,
        } = response_contract.transport
        else {
            panic!("task-read response contract must declare HTTP transport");
        };
        assert_eq!(actual_operation, Some(operation_key));
        assert_eq!(location, HttpTransportLocation::Success);
        assert!(parameters.is_empty());
    }
}

#[test]
fn current_train_freeze_requires_closed_authority() {
    let stream = endpoint_descriptor("sse.stream-events").expect("SSE endpoint descriptor");
    assert_eq!(
        stream.obligations.sse,
        EndpointObligation::Contract("sse.event.data")
    );
    assert_eq!(endpoint_obligation_todo_count(endpoint_catalog()), 0);
    let mut contract = 0;
    let mut todo = 0;
    let mut not_applicable = 0;
    let mut excluded = 0;
    for obligation in endpoint_catalog()
        .iter()
        .flat_map(|endpoint| endpoint.obligations.entries())
        .map(|(_, obligation)| obligation)
    {
        match obligation {
            EndpointObligation::Contract(_) => contract += 1,
            EndpointObligation::Todo => todo += 1,
            EndpointObligation::NotApplicable => not_applicable += 1,
            EndpointObligation::Excluded { .. } => excluded += 1,
        }
    }
    assert_eq!((contract, todo, not_applicable, excluded), (363, 0, 266, 1));
    let unfinished_contracts = operation_inventory()
        .iter()
        .filter(|contract| {
            matches!(
                contract.migration,
                MigrationState::Planned | MigrationState::Generated
            )
        })
        .count();
    let unfinished_surfaces = surface_operation_catalog()
        .iter()
        .filter(|surface| {
            matches!(
                surface.migration,
                MigrationState::Planned | MigrationState::Generated
            )
        })
        .count();
    assert_eq!((unfinished_contracts, unfinished_surfaces, todo), (0, 0, 0));
    assert_eq!(unfinished_contracts + unfinished_surfaces + todo, 0);

    let generated_api = operation_inventory()
        .iter()
        .filter(|contract| {
            contract.surface == ContractSurface::Api
                && contract.migration == MigrationState::Generated
        })
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();
    assert!(generated_api.is_empty());
    assert!(
        endpoint_catalog()
            .iter()
            .filter(|endpoint| endpoint.surface == ContractSurface::Api)
            .all(|endpoint| endpoint.migration == MigrationState::Adopted)
    );
}

#[test]
fn config_and_helper_protocols_have_exact_roots_surfaces_and_witnesses() {
    let expected = [
        "config.project.input",
        "config.selected-worker-profile.input",
        "helper.vector.handshake.response",
        "helper.vector.error.response",
        "helper.vector.check-provider.response",
        "helper.vector.status.response",
        "helper.vector.rebuild.response",
        "helper.vector.sync.response",
        "helper.vector.label-atoms-status.response",
        "helper.vector.rebuild-label-atoms.response",
        "helper.vector.sync-label-atoms.response",
        "helper.vector.query-chunks.response",
        "helper.vector.query-label-atoms.response",
        "helper.vector.embed-query.response",
    ];
    let inventory = operation_inventory();
    let roots = kanban_contract::schema_registry();
    let surfaces = surface_operation_catalog();

    for contract_id in expected {
        let contract = inventory
            .iter()
            .find(|contract| contract.id == contract_id)
            .unwrap_or_else(|| panic!("missing protocol contract {contract_id}"));
        assert_eq!(contract.migration, MigrationState::Adopted, "{contract_id}");
        assert!(contract.schema_id.is_some(), "{contract_id}");
        assert!(contract.fixture.is_some(), "{contract_id}");
        assert!(contract.adoption.is_some(), "{contract_id}");
        assert!(matches!(contract.transport, ContractTransport::NoTransport));
        assert_eq!(
            roots
                .iter()
                .filter(|root| root.contract_id == contract_id)
                .count(),
            1,
            "{contract_id}"
        );
        assert_eq!(
            surfaces
                .iter()
                .filter(|surface| {
                    surface.migration == MigrationState::Adopted
                        && surface.contracts == [contract_id]
                        && surface.key == contract.operation
                })
                .count(),
            1,
            "{contract_id}"
        );
    }
}

#[test]
fn vector_projection_protocol_has_two_exact_roots_and_four_runtime_witnesses() {
    let contract_ids = [
        "helper.vector-projection.request",
        "helper.vector-projection.response",
    ];
    let inventory = operation_inventory();
    let roots = kanban_contract::schema_registry();
    let surfaces = surface_operation_catalog();

    for contract_id in contract_ids {
        let contract = inventory
            .iter()
            .find(|contract| contract.id == contract_id)
            .unwrap_or_else(|| panic!("missing vector projection contract {contract_id}"));
        assert_eq!(contract.migration, MigrationState::Adopted);
        assert!(contract.schema_id.is_some());
        assert!(contract.fixture.is_some());
        let adoption = contract.adoption.expect("runtime adoption witnesses");
        assert_eq!(adoption.producer.package, "kanban-vector-lancedb");
        assert_eq!(adoption.consumer.package, "kanban-vector-lancedb");
        assert_eq!(
            adoption.producer.test_target,
            "vector_projection_contract_adoption"
        );
        assert_eq!(
            adoption.consumer.test_target,
            "vector_projection_contract_adoption"
        );
        assert_eq!(
            roots
                .iter()
                .filter(|root| root.contract_id == contract_id)
                .count(),
            1
        );
    }

    let surfaces = surfaces
        .iter()
        .filter(|surface| surface.key == "vector projection helper protocol")
        .collect::<Vec<_>>();
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].migration, MigrationState::Adopted);
    assert_eq!(surfaces[0].surface, ContractSurface::Helper);
    assert_eq!(surfaces[0].contracts, contract_ids);
}

#[test]
fn selected_worker_profile_contract_matches_runtime_selection_boundary() {
    let inventory = operation_inventory();
    assert!(
        inventory
            .iter()
            .all(|contract| contract.id != "config.worker-profiles.input"),
        "whole-document worker profile contract must not survive selected-only runtime adoption"
    );

    let contract = inventory
        .iter()
        .find(|contract| contract.id == "config.selected-worker-profile.input")
        .expect("selected worker profile contract");
    assert_eq!(contract.path, "selected [workers.<profile>] section");
    assert_eq!(
        contract.operation,
        "selected dispatcher worker profile after TOML decoding"
    );
    assert_eq!(contract.strictness, ContractStrictness::DenyUnknownFields);
    let adoption = contract
        .adoption
        .expect("selected profile adoption witness");
    assert_eq!(
        adoption.producer.exact_test,
        "tests::selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto"
    );
    assert_eq!(
        adoption.consumer.exact_test,
        "tests::selected_worker_profile_input_fixture_is_consumed_by_real_toml_decoder"
    );

    let root = kanban_contract::schema_registry()
        .iter()
        .find(|root| root.contract_id == contract.id)
        .expect("selected worker profile schema root");
    assert_eq!(
        root.id,
        "urn:kanban-tool:schema:config:selected-worker-profile-input:v1"
    );
    assert_eq!(
        root.valid_fixture,
        "schemas/fixtures/config/selected-worker-profile-input.v1.valid.json"
    );
}

#[test]
fn error_transport_is_serialize_shared_only_and_has_no_endpoint_obligation() {
    let error_contract = operation_inventory()
        .iter()
        .find(|contract| contract.id == "api.error.response")
        .expect("API error contract");
    assert!(matches!(
        error_contract.transport,
        ContractTransport::Http {
            location: HttpTransportLocation::Error,
            ..
        }
    ));
    assert_eq!(error_contract.binding, ContractBinding::SharedComponent);
    assert!(
        endpoint_catalog()
            .iter()
            .all(|endpoint| endpoint.obligations.entries().len() == 6),
        "Error 只是 shared transport location，不新增第七 endpoint obligation"
    );

    let mut wrong_direction = operation_inventory().to_vec();
    contract_mut(&mut wrong_direction, "api.error.response").direction =
        ContractDirection::Deserialize;
    let error = validate_contract_topology(&[], &wrong_direction, false)
        .expect_err("Error transport 必须只允许 Serialize");
    assert_diagnostic(
        &error,
        &[
            "contract=api.error.response",
            "location=error",
            "expected=serialize",
            "actual=deserialize",
        ],
    );

    let mut exact_error = operation_inventory().to_vec();
    let contract = contract_mut(&mut exact_error, "api.error.response");
    contract.binding = ContractBinding::ExactSurface;
    contract.transport = ContractTransport::Http {
        operation_key: Some("GET /api/v1/boards/:board/tasks"),
        location: HttpTransportLocation::Error,
        parameters: &[],
    };
    let error = validate_contract_topology(&[], &exact_error, false)
        .expect_err("Error transport 不得伪装 ExactSurface");
    assert_diagnostic(
        &error,
        &[
            "contract=api.error.response",
            "location=error",
            "expected=shared_component",
            "actual=exact_surface",
        ],
    );
}

#[test]
fn adopted_and_endpoint_exact_bindings_require_exact_granularity_without_closure() {
    let mut orphan_adopted = operation_inventory().to_vec();
    contract_mut(&mut orphan_adopted, "api.claim-task.request").granularity =
        ContractGranularity::Family;
    let error = validate_contract_topology(&[], &orphan_adopted, false)
        .expect_err("普通非 closure audit 也必须拒绝 Adopted Family");
    assert_diagnostic(
        &error,
        &[
            "contract=api.claim-task.request",
            "binding=exact_surface",
            "expected=exact",
            "actual=family",
        ],
    );

    let claim = *endpoint_descriptor("api.claim-task").expect("claim endpoint descriptor");
    let mut adopted_family = operation_inventory().to_vec();
    contract_mut(&mut adopted_family, "api.claim-task.request").granularity =
        ContractGranularity::Family;
    let error = validate_contract_topology(&[claim], &adopted_family, false)
        .expect_err("Adopted ExactSurface+Family obligation 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "obligation=body",
            "contract=api.claim-task.request",
            "binding=exact_surface",
            "expected=exact",
            "actual=family",
        ],
    );

    let delete = *endpoint_descriptor("api.delete-label-semantics")
        .expect("delete label semantics endpoint descriptor");
    let mut generated_family = operation_inventory().to_vec();
    contract_mut(&mut generated_family, "api.label-semantics-delete.response").granularity =
        ContractGranularity::Family;
    let error = validate_contract_topology(&[delete], &generated_family, false)
        .expect_err("Generated ExactSurface+Family obligation 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.delete-label-semantics",
            "obligation=success",
            "contract=api.label-semantics-delete.response",
            "binding=exact_surface",
            "expected=exact",
            "actual=family",
        ],
    );
}

#[test]
fn path_placeholder_mapping_rejects_name_missing_extra_order_and_case_drift() {
    for (mutation, parameters, declared) in [
        ("name", WRONG_PATH_NAME, "declared=[\"task\"]"),
        ("missing", &[] as &[WireParameter], "declared=[]"),
        (
            "extra",
            EXTRA_PATH_NAME,
            "declared=[\"task_id\", \"extra\"]",
        ),
        ("case", CASE_DRIFT_PATH_NAME, "declared=[\"TASK_ID\"]"),
    ] {
        let error = claim_path_mapping_error(parameters);
        assert_diagnostic(
            &error,
            &[
                "endpoint=api.claim-task",
                "obligation=path",
                "contract=api.claim-task.request",
                declared,
                "expected=[\"task_id\"]",
            ],
        );
        assert!(!error.contains("cardinality"), "{mutation}: {error}");
    }

    let mut endpoint = *endpoint_descriptor("api.delete-label-semantics")
        .expect("delete label semantics endpoint descriptor");
    endpoint.obligations.success = EndpointObligation::Todo;
    endpoint.obligations.path = EndpointObligation::Contract("api.label-semantics-delete.response");
    let mut inventory = operation_inventory().to_vec();
    let contract = contract_mut(&mut inventory, "api.label-semantics-delete.response");
    contract.direction = ContractDirection::Deserialize;
    contract.transport = ContractTransport::Http {
        operation_key: Some("DELETE /api/v1/boards/:board/labels/:label_id/semantics"),
        location: HttpTransportLocation::Path,
        parameters: REVERSED_LABEL_PATH_NAMES,
    };
    let error = validate_contract_topology(&[endpoint], &inventory, false)
        .expect_err("path placeholder 顺序漂移必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.delete-label-semantics",
            "obligation=path",
            "contract=api.label-semantics-delete.response",
            "declared=[\"label_id\", \"board\"]",
            "expected=[\"board\", \"label_id\"]",
        ],
    );
}

#[test]
fn cardinality_validation_covers_headers_names_forbidden_locations_and_shared_inputs() {
    let mut headers_endpoint = *endpoint_descriptor("api.claim-task").expect("claim endpoint");
    headers_endpoint.obligations.body = EndpointObligation::Todo;
    headers_endpoint.obligations.headers = EndpointObligation::Contract("api.claim-task.request");
    let mut header_conflict = operation_inventory().to_vec();
    contract_mut(&mut header_conflict, "api.claim-task.request").transport =
        ContractTransport::Http {
            operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
            location: HttpTransportLocation::Headers,
            parameters: HEADER_CASE_CONFLICT,
        };
    let error = validate_contract_topology(&[headers_endpoint], &header_conflict, false)
        .expect_err("header name 必须大小写不敏感地唯一");
    assert_diagnostic(
        &error,
        &[
            "contract=api.claim-task.request",
            "location=headers",
            "first=X-KB-Actor",
            "second=x-kb-actor",
        ],
    );

    for (parameters, actual, expected) in [
        (EMPTY_PARAMETER_NAME, "actual=\"\"", "non-empty"),
        (
            WHITESPACE_PARAMETER_NAME,
            "actual=\" status \"",
            "without_surrounding_whitespace",
        ),
    ] {
        let mut inventory = operation_inventory().to_vec();
        contract_mut(&mut inventory, "api.claim-task.request").transport =
            ContractTransport::Http {
                operation_key: Some("POST /api/v1/tasks/:task_id/transitions/claim"),
                location: HttpTransportLocation::Query,
                parameters,
            };
        let error = validate_contract_topology(&[], &inventory, false)
            .expect_err("空白 parameter name 必须失败");
        assert_diagnostic(
            &error,
            &[
                "contract=api.claim-task.request",
                "location=query",
                expected,
                actual,
            ],
        );
    }

    for (contract_id, location, operation_key) in [
        (
            "api.claim-task.request",
            HttpTransportLocation::Body,
            Some("POST /api/v1/tasks/:task_id/transitions/claim"),
        ),
        (
            "api.health.response",
            HttpTransportLocation::Success,
            Some("GET /health"),
        ),
        (
            "sse.event.data",
            HttpTransportLocation::Sse,
            Some("GET /api/v1/events"),
        ),
        ("api.error.response", HttpTransportLocation::Error, None),
    ] {
        let mut inventory = operation_inventory().to_vec();
        contract_mut(&mut inventory, contract_id).transport = ContractTransport::Http {
            operation_key,
            location,
            parameters: FORBIDDEN_WIRE_PARAMETER,
        };
        let error = validate_contract_topology(&[], &inventory, false)
            .expect_err("Body/Success/Sse/Error 不得声明 parameters");
        let expected_location = match location {
            HttpTransportLocation::Body => "body",
            HttpTransportLocation::Success => "success",
            HttpTransportLocation::Sse => "sse",
            HttpTransportLocation::Error => "error",
            _ => unreachable!(),
        };
        assert!(
            error.contains(&format!("contract={contract_id}")),
            "{error}"
        );
        assert!(
            error.contains(&format!("location={expected_location}")),
            "{error}"
        );
        assert_diagnostic(&error, &["expected=none", "actual_count=1"]);
    }

    let list_tasks = *endpoint_descriptor("api.list-tasks").expect("list tasks endpoint");
    let mut valid_shared_header = operation_inventory().to_vec();
    let shared = contract_mut(&mut valid_shared_header, "api.error.response");
    shared.direction = ContractDirection::Deserialize;
    shared.transport = ContractTransport::Http {
        operation_key: None,
        location: HttpTransportLocation::Headers,
        parameters: SHARED_HEADER_PARAMETER,
    };
    validate_contract_topology(&[list_tasks], &valid_shared_header, false)
        .expect("SharedComponent input header 可声明 OptionalOne cardinality");

    contract_mut(&mut valid_shared_header, "api.error.response").transport =
        ContractTransport::Http {
            operation_key: None,
            location: HttpTransportLocation::Headers,
            parameters: SHARED_HEADER_WITHOUT_CARDINALITY,
        };
    let error = validate_contract_topology(&[list_tasks], &valid_shared_header, false)
        .expect_err("shared header 缺 cardinality 必须失败");
    assert_diagnostic(
        &error,
        &[
            "contract=api.error.response",
            "location=headers",
            "parameter=X-Request-Id",
            "expected=some",
            "actual=none",
        ],
    );
}

#[test]
fn endpoint_drift_and_duplicate_diagnostics_are_actionable() {
    let claim = *endpoint_descriptor("api.claim-task").expect("claim endpoint");

    let mut wrong_direction = claim;
    wrong_direction.obligations.body = EndpointObligation::Contract("api.health.response");
    let error = validate_contract_topology(&[wrong_direction], operation_inventory(), false)
        .expect_err("wrong direction 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "obligation=body",
            "contract=api.health.response",
            "expected=deserialize",
            "actual=serialize",
        ],
    );

    let mut wrong_surface = claim;
    wrong_surface.obligations.body = EndpointObligation::Contract("metadata.decision.input");
    let error = validate_contract_topology(&[wrong_surface], operation_inventory(), false)
        .expect_err("wrong surface 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "obligation=body",
            "contract=metadata.decision.input",
            "expected=api",
            "actual=metadata",
        ],
    );

    let mut wrong_operation_inventory = operation_inventory().to_vec();
    contract_mut(&mut wrong_operation_inventory, "api.claim-task.request").transport =
        ContractTransport::Http {
            operation_key: Some("POST /api/v1/tasks/:task_id/transitions/other"),
            location: HttpTransportLocation::Body,
            parameters: &[],
        };
    let error = validate_contract_topology(&[claim], &wrong_operation_inventory, false)
        .expect_err("wrong operation 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "obligation=body",
            "contract=api.claim-task.request",
            "expected=POST /api/v1/tasks/:task_id/transitions/claim",
            "actual=POST /api/v1/tasks/:task_id/transitions/other",
        ],
    );

    let health = *endpoint_descriptor("api.health").expect("health endpoint");
    let duplicate_operation = [
        health,
        EndpointDescriptor {
            path: "/health-copy",
            ..health
        },
    ];
    let error = validate_contract_topology(&duplicate_operation, operation_inventory(), false)
        .expect_err("duplicate operation_id 必须失败");
    assert_diagnostic(
        &error,
        &[
            "operation_id=api.health",
            "first=GET /health",
            "second=GET /health-copy",
        ],
    );

    let duplicate_route = [
        health,
        EndpointDescriptor {
            operation_id: "api.health-copy",
            ..health
        },
    ];
    let error = validate_contract_topology(&duplicate_route, operation_inventory(), false)
        .expect_err("duplicate method/path 必须失败");
    assert_diagnostic(
        &error,
        &[
            "method/path",
            "first=api.health",
            "second=api.health-copy",
            "actual=GET /health",
        ],
    );
}

#[test]
fn exact_endpoint_uniqueness_is_structural_not_a_redundant_global_binding_guard() {
    let claim = *endpoint_descriptor("api.claim-task").expect("claim endpoint");

    let duplicate_route = [
        claim,
        EndpointDescriptor {
            operation_id: "api.claim-task-copy",
            ..claim
        },
    ];
    let error = validate_contract_topology(&duplicate_route, operation_inventory(), false)
        .expect_err("同 method/path 不能制造第二个 exact binding");
    assert_diagnostic(
        &error,
        &[
            "method/path",
            "first=api.claim-task",
            "second=api.claim-task-copy",
        ],
    );

    let different_route = [
        claim,
        EndpointDescriptor {
            operation_id: "api.claim-task-copy",
            path: "/api/v1/tasks/:task_id/transitions/claim-copy",
            ..claim
        },
    ];
    let error = validate_contract_topology(&different_route, operation_inventory(), false)
        .expect_err("不同 method/path 必须被 exact operation_key 拒绝");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task-copy",
            "obligation=path",
            "contract=api.claim-task.path",
            "expected=POST /api/v1/tasks/:task_id/transitions/claim-copy",
            "actual=POST /api/v1/tasks/:task_id/transitions/claim",
        ],
    );

    let mut two_locations = claim;
    two_locations.obligations.query = EndpointObligation::Contract("api.claim-task.request");
    let error = validate_contract_topology(&[two_locations], operation_inventory(), false)
        .expect_err("同 endpoint 的不同 obligation 必须被 location 拒绝");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "obligation=query",
            "contract=api.claim-task.request",
            "expected=query",
            "actual=body",
        ],
    );
}

#[test]
fn shared_and_exact_binding_surfaces_cannot_conflict_or_change_adoption() {
    let list_tasks = *endpoint_descriptor("api.list-tasks").expect("list tasks endpoint");
    assert_eq!(list_tasks.migration, MigrationState::Adopted);

    let mut shared_as_obligation = list_tasks;
    shared_as_obligation.obligations.success = EndpointObligation::Contract("api.error.response");
    let error = validate_contract_topology(&[shared_as_obligation], operation_inventory(), false)
        .expect_err("SharedComponent 不得进入 exact obligation");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.list-tasks",
            "obligation=success",
            "contract=api.error.response",
            "expected=exact_surface",
            "actual=shared_component",
        ],
    );

    let mut exact_as_shared = *endpoint_descriptor("api.claim-task").expect("claim endpoint");
    exact_as_shared.shared_components = &["api.claim-task.request"];
    let error = validate_contract_topology(&[exact_as_shared], operation_inventory(), false)
        .expect_err("ExactSurface 不得进入 shared_components");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.claim-task",
            "contract=api.claim-task.request",
            "expected=shared_component",
            "actual=exact_surface",
        ],
    );

    let mut duplicate_shared = list_tasks;
    duplicate_shared.shared_components = DUPLICATE_SHARED_API_ERROR;
    let error = validate_contract_topology(&[duplicate_shared], operation_inventory(), false)
        .expect_err("同 endpoint 重复 shared linkage 必须失败");
    assert_diagnostic(
        &error,
        &[
            "endpoint=api.list-tasks",
            "contract=api.error.response",
            "first=0",
            "second=1",
        ],
    );
}

const REQUIRED_NULLABLE_TASK_FIELDS: &[&str] = &[
    "description",
    "status_reason",
    "assignee",
    "scheduled_at",
    "due_at",
    "started_at",
    "completed_at",
    "archived_at",
    "claim_owner",
    "claim_expires_at",
    "last_heartbeat_at",
    "current_run_id",
    "max_retries",
    "result_summary",
];

fn api_task_fixture() -> serde_json::Value {
    serde_json::json!({
        "id": "task-1",
        "board_id": "board-1",
        "board_slug": "default",
        "ref": "default#1",
        "seq": 1,
        "title": "契约任务",
        "description": null,
        "status": "ready",
        "status_reason": null,
        "assignee": null,
        "priority": 3,
        "position": 1024,
        "scheduled_at": null,
        "due_at": null,
        "created_by": "tester",
        "created_at": 1,
        "updated_at": 2,
        "started_at": null,
        "completed_at": null,
        "archived_at": null,
        "claim_owner": null,
        "claim_expires_at": null,
        "last_heartbeat_at": null,
        "current_run_id": null,
        "retry_count": 0,
        "max_retries": null,
        "result_summary": null,
        "result": null,
        "metadata": {},
        "lock_version": 0,
        "dependency_blocked": false,
        "unfinished_parent_count": 0,
        "execution_plan_state": "planned",
        "required_step_count": 1,
        "completed_required_step_count": 0,
        "optional_step_count": 0,
        "labels": [{
            "id": "label-1",
            "board_id": "board-1",
            "name": "backend",
            "color": null,
            "created_at": 1,
            "updated_at": 2
        }]
    })
}

#[test]
fn api_task_and_label_wire_components_preserve_all_public_keys_and_nulls() {
    let fixture = api_task_fixture();
    let task: kanban_contract::ApiTask =
        serde_json::from_value(fixture.clone()).expect("完整公开 task wire 应可反序列化");
    assert_eq!(
        serde_json::to_value(task).expect("ApiTask 应可序列化"),
        fixture,
        "nullable 字段必须保留为显式 null，且 claim_token 不得进入公开 wire",
    );
}

#[test]
fn api_task_and_label_wire_components_reject_ambiguous_or_invalid_shapes() {
    for field in REQUIRED_NULLABLE_TASK_FIELDS {
        let mut value = api_task_fixture();
        value.as_object_mut().expect("task object").remove(*field);
        serde_json::from_value::<kanban_contract::ApiTask>(value)
            .expect_err("缺失 nullable key 必须失败");
    }

    for claim_token in [serde_json::Value::Null, serde_json::json!("secret")] {
        let mut value = api_task_fixture();
        value
            .as_object_mut()
            .expect("task object")
            .insert("claim_token".to_owned(), claim_token);
        serde_json::from_value::<kanban_contract::ApiTask>(value)
            .expect_err("任意 claim_token 都必须作为未知字段拒绝");
    }

    for (field, invalid) in [
        ("status", serde_json::json!("unknown")),
        ("execution_plan_state", serde_json::json!("unknown")),
        ("priority", serde_json::json!(-1)),
        ("priority", serde_json::json!(4)),
        ("labels", serde_json::Value::Null),
    ] {
        let mut value = api_task_fixture();
        value
            .as_object_mut()
            .expect("task object")
            .insert(field.to_owned(), invalid);
        serde_json::from_value::<kanban_contract::ApiTask>(value)
            .expect_err("非法 enum/priority/labels:null 必须失败");
    }

    let mut missing_color = api_task_fixture();
    missing_color["labels"][0]
        .as_object_mut()
        .expect("label object")
        .remove("color");
    serde_json::from_value::<kanban_contract::ApiTask>(missing_color)
        .expect_err("label.color 即使 nullable 也必须存在");

    let mut unknown_label_field = api_task_fixture();
    unknown_label_field["labels"][0]
        .as_object_mut()
        .expect("label object")
        .insert("claim_token".to_owned(), serde_json::json!("secret"));
    serde_json::from_value::<kanban_contract::ApiTask>(unknown_label_field)
        .expect_err("label 未知字段必须失败");
}

#[test]
fn api_status_priority_and_execution_plan_vocabulary_is_closed_and_checked() {
    let statuses = [
        (kanban_contract::ApiTaskStatus::Triage, "triage"),
        (kanban_contract::ApiTaskStatus::Todo, "todo"),
        (kanban_contract::ApiTaskStatus::Scheduled, "scheduled"),
        (kanban_contract::ApiTaskStatus::Ready, "ready"),
        (kanban_contract::ApiTaskStatus::Running, "running"),
        (kanban_contract::ApiTaskStatus::Blocked, "blocked"),
        (kanban_contract::ApiTaskStatus::Review, "review"),
        (kanban_contract::ApiTaskStatus::Done, "done"),
        (kanban_contract::ApiTaskStatus::Archived, "archived"),
    ];
    for (status, wire) in statuses {
        assert_eq!(status.as_str(), wire);
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::json!(wire)
        );
    }

    let plans = [
        (
            kanban_contract::ApiExecutionPlanState::Unplanned,
            "unplanned",
        ),
        (kanban_contract::ApiExecutionPlanState::Planned, "planned"),
        (
            kanban_contract::ApiExecutionPlanState::NotRequired,
            "not_required",
        ),
    ];
    for (state, wire) in plans {
        assert_eq!(state.as_str(), wire);
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!(wire)
        );
    }

    for value in 0..=3 {
        let priority = kanban_contract::ApiTaskPriority::try_from(value).expect("0..=3");
        assert_eq!(i64::from(priority.get()), value);
    }
    for value in [-1, 4, i64::MAX] {
        assert!(kanban_contract::ApiTaskPriority::try_from(value).is_err());
        assert!(
            serde_json::from_value::<kanban_contract::ApiTaskPriority>(serde_json::json!(value))
                .is_err()
        );
    }
    assert_eq!(kanban_contract::ApiTaskPriority::default().get(), 3);
}

fn collect_schema_types(schema: &serde_json::Value, output: &mut BTreeSet<String>) {
    match &schema["type"] {
        serde_json::Value::String(kind) => {
            output.insert(kind.clone());
        }
        serde_json::Value::Array(kinds) => {
            output.extend(
                kinds
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned),
            );
        }
        _ => {}
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(branches) = schema[keyword].as_array() {
            for branch in branches {
                collect_schema_types(branch, output);
            }
        }
    }
}

fn assert_required_nullable_schema(schema: &serde_json::Value, field: &str) {
    let mut types = BTreeSet::new();
    collect_schema_types(schema, &mut types);
    assert!(
        types.contains("null"),
        "{field} schema 必须允许 null，实际 type 集合为 {types:?}"
    );
    assert!(
        types.iter().any(|kind| kind != "null"),
        "{field} schema 必须同时允许非 null 值，实际 type 集合为 {types:?}"
    );
}

#[test]
fn api_task_schema_requires_nullable_keys_and_bounds_priority() {
    let task_schema = serde_json::to_value(schemars::schema_for!(kanban_contract::ApiTask))
        .expect("serialize ApiTask schema");
    assert_eq!(
        task_schema["additionalProperties"],
        serde_json::json!(false),
        "ApiTask schema 必须拒绝未知字段"
    );

    let required = task_schema["required"]
        .as_array()
        .expect("ApiTask schema required array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<BTreeSet<_>>();
    let expected_task_keys = api_task_fixture()
        .as_object()
        .expect("task fixture object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let properties = task_schema["properties"]
        .as_object()
        .expect("ApiTask schema properties");
    assert_eq!(
        properties.keys().cloned().collect::<BTreeSet<_>>(),
        expected_task_keys,
        "ApiTask schema properties 必须精确覆盖公开 wire 字段"
    );
    assert_eq!(
        required
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        expected_task_keys,
        "ApiTask 所有公开字段（包括 nullable）都必须是 required key"
    );
    for field in REQUIRED_NULLABLE_TASK_FIELDS {
        assert_required_nullable_schema(&properties[*field], field);
    }
    assert_eq!(
        properties["result"],
        serde_json::json!(true),
        "ApiTask.result must preserve arbitrary natural JSON, including null"
    );

    assert_eq!(
        properties["labels"]["items"]["$ref"],
        serde_json::json!("#/$defs/ApiLabel"),
        "ApiTask.labels 必须连接到同一 schema 内的 ApiLabel definition"
    );
    let nested_label = &task_schema["$defs"]["ApiLabel"];
    assert_eq!(
        nested_label["additionalProperties"],
        serde_json::json!(false),
        "嵌套 ApiLabel definition 必须拒绝未知字段"
    );
    let expected_label_keys = api_task_fixture()["labels"][0]
        .as_object()
        .expect("label fixture object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let nested_label_properties = nested_label["properties"]
        .as_object()
        .expect("nested ApiLabel properties");
    assert_eq!(
        nested_label_properties
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected_label_keys
    );
    assert_eq!(
        nested_label["required"]
            .as_array()
            .expect("nested ApiLabel required")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        expected_label_keys
    );
    assert_required_nullable_schema(&nested_label_properties["color"], "ApiLabel.color");

    let label_schema = serde_json::to_value(schemars::schema_for!(kanban_contract::ApiLabel))
        .expect("serialize ApiLabel schema");
    assert_eq!(
        label_schema["additionalProperties"],
        serde_json::json!(false)
    );
    assert_required_nullable_schema(&label_schema["properties"]["color"], "ApiLabel.color root");

    let status_schema = serde_json::to_value(schemars::schema_for!(kanban_contract::ApiTaskStatus))
        .expect("serialize status schema");
    assert_eq!(
        status_schema["enum"],
        serde_json::json!([
            "triage",
            "todo",
            "scheduled",
            "ready",
            "running",
            "blocked",
            "review",
            "done",
            "archived"
        ])
    );
    let plan_schema = serde_json::to_value(schemars::schema_for!(
        kanban_contract::ApiExecutionPlanState
    ))
    .expect("serialize execution plan schema");
    assert_eq!(
        plan_schema["enum"],
        serde_json::json!(["unplanned", "planned", "not_required"])
    );

    let priority_schema =
        serde_json::to_value(schemars::schema_for!(kanban_contract::ApiTaskPriority))
            .expect("serialize priority schema");
    assert_eq!(priority_schema["minimum"], serde_json::json!(0));
    assert_eq!(priority_schema["maximum"], serde_json::json!(3));
}
