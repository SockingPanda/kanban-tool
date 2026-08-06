//! CLI→localhost 的 entity/relation/search/vector/graph/context/index 真实冒烟。

mod knowledge_support;

use kanban_protocol::cli_helpers::{
    CliGraphNeighborsOutput, CliGraphQueryOutput, CliGraphRebuildOutput, CliGraphStatusOutput,
    CliGraphSyncOutput, CliSearchOutput,
};
use kanban_protocol::{
    BoardTaskMapResponse, BuildContextResponse, CliEntityListOutput, CliEntityShowOutput,
    CliIndexDoctorOutput, CliIndexStatusOutput, CliTaskCreateOutput, TaskNeighborhoodResponse,
    VectorConfigureResponse, VectorProjectionResponse, VectorStatusResponse,
};

use knowledge_support::Host;

#[test]
fn knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers() {
    let host = Host::new();
    let label: kanban_protocol::cli_labels::CliLabelCreateOutput =
        host.json(&["label", "create", "knowledge-label"]);
    let parent: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "Canonical search parent",
        "--task-id",
        "t_cli_knowledge_parent",
        "--description",
        "unique-knowledge-phrase parent task",
        "--status",
        "todo",
    ]);
    let child: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "Canonical search child",
        "--task-id",
        "t_cli_knowledge_child",
        "--description",
        "unique-knowledge-phrase child task",
        "--status",
        "todo",
        "--label",
        label.data.name.as_str(),
        "--depends-on",
        parent.data.id.as_str(),
    ]);
    assert_eq!(parent.data.id, "t_cli_knowledge_parent");
    assert_eq!(child.data.id, "t_cli_knowledge_child");

    let entity_upsert: CliEntityShowOutput = host.json(&[
        "entity",
        "upsert",
        "--uri",
        "kb://note/cli-knowledge",
        "--kind",
        "note",
        "--source-table",
        "cli_adoption",
        "--source-id",
        "knowledge-note-1",
        "--title",
        "Knowledge note",
        "--summary",
        "Entity upsert through the CLI",
    ]);
    assert_eq!(entity_upsert.data.uri, "kb://note/cli-knowledge");
    assert_eq!(entity_upsert.data.kind, "note");
    let entity: CliEntityShowOutput = host.json(&["entity", "show", "kb://note/cli-knowledge"]);
    assert_eq!(entity.data.title.as_deref(), Some("Knowledge note"));
    let entities: CliEntityListOutput = host.json(&["entity", "list", "--kind", "note"]);
    assert!(
        entities
            .data
            .iter()
            .any(|value| value.uri == "kb://note/cli-knowledge")
    );

    let graph_rebuild: CliGraphRebuildOutput = host.json(&["graph", "rebuild"]);
    assert_eq!(graph_rebuild.data.board_id, "b_default");
    assert!(graph_rebuild.data.validated_tasks >= 2);
    let graph_status: CliGraphStatusOutput = host.json(&["graph", "status"]);
    assert!(graph_status.data.enabled);
    assert_eq!(graph_status.data.backend, "turso-canonical");

    let neighbors: CliGraphNeighborsOutput =
        host.json(&["graph", "neighbors", "kb://task/t_cli_knowledge_parent"]);
    assert!(
        neighbors
            .data
            .iter()
            .any(|relation| relation.subject_uri == "kb://task/t_cli_knowledge_parent")
    );
    let query: CliGraphQueryOutput = host.json(&[
        "graph",
        "query",
        "SELECT ?subject ?predicate ?object WHERE { ?subject ?predicate ?object }",
        "--limit",
        "20",
    ]);
    assert!(query.data.iter().any(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.value == "kb://task/t_cli_knowledge_parent")
    }));
    let sync: CliGraphSyncOutput = host.json(&["graph", "sync"]);
    assert_eq!(sync.data.mode, "sync");

    let neighborhood: TaskNeighborhoodResponse = host.json(&[
        "graph",
        "neighborhood",
        "t_cli_knowledge_child",
        "--depth",
        "2",
    ]);
    assert_eq!(neighborhood.data.center_task_id, "t_cli_knowledge_child");
    assert!(
        neighborhood
            .data
            .nodes
            .iter()
            .any(|node| node.task.id == "t_cli_knowledge_parent")
    );
    assert!(
        neighborhood
            .data
            .edges
            .iter()
            .any(|edge| edge.source_task_id == "t_cli_knowledge_parent")
    );
    let map: BoardTaskMapResponse = host.json(&["graph", "map"]);
    assert!(
        map.data
            .nodes
            .iter()
            .any(|node| node.task.id == "t_cli_knowledge_child")
    );

    let rebuilt_index: CliIndexStatusOutput = host.json(&["index", "rebuild"]);
    assert_eq!(rebuilt_index.data.resolved_board_id, "b_default");
    let index_status: CliIndexStatusOutput = host.json(&["index", "status"]);
    assert_eq!(index_status.data.resolved_board_id, "b_default");
    let index_doctor: CliIndexDoctorOutput = host.json(&["index", "doctor"]);
    assert_eq!(index_doctor.data.backend, index_status.data.backend);
    let _: CliIndexStatusOutput = host.json(&["index", "sync"]);

    let search: CliSearchOutput = host.json(&[
        "search",
        "unique-knowledge-phrase",
        "--label",
        "knowledge-label",
        "--limit",
        "10",
    ]);
    assert!(
        search
            .data
            .hits
            .iter()
            .any(|hit| hit.task_id == "t_cli_knowledge_child")
    );
    assert_eq!(search.meta.resolved_board_id, "b_default");

    let vector_config: VectorConfigureResponse = host.json(&[
        "vector",
        "configure",
        "--endpoint",
        "http://127.0.0.1:1",
        "--model",
        "missing-embedding-model",
        "--dimensions",
        "3",
    ]);
    assert_eq!(vector_config.data.endpoint, "http://127.0.0.1:1");
    assert_eq!(vector_config.data.model, "missing-embedding-model");
    let vector_status: VectorStatusResponse = host.json(&["vector", "status"]);
    assert!(
        vector_status.data.enabled,
        "vector status after unavailable configuration: {:?}",
        vector_status.data
    );
    assert!(vector_status.data.dirty.unwrap_or(false));
    let rebuilt_vector: VectorProjectionResponse = host.json(&["vector", "rebuild"]);
    assert!(rebuilt_vector.data.enabled);
    let synced_vector: VectorProjectionResponse = host.json(&["vector", "sync"]);
    assert!(synced_vector.data.enabled);
    assert!(
        synced_vector.data.message.contains("vector projection")
            || !synced_vector.data.diagnostics.is_empty()
    );

    let context: BuildContextResponse = host.json(&[
        "context",
        "build",
        "t_cli_knowledge_child",
        "--query",
        "unique-knowledge-phrase",
        "--max-items",
        "10",
    ]);
    assert_eq!(context.data.subject, "kb://task/t_cli_knowledge_child");
    assert!(context.data.providers.iter().any(|provider| {
        provider.provider == "vector"
            && provider.capability == "turso_vector32_ollama"
            && provider.degraded
            && !provider.available
    }));
    assert!(context.data.items.iter().any(|item| {
        item.entity_uri == "kb://task/t_cli_knowledge_child"
            || item.entity_uri == "kb://task/t_cli_knowledge_parent"
    }));
}
