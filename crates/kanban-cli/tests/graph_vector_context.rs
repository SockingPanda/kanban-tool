mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir_envs, kanban_in_dir_envs_with_stdin};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn write_executable(path: &std::path::Path, body: &str) -> anyhow::Result<()> {
    std::fs::write(path, body)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[test]
fn substrate_commands_report_entities_outbox_and_derived_status() -> anyhow::Result<()> {
    let temp = TempDb::new("substrate_commands_report_entities_outbox_and_derived_status")?;
    kanban(&temp.path, &["init"])?.success()?;

    let entities = kanban(
        &temp.path,
        &[
            "--json", "entity", "list", "--kind", "board", "--limit", "5",
        ],
    )?
    .success_json()?;
    let entity_rows = entities["data"].as_array().context("expected JSON array")?;
    assert_eq!(entity_rows.len(), 1);
    assert_eq!(entity_rows[0]["kind"], "board");
    let uri = entity_rows[0]["uri"]
        .as_str()
        .context("expected JSON string")?;
    assert!(uri.starts_with("kb://board/"));

    let shown = kanban(&temp.path, &["--json", "entity", "show", uri])?.success_json()?;
    assert_eq!(shown["data"]["uri"], uri);

    let outbox = kanban(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    assert_eq!(
        outbox["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        0
    );

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "substrate task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let task_uri = format!("kb://task/{task_id}");
    let task_entity =
        kanban(&temp.path, &["--json", "entity", "show", &task_uri])?.success_json()?;
    assert_eq!(task_entity["data"]["title"], "substrate task");

    let outbox = kanban(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    let jobs = outbox["data"].as_array().context("expected JSON array")?;
    assert_eq!(jobs.len(), 3);
    let targets = jobs
        .iter()
        .map(|job| job["target"].as_str().context("expected JSON string"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(targets, vec!["tantivy", "oxigraph", "lancedb"]);
    assert!(jobs.iter().all(|job| job["entity_uri"] == task_uri));

    let derived = kanban(&temp.path, &["--json", "derived", "status"])?.success_json()?;
    let stores = derived["data"].as_array().context("expected JSON array")?;
    assert_eq!(stores.len(), 4);
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "tantivy_tasks")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "oxigraph_relations")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_chunks")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_label_atoms" && store["dirty"] == false)
    );
    Ok(())
}

#[test]
fn graph_vector_and_context_commands_report_disabled_fallbacks() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_vector_and_context_commands_report_disabled_fallbacks")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "fallback context source",
            "--description",
            "ready spec context-needle",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let missing_graph_helper = temp.dir.join("missing-graph-helper");
    let missing_vector_helper = temp.dir.join("missing-vector-helper");
    let graph = kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", missing_graph_helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(graph["data"]["backend"], "helper-missing");
    assert_eq!(graph["data"]["enabled"], false);
    assert!(
        graph["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("graph helper unavailable")
    );

    kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "graph",
            "neighbors",
            &format!("kb://task/{task_id}"),
        ],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", missing_graph_helper.as_path())],
    )?
    .json_failure_containing("failed to run graph helper")?;

    let vector = kanban_in_dir_envs(
        &temp.path,
        &["--json", "vector", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", missing_vector_helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(vector["data"]["backend"], "helper-missing");
    assert_eq!(vector["data"]["enabled"], false);
    assert!(
        vector["data"]["diagnostics"]
            .as_array()
            .context("expected diagnostics array")?
            .iter()
            .any(|value| value == "helper_missing")
    );

    let context = kanban(
        &temp.path,
        &[
            "--json",
            "context",
            "build",
            task_id,
            "--lexical-limit",
            "3",
        ],
    )?
    .success_json()?;
    assert_eq!(context["data"]["subject"], format!("kb://task/{task_id}"));
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "graph_disabled")
    );
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    assert_eq!(
        context["data"]["items"][0]["entity_uri"],
        format!("kb://task/{task_id}")
    );
    assert_eq!(context["data"]["items"][0]["source"], "subject");
    assert!(
        context["data"]["items"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|item| item["entity_uri"] == format!("kb://task/{task_id}"))
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn context_build_uses_vector_helper_chunks_when_available() -> anyhow::Result<()> {
    let temp = TempDb::new("context_build_uses_vector_helper_chunks_when_available")?;
    kanban(&temp.path, &["init"])?.success()?;
    let subject = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "subject context helper",
            "--description",
            "needs helper context",
        ],
    )?
    .success_json()?;
    let subject_id = subject["data"]["id"].as_str().context("subject id")?;
    let related = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "vector helper related",
            "--description",
            "vector-only context item",
        ],
    )?
    .success_json()?;
    let related_id = related["data"]["id"].as_str().context("related id")?;
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
cmd = args[0]
if cmd == "status":
    payload = {{"backend":"test-vector-helper","enabled":True,"message":"ok","diagnostics":[],"dirty":False,"board_dirty":False}}
elif cmd == "query-chunks":
    payload = [{{
        "chunk": {{"uri":"kb://chunk/task/{related_id}/0","entity_uri":"kb://task/{related_id}","ordinal":0,"content_hash":"hash"}},
        "score": 0.91,
        "text": "vector-only context item",
        "summary": "vector helper related"
    }}]
else:
    payload = []
print(json.dumps({{"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}}))
"#
        ),
    )?;

    let context = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "context",
            "build",
            subject_id,
            "--vector-limit",
            "2",
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    assert!(
        context["data"]["items"]
            .as_array()
            .context("items")?
            .iter()
            .any(
                |item| item["entity_uri"] == format!("kb://task/{related_id}")
                    && item["source"] == "vector"
            )
    );
    assert!(
        !context["data"]["degraded"]
            .as_array()
            .context("degraded")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn vector_query_label_atoms_supports_raw_vector_helper_query() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_query_label_atoms_supports_raw_vector_helper_query")?;
    kanban(&temp.path, &["init"])?.success()?;
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
cmd = args[0]
if cmd == "query-label-atoms":
    assert "--vector-json" in args, args
    assert args[args.index("--vector-json") + 1] == "[1.0,0.0]", args
    assert args[args.index("--embedding-model") + 1] == "review-model", args
    assert args[args.index("--polarity") + 1] == "positive", args
    assert "--include-vector" in args, args
    payload = [{"hit": {
        "atom_id":"atom_backend_positive",
        "label_id":"label_backend",
        "label_name":"backend",
        "board_id":"b_default",
        "polarity":"positive",
        "kind":"applies_when",
        "text":"touches rust service code",
        "ordinal":0,
        "content_hash":"hash",
        "embedding_model":"review-model",
        "distance":0.0
    }, "vector": [1.0, 0.0]}]
else:
    payload = []
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;

    let hits = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "vector",
            "query-label-atoms",
            "--vector-json",
            "[1.0,0.0]",
            "--include-vector",
            "--embedding-model",
            "review-model",
            "--polarity",
            "positive",
            "--limit",
            "2",
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    assert_eq!(hits["data"][0]["hit"]["label_name"], "backend");
    assert_eq!(hits["data"][0]["vector"], serde_json::json!([1.0, 0.0]));
    Ok(())
}

#[test]
fn vector_query_label_atoms_requires_exactly_one_input() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_query_label_atoms_requires_exactly_one_input")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(&temp.path, &["vector", "query-label-atoms"])?.failure_containing("required")?;
    kanban(
        &temp.path,
        &[
            "vector",
            "query-label-atoms",
            "inline text",
            "--text-file",
            "query.txt",
        ],
    )?
    .failure_containing("cannot be used with")?;
    kanban(
        &temp.path,
        &[
            "vector",
            "query-label-atoms",
            "--vector-json",
            "[1.0]",
            "--vector-json-file",
            "vector.json",
        ],
    )?
    .failure_containing("cannot be used with")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn vector_query_label_atoms_accepts_file_and_stdin_inputs() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_query_label_atoms_accepts_file_and_stdin_inputs")?;
    kanban(&temp.path, &["init"])?.success()?;
    let query_path = temp.dir.join("label-query.txt");
    std::fs::write(&query_path, "touches $RUST and `service`\n")?;
    let log = temp.dir.join("vector-helper-calls.jsonl");
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        &format!(
            r#"#!/usr/bin/env python3
import json, pathlib, sys
log = pathlib.Path({:?})
args = sys.argv[1:]
with log.open("a") as handle:
    handle.write(json.dumps(args) + "\n")
cmd = args[0]
if cmd != "query-label-atoms":
    raise SystemExit("unexpected command " + cmd)
payload = [{{
    "atom_id":"atom_backend_positive",
    "label_id":"label_backend",
    "label_name":"backend",
    "board_id":"b_default",
    "polarity":"positive",
    "kind":"applies_when",
    "text":"touches rust service code",
    "ordinal":0,
    "content_hash":"hash",
    "embedding_model":"review-model",
    "distance":0.0
}}]
print(json.dumps({{"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}}))
"#,
            log.display().to_string()
        ),
    )?;

    kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "vector",
            "query-label-atoms",
            "--text-file",
            query_path.to_str().context("query path")?,
            "--limit",
            "3",
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    kanban_in_dir_envs_with_stdin(
        &temp.path,
        &[
            "--json",
            "vector",
            "query-label-atoms",
            "--vector-json-file",
            "-",
            "--include-vector",
        ],
        "[0.25,0.75]",
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    let calls = std::fs::read_to_string(&log)?;
    let parsed_calls = calls
        .lines()
        .map(serde_json::from_str::<Vec<String>>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(parsed_calls.len(), 2, "{calls}");
    let text_value_index = parsed_calls[0]
        .iter()
        .position(|arg| arg == "--text")
        .context("--text")?
        + 1;
    assert_eq!(
        parsed_calls[0][text_value_index],
        "touches $RUST and `service`\n"
    );
    let vector_value_index = parsed_calls[1]
        .iter()
        .position(|arg| arg == "--vector-json")
        .context("--vector-json")?
        + 1;
    assert_eq!(parsed_calls[1][vector_value_index], "[0.25,0.75]");
    assert!(
        parsed_calls[1].iter().any(|arg| arg == "--include-vector"),
        "{calls}"
    );
    Ok(())
}

#[test]
fn vector_status_reports_invalid_helper_json_as_degraded() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_status_reports_invalid_helper_json_as_degraded")?;
    kanban(&temp.path, &["init"])?.success()?;
    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "vector", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", std::path::Path::new("/bin/echo"))],
    )?
    .success_json()?;
    assert_eq!(status["data"]["backend"], "helper-invalid");
    assert_eq!(status["data"]["enabled"], false);
    assert!(
        status["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("invalid JSON envelope")
    );
    assert!(
        status["data"]["diagnostics"]
            .as_array()
            .context("expected diagnostics array")?
            .iter()
            .any(|value| value == "helper_invalid_envelope")
    );
    Ok(())
}

#[test]
fn graph_status_reports_invalid_helper_json_as_degraded() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_status_reports_invalid_helper_json_as_degraded")?;
    kanban(&temp.path, &["init"])?.success()?;
    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", std::path::Path::new("/bin/echo"))],
    )?
    .success_json()?;
    assert_eq!(status["data"]["backend"], "helper-invalid");
    assert_eq!(status["data"]["enabled"], false);
    assert!(
        status["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("invalid JSON envelope")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn graph_status_preserves_helper_error_envelope() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_status_preserves_helper_error_envelope")?;
    kanban(&temp.path, &["init"])?.success()?;
    let helper = temp.dir.join("helper-error.sh");
    std::fs::write(
        &helper,
        r#"#!/usr/bin/env bash
printf '%s\n' '{"protocol":"kanban-derived-helper.v1","payload_json":"{\"code\":\"bad_board\",\"message\":\"bad board\"}"}'
exit 1
"#,
    )?;
    let mut perms = std::fs::metadata(&helper)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&helper, perms)?;

    kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", helper.as_path())],
    )?
    .json_failure_containing("graph helper failed: bad board (bad_board)")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn graph_query_sparql_file_preserves_shell_sensitive_query() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_query_sparql_file_preserves_shell_sensitive_query")?;
    kanban(&temp.path, &["init"])?.success()?;
    let calls_path = temp.dir.join("graph-helper-calls.json");
    let helper = temp.dir.join("graph-helper.sh");
    write_executable(
        &helper,
        &format!(
            r#"#!/usr/bin/env bash
python3 - "$@" <<'PY'
import json
import pathlib
import sys
pathlib.Path({calls_path:?}).write_text(json.dumps(sys.argv[1:]))
print('{{"protocol":"kanban-derived-helper.v1","payload_json":"[]"}}')
PY
"#,
            calls_path = calls_path.to_string_lossy()
        ),
    )?;
    let sparql = "SELECT ?task WHERE {\n  ?task ?p \"literal $VAR $(date) `code`\" .\n}";
    let sparql_path = temp.dir.join("query.sparql");
    std::fs::write(&sparql_path, sparql)?;

    let rows = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "graph",
            "query",
            "--sparql-file",
            sparql_path.to_str().context("sparql path")?,
        ],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(rows["data"].as_array().context("rows")?.len(), 0);

    let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&calls_path)?)?;
    assert_eq!(
        args,
        vec![
            "query".to_owned(),
            "--sparql".to_owned(),
            sparql.to_owned(),
            "--limit".to_owned(),
            "50".to_owned(),
            "--db".to_owned(),
            temp.path.to_string_lossy().into_owned(),
            "--board".to_owned(),
            "default".to_owned(),
        ]
    );
    Ok(())
}

#[test]
fn context_build_command_rejects_zero_max_items() -> anyhow::Result<()> {
    let temp = TempDb::new("context_build_command_rejects_zero_max_items")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "zero budget context",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    kanban(
        &temp.path,
        &["context", "build", task_id, "--max-items", "0"],
    )?
    .failure_containing("max_items must be >= 1")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn label_atom_index_commands_use_label_atom_helper_commands() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_index_commands_use_label_atom_helper_commands")?;
    kanban(&temp.path, &["init"])?.success()?;
    let vector_config = temp.dir.join("vector.toml");
    std::fs::write(
        &vector_config,
        r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "label-helper-test"
dimensions = 3
"#,
    )?;
    let log = temp.dir.join("label-helper-calls.jsonl");
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        &format!(
            r#"#!/usr/bin/env python3
import json, pathlib, sys
log = pathlib.Path({:?})
args = sys.argv[1:]
with log.open("a") as handle:
    handle.write(json.dumps(args) + "\n")
cmd = args[0]
if cmd == "status":
    raise SystemExit("chunk status must not be used for label atom status")
if cmd == "label-atoms-status":
    payload = {{"backend":"label-helper","enabled":True,"message":"label status","diagnostics":["label_atom_helper"],"dirty":False,"board_dirty":False}}
elif cmd == "rebuild-label-atoms":
    payload = {{"backend":"label-helper","enabled":True,"message":"rebuilt labels","diagnostics":["label_atom_helper"],"dirty":False,"board_dirty":False}}
elif cmd == "query-label-atoms":
    payload = [{{
        "atom_id":"atom_backend_positive",
        "label_id":"label_backend",
        "label_name":"backend",
        "board_id":"b_default",
        "polarity":"positive",
        "kind":"applies_when",
        "text":"touches rust service code",
        "ordinal":0,
        "content_hash":"hash",
        "embedding_model":"review-model",
        "distance":0.0
    }}]
else:
    raise SystemExit("unexpected command " + cmd)
print(json.dumps({{"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}}))
"#,
            log.display().to_string()
        ),
    )?;

    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "label", "atom-index", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(status["data"]["backend"], "label-helper");

    let rebuilt_without_config = kanban_in_dir_envs(
        &temp.path,
        &["--json", "label", "atom-index", "rebuild"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(rebuilt_without_config["data"]["message"], "rebuilt labels");

    let query_without_config = kanban_in_dir_envs(
        &temp.path,
        &["--json", "label", "atom-index", "query", "backend"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(query_without_config["data"][0]["label_name"], "backend");

    let rebuilt_with_config = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "label",
            "atom-index",
            "rebuild",
            "--vector-config",
            vector_config.to_str().context("vector config")?,
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(rebuilt_with_config["data"]["message"], "rebuilt labels");

    let query_with_config = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "label",
            "atom-index",
            "query",
            "backend",
            "--vector-config",
            vector_config.to_str().context("vector config")?,
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(query_with_config["data"][0]["label_name"], "backend");

    let calls = std::fs::read_to_string(&log)?;
    let parsed_calls = calls
        .lines()
        .map(serde_json::from_str::<Vec<String>>)
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        calls
            .lines()
            .any(|line| line.contains("label-atoms-status")),
        "{calls}"
    );
    assert!(
        calls
            .lines()
            .any(|line| line.contains("rebuild-label-atoms")),
        "{calls}"
    );
    let rebuild_calls: Vec<_> = parsed_calls
        .iter()
        .filter(|args| args.first().is_some_and(|arg| arg == "rebuild-label-atoms"))
        .collect();
    assert_eq!(rebuild_calls.len(), 2, "{calls}");
    assert!(
        !rebuild_calls[0].iter().any(|arg| arg == "--vector-config"),
        "{calls}"
    );
    assert!(
        rebuild_calls[1].iter().any(|arg| arg == "--vector-config"),
        "{calls}"
    );
    let query_calls: Vec<_> = parsed_calls
        .iter()
        .filter(|args| args.first().is_some_and(|arg| arg == "query-label-atoms"))
        .collect();
    assert_eq!(query_calls.len(), 2, "{calls}");
    assert!(
        !query_calls[0].iter().any(|arg| arg == "--vector-config"),
        "{calls}"
    );
    assert!(
        query_calls[1].iter().any(|arg| arg == "--vector-config"),
        "{calls}"
    );
    assert!(
        !calls.lines().any(|line| line.contains("\"status\"")),
        "{calls}"
    );
    Ok(())
}
