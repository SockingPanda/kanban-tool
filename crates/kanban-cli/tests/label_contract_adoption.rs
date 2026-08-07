//! 通过真实 `kanban` 子进程和 localhost host 验证 label/ontology CLI surface。

mod knowledge_support;

use kanban_protocol::{
    CliTaskCreateOutput,
    cli_labels::{
        CliLabelAtomIndexQueryOutput, CliLabelAtomIndexRebuildOutput,
        CliLabelAtomIndexStatusOutput, CliLabelAtomsExplainOutput, CliLabelAtomsListOutput,
        CliLabelBootstrapOutput, CliLabelCreateOutput, CliLabelDeleteOutput,
        CliLabelOntologyConfirmOutput, CliLabelOntologyQualityOutput, CliLabelOntologyRecordOutput,
        CliLabelOntologyShowOutput, CliLabelProposalsAcceptOutput, CliLabelProposalsListOutput,
        CliLabelProposalsRejectOutput, CliLabelProposalsShowOutput, CliLabelProposeOutput,
        CliLabelSemanticsListOutput, CliLabelSemanticsShowOutput, CliLabelSemanticsUpsertOutput,
        CliLabelSuggestOutput,
    },
};

use knowledge_support::Host;

#[test]
fn label_delete_flow_through_real_cli() {
    let host = Host::new();
    let label: CliLabelCreateOutput = host.json(&["label", "create", "temporary-delete"]);
    let deleted: CliLabelDeleteOutput = host.json(&["label", "delete", label.data.name.as_str()]);
    assert_eq!(deleted.data.label.id, label.data.id);
    assert!(!deleted.data.forced);
    assert_eq!(deleted.data.removed_task_bindings, 0);
    assert!(!deleted.data.removed_semantics);
    assert_eq!(deleted.data.removed_atoms, 0);
}

#[test]
fn labels_semantics_atoms_and_proposals_flow_through_real_cli() {
    let host = Host::new();
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "CLI ontology task",
        "--task-id",
        "t_cli_label_adoption",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.id, "t_cli_label_adoption");

    let label: CliLabelCreateOutput =
        host.json(&["label", "create", "backend-api", "--color", "#123456"]);
    assert_eq!(label.data.name, "backend-api");
    assert_eq!(label.data.color.as_deref(), Some("#123456"));

    let semantics_payload = r#"{
        "description":"Backend HTTP integration work",
        "applies_when":["localhost API","HTTP route"],
        "excludes_when":["desktop-only"],
        "positive_examples":["add a route"],
        "negative_examples":["change a color"]
    }"#;
    let semantics: CliLabelSemanticsUpsertOutput = host.json(&[
        "label",
        "semantics",
        "upsert",
        label.data.name.as_str(),
        "--payload",
        semantics_payload,
    ]);
    assert_eq!(semantics.data.label_name, "backend-api");
    assert_eq!(
        semantics.data.description.as_deref(),
        Some("Backend HTTP integration work")
    );
    assert_eq!(semantics.data.applies_when, ["localhost API", "HTTP route"]);
    assert!(!semantics.data.semantics_hash.is_empty());

    let listed: CliLabelSemanticsListOutput = host.json(&["label", "semantics", "list"]);
    assert!(
        listed
            .data
            .iter()
            .any(|item| item.label_id == label.data.id)
    );
    let shown: CliLabelSemanticsShowOutput =
        host.json(&["label", "semantics", "show", label.data.name.as_str()]);
    assert_eq!(shown.data.semantics_hash, semantics.data.semantics_hash);

    let atoms: CliLabelAtomsListOutput = host.json(&["label", "atoms", "list"]);
    assert!(
        atoms
            .data
            .iter()
            .any(|atom| atom.label_id == label.data.id && atom.text == "localhost API")
    );
    let atom = atoms
        .data
        .iter()
        .find(|atom| atom.label_id == label.data.id && atom.text == "localhost API")
        .expect("semantics should produce a label atom");
    let explained: CliLabelAtomsExplainOutput =
        host.json(&["label", "atoms", "explain", atom.id.as_str()]);
    assert_eq!(explained.data.query, atom.id);
    assert_eq!(
        explained.data.atom.as_ref().map(|value| value.id.as_str()),
        Some(atom.id.as_str())
    );

    let index_status: CliLabelAtomIndexStatusOutput = host.json(&["label", "atom-index", "status"]);
    assert!(!index_status.data.backend.is_empty());
    let rebuilt: CliLabelAtomIndexRebuildOutput = host.json(&["label", "atom-index", "rebuild"]);
    assert_eq!(rebuilt.data.backend, index_status.data.backend);
    let index_query: CliLabelAtomIndexQueryOutput = host.json(&[
        "label",
        "atom-index",
        "query",
        "--q",
        "localhost",
        "--limit",
        "5",
    ]);
    assert!(index_query.data.iter().any(|hit| hit.atom_id == atom.id));

    let suggestions: CliLabelSuggestOutput =
        host.json(&["label", "suggest", "t_cli_label_adoption"]);
    assert_eq!(suggestions.data.task_id, "t_cli_label_adoption");
    assert!(
        suggestions.data.degraded,
        "the test host has no embedding provider; the CLI must preserve degraded state"
    );
    assert!(
        suggestions
            .data
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "vector_provider_unavailable")
    );

    let proposal_payload = r#"{
        "description":"A proposed deployment label",
        "applies_when":["release pipeline"],
        "positive_examples":["deploy service"]
    }"#;
    let attempted: CliLabelProposeOutput = host.json(&[
        "label",
        "propose",
        "t_cli_label_adoption",
        "--name",
        "deployment",
        "--payload",
        proposal_payload,
    ]);
    assert!(attempted.data.degraded);
    let proposal = attempted
        .data
        .proposal
        .as_ref()
        .expect("explicit proposal name should persist a proposal");
    assert_eq!(proposal.name, "deployment");
    let proposal_id = proposal.id.clone();

    let proposals: CliLabelProposalsListOutput = host.json(&[
        "label",
        "proposals",
        "list",
        "--task-ref",
        "t_cli_label_adoption",
    ]);
    assert!(proposals.data.iter().any(|item| item.id == proposal_id));
    let shown: CliLabelProposalsShowOutput =
        host.json(&["label", "proposals", "show", proposal_id.as_str()]);
    assert_eq!(shown.data.id, proposal_id);

    let accepted: CliLabelProposalsAcceptOutput = host.json(&[
        "label",
        "proposals",
        "accept",
        proposal_id.as_str(),
        "--reason",
        "adopted by CLI adoption test",
    ]);
    assert_eq!(accepted.data.id, proposal_id);
    assert!(matches!(
        accepted.data.status,
        kanban_protocol::LabelProposalStatusWire::Accepted
    ));

    let second: CliLabelProposeOutput = host.json(&[
        "label",
        "propose",
        "t_cli_label_adoption",
        "--name",
        "temporary-label",
    ]);
    let second_id = second
        .data
        .proposal
        .as_ref()
        .expect("second proposal should persist")
        .id
        .clone();
    let rejected: CliLabelProposalsRejectOutput = host.json(&[
        "label",
        "proposals",
        "reject",
        second_id.as_str(),
        "--reason",
        "not needed",
    ]);
    assert_eq!(rejected.data.id, second_id);
    assert!(matches!(
        rejected.data.status,
        kanban_protocol::LabelProposalStatusWire::Rejected
    ));
}

#[test]
fn bootstrap_label_flow_through_real_cli() {
    let host = Host::new();
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "CLI bootstrap task",
        "--task-id",
        "t_cli_label_bootstrap",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.id, "t_cli_label_bootstrap");

    let output: CliLabelBootstrapOutput = host.json(&[
        "label",
        "bootstrap",
        task.data.id.as_str(),
        "backend",
        "--description",
        "Backend implementation work",
        "--applies-when",
        "touches Rust service code",
        "--positive-example",
        "add a service command",
    ]);
    let fixture: CliLabelBootstrapOutput = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/cli/label-bootstrap-output.v1.valid.json"
    ))
    .expect("bootstrap CLI fixture");
    assert_eq!(output.data.task.id, task.data.id);
    assert_eq!(
        output.data.semantics.label_name,
        fixture.data.semantics.label_name
    );
    assert_eq!(
        output.data.semantics.applies_when,
        fixture.data.semantics.applies_when
    );
    assert_eq!(output.data.verification, fixture.data.verification);
}

#[test]
fn ontology_observation_signal_review_and_action_flow_through_real_cli() {
    let host = Host::new();
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "Ontology signal task",
        "--task-id",
        "t_cli_ontology_signal",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.id, "t_cli_ontology_signal");
    let _: CliLabelCreateOutput = host.json(&["label", "create", "ontology-target"]);

    let observation_payload = r#"{
        "actor":{"name":"cli-adoption","type":"user"},
        "agent_candidates":[],
        "suggestion_snapshot":{},
        "final_decision":{},
        "signals":[{
            "kind":"vocabulary_gap",
            "target_label_ref":"ontology-target",
            "related_labels":[],
            "proposed_action":"observe",
            "candidate_atom":null,
            "proposed_label_name":"release-train",
            "proposal":{},
            "agent_selected":false,
            "suggest_state":"candidate",
            "suggest_score":0.3,
            "suggest_rank":1,
            "final_selected":false,
            "rationale":"CLI ontology adoption evidence",
            "confidence":0.7,
            "signal_key":"cli-adoption-vocabulary-gap"
        }]
    }"#;
    let observation: CliLabelOntologyRecordOutput = host.json(&[
        "label",
        "ontology",
        "record",
        "t_cli_ontology_signal",
        "--payload",
        observation_payload,
    ]);
    assert_eq!(observation.data.task_id, "t_cli_ontology_signal");
    assert_eq!(observation.data.signals.len(), 1);
    let signal_id = observation.data.signals[0].id.clone();

    let signals: kanban_protocol::MetadataEnvelope<
        Vec<kanban_protocol::LabelOntologySignalWire>,
        kanban_protocol::SignalFilterMeta,
    > = host.json(&["label", "ontology", "signals", "--include-all"]);
    assert!(signals.data.iter().any(|signal| signal.id == signal_id));
    let detail: CliLabelOntologyShowOutput =
        host.json(&["label", "ontology", "show", signal_id.as_str()]);
    assert_eq!(detail.data.signal.id, signal_id);
    assert_eq!(detail.data.observation.id, observation.data.id);

    let review: kanban_protocol::ReviewLabelOntologyResponse = host.json(&[
        "label",
        "ontology",
        "review",
        "--group-by",
        "label",
        "--include-all",
    ]);
    assert!(!review.data.is_empty());

    let quality: CliLabelOntologyQualityOutput =
        host.json(&["label", "ontology", "quality", "--sample-limit", "10"]);
    assert_eq!(quality.data.board_id, "b_default");
    assert_eq!(quality.data.denominator.observation_count, 1);
    assert_eq!(quality.data.denominator.distinct_task_count, 1);

    let action_payload = format!(
        r#"{{
            "actor":{{"name":"cli-adoption","type":"user"}},
            "signal_ids":["{signal_id}"],
            "reason":"confirm ontology observation"
        }}"#
    );
    let action: CliLabelOntologyConfirmOutput = host.json(&[
        "label",
        "ontology",
        "confirm",
        "--payload",
        action_payload.as_str(),
    ]);
    assert!(matches!(
        action.data.action_type,
        kanban_protocol::LabelOntologyActionTypeWire::Confirm
    ));
    assert!(action.data.signal_ids.iter().any(|id| id == &signal_id));

    let closed: CliLabelOntologyShowOutput =
        host.json(&["label", "ontology", "show", signal_id.as_str()]);
    assert_eq!(closed.data.signal.status, "confirmed");
}

#[test]
fn generic_signals_record_review_and_confirm_flow_through_real_cli() {
    let host = Host::new();
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "Generic signal task",
        "--task-id",
        "t_cli_generic_signal",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.id, "t_cli_generic_signal");

    let request = format!(
        r#"{{
        "kind":"cli_adoption",
        "title":"CLI adoption signal",
        "summary":"recorded through stdin",
        "severity":"warning",
        "task_ref":"{}",
        "actor":"cli-adoption",
        "agent_type":"test",
        "source":"cli-adoption",
        "dedupe_key":"cli-adoption-signal-1",
        "evidence":{{"command":"kanban signal record"}}
    }}"#,
        task.data.task_ref
    );
    let recorded: kanban_protocol::cli_operator::CliSignalRecordOutput = host
        .run_with_stdin(&["signal", "record"], &request)
        .pipe_json();
    assert_eq!(recorded.data.signal.kind, "cli_adoption");
    assert_eq!(
        recorded.data.signal.observation.task_id.as_deref(),
        Some("t_cli_generic_signal")
    );
    assert!(
        recorded
            .data
            .signal
            .observation
            .task_ref_snapshot
            .as_deref()
            .is_some_and(|task_ref| task_ref.starts_with("default#"))
    );
    let signal_id = recorded.data.signal.id.clone();

    let review: kanban_protocol::cli_operator::CliSignalReviewOutput =
        host.json(&["signal", "review"]);
    assert!(review.data.iter().any(|signal| signal.id == signal_id));
    let shown: kanban_protocol::cli_operator::CliSignalShowOutput =
        host.json(&["signal", "show", signal_id.as_str()]);
    assert_eq!(shown.data.id, signal_id);

    let confirmed: kanban_protocol::cli_operator::CliSignalConfirmOutput = host.json(&[
        "signal",
        "confirm",
        signal_id.as_str(),
        "--reason",
        "verified",
    ]);
    assert_eq!(confirmed.data.len(), 1);
    assert!(matches!(
        confirmed.data[0].status,
        kanban_protocol::cli_operator::CliSignalStatus::Confirmed
    ));
}

trait OutputJson {
    fn pipe_json<T: serde::de::DeserializeOwned>(self) -> T;
}

impl OutputJson for std::process::Output {
    fn pipe_json<T: serde::de::DeserializeOwned>(self) -> T {
        serde_json::from_slice(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "CLI output is not valid typed JSON: {error}; stdout={}",
                String::from_utf8_lossy(&self.stdout)
            )
        })
    }
}
