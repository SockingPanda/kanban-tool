mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir_envs};
use kanban_contract::cli_helpers::{
    CliContextBuildOutput, CliGraphNeighborsOutput, CliGraphQueryOutput, CliGraphRebuildOutput,
    CliGraphStatusOutput, CliGraphSyncOutput, CliIndexRebuildOutput, CliIndexSyncOutput,
    CliSearchOutput, CliVectorConfigureOutput, CliVectorQueryChunksOutput,
    CliVectorQueryLabelAtomsOutput, CliVectorRebuildOutput, CliVectorStatusOutput,
    CliVectorSyncOutput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str, validity: &str) -> anyhow::Result<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        format!("schemas/fixtures/cli/{operation}-output.v1.{validity}.json"),
    ))?)?)
}

fn consume_contract<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    let valid = fixture(operation, "valid")?;
    serde_json::from_value::<T>(valid.clone())?;
    anyhow::ensure!(
        serde_json::from_value::<T>(fixture(operation, "invalid")?).is_err(),
        "{operation} invalid fixture must be rejected"
    );

    let mut unknown = valid;
    let data = unknown.get_mut("data").context("fixture data")?;
    match data {
        Value::Object(object) => {
            object.insert("unexpected".to_owned(), json!(true));
        }
        Value::Array(items) => {
            items
                .first_mut()
                .and_then(Value::as_object_mut)
                .context("non-empty object array fixture")?
                .insert("unexpected".to_owned(), json!(true));
        }
        _ => anyhow::bail!("{operation} fixture data must be object or array"),
    }
    anyhow::ensure!(
        serde_json::from_value::<T>(unknown).is_err(),
        "{operation} public contract must reject unknown fields"
    );
    Ok(())
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source start: {start}"));
    let end = source[start..]
        .find(end)
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing source end: {end}"));
    &source[start..end]
}

fn contract_output_ownership_violations(handler: &str, outputs: &[&str]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut constructors = Vec::new();
    for output in outputs {
        let constructor = format!("{output}::new(");
        let positions = handler
            .match_indices(&constructor)
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        if positions.len() != 1 {
            violations.push(format!(
                "{output} must be constructed exactly once, found {}",
                positions.len()
            ));
        } else {
            constructors.push((positions[0], *output));
        }
    }
    constructors.sort_unstable_by_key(|(position, _)| *position);
    for (index, (start, output)) in constructors.iter().enumerate() {
        let end = constructors
            .get(index + 1)
            .map_or(handler.len(), |(position, _)| *position);
        let leaf = &handler[*start..end];
        let renders = leaf
            .matches("print_contract_or_human(json, &output,")
            .count();
        if renders != 1 {
            violations.push(format!(
                "{output} must render its contract-owned output exactly once, found {renders}"
            ));
        }
    }
    violations
}

fn assert_fail_closed_ownership(family: &str, handler: &str, outputs: &[&str]) {
    let violations = contract_output_ownership_violations(handler, outputs);
    assert!(
        violations.is_empty(),
        "{family} production ownership violations: {violations:#?}"
    );

    for output in outputs {
        let constructor = format!("{output}::new(");
        let private_construction = handler.replacen(&constructor, "legacy_domain_output(", 1);
        assert!(
            !contract_output_ownership_violations(&private_construction, outputs).is_empty(),
            "{family}/{output}: ownership gate must reject loss of contract construction"
        );

        let constructor_start = handler
            .find(&constructor)
            .unwrap_or_else(|| panic!("missing {output} constructor"));
        let render = "print_contract_or_human(json, &output,";
        let render_start = constructor_start
            + handler[constructor_start..]
                .find(render)
                .unwrap_or_else(|| panic!("missing {output} contract renderer"));
        let mut private_serialization = handler.to_owned();
        private_serialization.replace_range(
            render_start..render_start + render.len(),
            "print_or_json(json, &legacy_domain_value,",
        );
        assert!(
            !contract_output_ownership_violations(&private_serialization, outputs).is_empty(),
            "{family}/{output}: ownership gate must reject legacy serializer regression"
        );
    }
}

#[test]
fn helper_handlers_have_fail_closed_contract_output_ownership() {
    let substrate = include_str!("../src/commands/substrate.rs");
    let graph = source_between(
        substrate,
        "pub(crate) fn handle_graph",
        "fn graph_degraded_status",
    );
    assert_fail_closed_ownership(
        "graph",
        graph,
        &[
            "CliGraphStatusOutput",
            "CliGraphNeighborsOutput",
            "CliGraphRebuildOutput",
            "CliGraphSyncOutput",
            "CliGraphQueryOutput",
        ],
    );

    let vector = source_between(
        substrate,
        "pub(crate) fn handle_vector",
        "pub(crate) fn handle_context",
    );
    assert_fail_closed_ownership(
        "vector",
        vector,
        &[
            "CliVectorConfigureOutput",
            "CliVectorStatusOutput",
            "CliVectorRebuildOutput",
            "CliVectorSyncOutput",
            "CliVectorQueryChunksOutput",
            "CliVectorQueryLabelAtomsOutput",
        ],
    );
    assert!(
        vector.contains("vector_helper_json::<VectorHelperQueryLabelAtomsResponse>")
            && vector.contains(".map(cli_vector_label_atom_hit)"),
        "query-label-atoms must decode the helper contract before explicit public CLI mapping"
    );

    let context = source_between(
        substrate,
        "pub(crate) fn handle_context",
        "fn vector_config_from_args",
    );
    assert_fail_closed_ownership("context", context, &["CliContextBuildOutput"]);

    let search_source = include_str!("../src/commands/search.rs");
    let search = &search_source[search_source
        .find("pub(crate) fn handle_search")
        .expect("search handler source")..];
    assert_fail_closed_ownership("search", search, &["CliSearchOutput"]);

    let index_source = include_str!("../src/commands/index.rs");
    let index = source_between(
        index_source,
        "pub(crate) fn handle_index",
        "fn status_summary",
    );
    assert_fail_closed_ownership(
        "index",
        index,
        &["CliIndexRebuildOutput", "CliIndexSyncOutput"],
    );
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn write_helper(temp: &TempDb) -> anyhow::Result<PathBuf> {
    let helper = temp.dir.join("fixture-helper.py");
    std::fs::write(
        &helper,
        r#"#!/usr/bin/env python3
import json, os, sys
cmd = sys.argv[1]
if cmd == "status":
    payload = {"backend":"fixture-helper","enabled":True,"message":"helper status"}
    if os.environ.get("KANBAN_GRAPH_HELPER") != sys.argv[0]:
        payload.update({"diagnostics":["fixture"],"dirty":False,"board_dirty":False,"generation":7})
elif cmd == "rebuild":
    payload = {"backend":"fixture-helper","enabled":True,"message":"helper rebuilt"}
    if os.environ.get("KANBAN_GRAPH_HELPER") != sys.argv[0]:
        payload.update({"diagnostics":["fixture"],"dirty":False,"board_dirty":False,"generation":8})
elif cmd == "sync":
    payload = {"backend":"fixture-helper","enabled":True,"message":"helper synced"}
    if os.environ.get("KANBAN_GRAPH_HELPER") != sys.argv[0]:
        payload.update({"diagnostics":["fixture"],"dirty":False,"board_dirty":False,"generation":9})
elif cmd == "neighbors":
    payload = [{"subject_uri":"kb://task/t_subject","predicate":"depends_on","object_uri":"kb://task/t_parent","graph_uri":"kb://graph/relations","provenance":{"source_table":"task_dependencies","source_id":"t_parent->t_subject","source_event_id":12,"authoritative_store":"sqlite"},"metadata":{},"created_at":100,"updated_at":101}]
elif cmd == "query":
    payload = [{"bindings":[{"name":"task","value":"kb://task/t_subject"},{"name":"parent","value":"kb://task/t_parent"}]}]
elif cmd == "query-chunks":
    payload = [{"chunk":{"uri":"kb://chunk/task/t_vector_fixture/0","entity_uri":"kb://task/t_vector_fixture","ordinal":0,"content_hash":"hash-vector"},"score":0.91,"text":"vector fixture text","summary":"Vector fixture"}]
elif cmd == "query-label-atoms":
    payload = [{"hit":{"atom_id":"atom_backend_positive","label_id":"label_backend","label_name":"backend","board_id":"b_fixture","polarity":"positive","kind":"applies_when","text":"touches rust service code","ordinal":0,"content_hash":"hash-atom","embedding_model":"fixture-model","distance":0.125},"vector":[1.0,0.0]}]
else:
    raise SystemExit("unexpected command " + cmd)
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;
    let mut permissions = std::fs::metadata(&helper)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&helper, permissions)?;
    Ok(helper)
}

fn helper_json(
    temp: &TempDb,
    helper_env: &'static str,
    helper: &Path,
    args: &[&str],
) -> anyhow::Result<Value> {
    kanban_in_dir_envs(&temp.path, args, &temp.dir, &[(helper_env, helper)])?.success_json()
}

fn normalize_index_status(
    mut output: Value,
    expected_prefix: &str,
    fixture_message: &str,
) -> anyhow::Result<Value> {
    output["data"]["last_event_id"] = json!(10);
    let message = output["data"]["message"]
        .as_str()
        .context("index status message")?;
    anyhow::ensure!(
        message.starts_with(expected_prefix),
        "unexpected index message: {message}"
    );
    output["data"]["message"] = json!(fixture_message);
    Ok(output)
}

fn normalize_context(mut output: Value, task_id: &str) -> anyhow::Result<Value> {
    let actual_uri = format!("kb://task/{task_id}");
    if output["data"]["subject"] == actual_uri {
        output["data"]["subject"] = json!("kb://task/t_fixture");
    }
    for item in output["data"]["items"]
        .as_array_mut()
        .context("context items")?
    {
        if item["entity_uri"] == actual_uri {
            item["entity_uri"] = json!("kb://task/t_fixture");
        }
    }
    Ok(output)
}

fn normalize_search(mut output: Value, task_id: &str, board_id: &str) -> anyhow::Result<Value> {
    let hit = output["data"]["hits"]
        .as_array_mut()
        .and_then(|hits| hits.first_mut())
        .context("search hit")?;
    anyhow::ensure!(hit["task_id"] == task_id);
    hit["task_id"] = json!("t_fixture");
    let task = hit["task"].as_object_mut().context("search task")?;
    anyhow::ensure!(task["id"] == task_id);
    anyhow::ensure!(task["board_id"] == board_id);
    task.insert("id".to_owned(), json!("t_fixture"));
    task.insert("board_id".to_owned(), json!("b_fixture"));
    task.insert("created_at".to_owned(), json!(100));
    task.insert("updated_at".to_owned(), json!(101));
    output["meta"]["last_event_id"] = json!(10);
    Ok(output)
}

#[test]
fn producer_graph_neighbors_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_graph_neighbors_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &helper,
        &["--json", "graph", "neighbors", "kb://task/t_subject"],
    )?;
    serde_json::from_value::<CliGraphNeighborsOutput>(output.clone())?;
    assert_eq!(output, fixture("graph-neighbors", "valid")?);
    Ok(())
}

#[test]
fn graph_neighbors_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliGraphNeighborsOutput>("graph-neighbors")
}

#[test]
fn producer_graph_query_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_graph_query_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &helper,
        &["--json", "graph", "query", "SELECT * WHERE { ?s ?p ?o }"],
    )?;
    serde_json::from_value::<CliGraphQueryOutput>(output.clone())?;
    assert_eq!(output, fixture("graph-query", "valid")?);
    Ok(())
}

#[test]
fn graph_query_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliGraphQueryOutput>("graph-query")
}

#[test]
fn producer_graph_rebuild_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_graph_rebuild_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &helper,
        &["--json", "graph", "rebuild"],
    )?;
    serde_json::from_value::<CliGraphRebuildOutput>(output.clone())?;
    assert_eq!(output, fixture("graph-rebuild", "valid")?);
    Ok(())
}

#[test]
fn graph_rebuild_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliGraphRebuildOutput>("graph-rebuild")
}

#[test]
fn producer_graph_status_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_graph_status_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &helper,
        &["--json", "graph", "status"],
    )?;
    serde_json::from_value::<CliGraphStatusOutput>(output.clone())?;
    assert_eq!(output, fixture("graph-status", "valid")?);
    Ok(())
}

#[test]
fn graph_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliGraphStatusOutput>("graph-status")
}

#[test]
fn producer_graph_sync_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_graph_sync_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &helper,
        &["--json", "graph", "sync"],
    )?;
    serde_json::from_value::<CliGraphSyncOutput>(output.clone())?;
    assert_eq!(output, fixture("graph-sync", "valid")?);
    Ok(())
}

#[test]
fn graph_sync_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliGraphSyncOutput>("graph-sync")
}

#[test]
fn producer_vector_configure_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_configure_matches_exact_fixture")?;
    let config = temp.dir.join("vector.toml");
    let output = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "vector",
            "configure",
            "--skip-check",
            "--endpoint",
            "http://127.0.0.1:11434",
            "--model",
            "fixture-model",
            "--dimensions",
            "3",
            "--vector-config",
            config.to_str().context("config path")?,
        ],
        &temp.dir,
        &[],
    )?
    .success_json()?;
    serde_json::from_value::<CliVectorConfigureOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-configure", "valid")?);
    Ok(())
}

#[test]
fn vector_configure_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorConfigureOutput>("vector-configure")
}

#[test]
fn producer_vector_query_chunks_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_query_chunks_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &["--json", "vector", "query-chunks", "fixture query"],
    )?;
    serde_json::from_value::<CliVectorQueryChunksOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-query-chunks", "valid")?);
    Ok(())
}

#[test]
fn vector_query_chunks_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorQueryChunksOutput>("vector-query-chunks")
}

#[test]
fn producer_vector_query_label_atoms_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_query_label_atoms_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &[
            "--json",
            "vector",
            "query-label-atoms",
            "--vector-json",
            "[1.0,0.0]",
            "--include-vector",
        ],
    )?;
    serde_json::from_value::<CliVectorQueryLabelAtomsOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-query-label-atoms", "valid")?);
    Ok(())
}

#[test]
fn vector_query_label_atoms_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorQueryLabelAtomsOutput>("vector-query-label-atoms")
}

#[test]
fn producer_vector_rebuild_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_rebuild_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &["--json", "vector", "rebuild"],
    )?;
    serde_json::from_value::<CliVectorRebuildOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-rebuild", "valid")?);
    Ok(())
}

#[test]
fn vector_rebuild_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorRebuildOutput>("vector-rebuild")
}

#[test]
fn producer_vector_status_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_status_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &["--json", "vector", "status"],
    )?;
    serde_json::from_value::<CliVectorStatusOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-status", "valid")?);
    Ok(())
}

#[test]
fn vector_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorStatusOutput>("vector-status")
}

#[test]
fn producer_vector_sync_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_vector_sync_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &["--json", "vector", "sync"],
    )?;
    serde_json::from_value::<CliVectorSyncOutput>(output.clone())?;
    assert_eq!(output, fixture("vector-sync", "valid")?);
    Ok(())
}

#[test]
fn vector_sync_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliVectorSyncOutput>("vector-sync")
}

#[test]
fn producer_context_build_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_context_build_matches_exact_fixture")?;
    let helper = write_helper(&temp)?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Context fixture subject",
            "--description",
            "ready spec context fixture",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let output = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &helper,
        &["--json", "context", "build", task_id],
    )?;
    serde_json::from_value::<CliContextBuildOutput>(output.clone())?;
    assert_eq!(
        normalize_context(output, task_id)?,
        fixture("context-build", "valid")?
    );
    Ok(())
}

#[test]
fn context_build_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliContextBuildOutput>("context-build")
}

#[test]
fn producer_search_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_search_matches_exact_fixture")?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Helper search target",
            "--description",
            "ready spec helper-search-needle",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let board_id = task["data"]["board_id"].as_str().context("board id")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "not-required",
            task_id,
            "--reason",
            "fixture",
        ],
    )?
    .success()?;
    let output =
        kanban(&temp.path, &["--json", "search", "helper-search-needle"])?.success_json()?;
    serde_json::from_value::<CliSearchOutput>(output.clone())?;
    assert_eq!(
        normalize_search(output, task_id, board_id)?,
        fixture("search", "valid")?
    );
    Ok(())
}

#[test]
fn search_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliSearchOutput>("search")
}

#[test]
fn producer_index_rebuild_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_index_rebuild_matches_exact_fixture")?;
    let output = kanban(&temp.path, &["--json", "index", "rebuild"])?.success_json()?;
    serde_json::from_value::<CliIndexRebuildOutput>(output.clone())?;
    assert_eq!(
        normalize_index_status(
            output,
            "Rebuilt Tantivy task index at ",
            "Rebuilt Tantivy task index at <INDEX_PATH>",
        )?,
        fixture("index-rebuild", "valid")?
    );
    Ok(())
}

#[test]
fn index_rebuild_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliIndexRebuildOutput>("index-rebuild")
}

#[test]
fn producer_index_sync_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("producer_index_sync_matches_exact_fixture")?;
    kanban(&temp.path, &["index", "rebuild"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "index sync fixture",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;
    let output = kanban(&temp.path, &["--json", "index", "sync"])?.success_json()?;
    serde_json::from_value::<CliIndexSyncOutput>(output.clone())?;
    assert_eq!(
        normalize_index_status(
            output,
            "Synced Tantivy task index at ",
            "Synced Tantivy task index at <INDEX_PATH> (<AFFECTED> affected task(s))",
        )?,
        fixture("index-sync", "valid")?
    );
    Ok(())
}

#[test]
fn index_sync_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume_contract::<CliIndexSyncOutput>("index-sync")
}

#[test]
fn helper_contracts_preserve_degraded_error_and_human_behavior() -> anyhow::Result<()> {
    let temp = setup("helper_contracts_preserve_degraded_error_and_human_behavior")?;
    let missing_graph = temp.dir.join("missing-graph-helper");
    let graph = helper_json(
        &temp,
        "KANBAN_GRAPH_HELPER",
        &missing_graph,
        &["--json", "graph", "status"],
    )?;
    serde_json::from_value::<CliGraphStatusOutput>(graph.clone())?;
    anyhow::ensure!(graph["data"]["backend"] == "helper-missing");

    let missing_vector = temp.dir.join("missing-vector-helper");
    let vector = helper_json(
        &temp,
        "KANBAN_VECTOR_HELPER",
        &missing_vector,
        &["--json", "vector", "status"],
    )?;
    serde_json::from_value::<CliVectorStatusOutput>(vector.clone())?;
    anyhow::ensure!(vector["data"]["diagnostics"] == json!(["helper_missing"]));

    kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "neighbors", "kb://task/t_missing"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", missing_graph.as_path())],
    )?
    .json_failure_containing("failed to run graph helper")?;

    let helper = write_helper(&temp)?;
    let human = kanban_in_dir_envs(
        &temp.path,
        &["graph", "neighbors", "kb://task/t_subject"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", helper.as_path())],
    )?
    .success_stdout()?;
    anyhow::ensure!(human.contains("kb://task/t_subject --depends_on--> kb://task/t_parent"));
    Ok(())
}
