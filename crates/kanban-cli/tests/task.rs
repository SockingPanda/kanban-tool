mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir_envs, kanban_with_stdin};
use kanban_sqlite::api::{
    CreateLabel, CreateTask, LabelOntologyActionInput, LabelOntologyActionType, LabelOntologyActor,
    LabelOntologyAtomApplyInput, LabelOntologyCandidateAtomInput, LabelOntologyProposedAction,
    LabelOntologyRecordInput, LabelOntologySignalInput, LabelOntologySignalKind,
    LabelOntologySuggestState, LabelProposalCandidate, UpsertLabelSemantics,
    complete_task_with_summary_and_result, get_label_semantics, get_task, list_label_atoms,
    list_labels,
};
use pretty_assertions::assert_eq;
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path};

#[cfg(unix)]
fn write_executable(path: &Path, body: &str) -> anyhow::Result<()> {
    fs::write(path, body)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn mark_no_plan_required(db_path: &Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::api::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-task-test",
        task_id,
        "task cli fixture does not need steps",
    )?;
    Ok(())
}

#[test]
fn task_show_defaults_to_one_line_summary() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_defaults_to_one_line_summary")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "show summary title",
            "--description",
            "line one\nline two",
            "--assignee",
            "operator",
            "--priority",
            "2",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, task_id)?;

    let stdout = kanban(&temp.path, &["task", "show", task_id])?.success_stdout()?;

    assert_eq!(
        stdout,
        "default#1 [ready] P2 show summary title · plan: not_required · steps: 0/0\n"
    );
    assert!(!stdout.contains(task_id), "{stdout}");
    assert!(!stdout.contains("line one"), "{stdout}");
    assert!(!stdout.contains("plan="), "{stdout}");
    assert!(!stdout.contains("steps="), "{stdout}");
    assert_eq!(stdout.lines().count(), 1);
    Ok(())
}

#[test]
fn task_list_defaults_to_human_summary_without_internal_ids() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_defaults_to_human_summary_without_internal_ids")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "list summary title",
            "--description",
            "list details stay out of the summary",
            "--priority",
            "1",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, task_id)?;

    let stdout = kanban(&temp.path, &["task", "list"])?.success_stdout()?;

    assert_eq!(
        stdout,
        "default#1 [ready] P1 list summary title · plan: not_required · steps: 0/0\n"
    );
    assert!(!stdout.contains(task_id), "{stdout}");
    assert!(!stdout.contains("description"), "{stdout}");
    assert!(!stdout.contains("plan="), "{stdout}");
    assert!(!stdout.contains("steps="), "{stdout}");
    Ok(())
}

#[test]
fn task_show_details_prints_grouped_readable_record() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_prints_grouped_readable_record")?;
    kanban(&temp.path, &["init"])?.success()?;
    let description = "first detail line\nsecond detail line";
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "detailed task title",
            "--description",
            description,
            "--assignee",
            "executor",
            "--priority",
            "1",
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, task_id)?;

    let stdout = kanban(&temp.path, &["task", "show", task_id, "--details"])?.success_stdout()?;

    assert!(stdout.contains("Task\n"), "{stdout}");
    assert!(stdout.contains("  ref: default#1"), "{stdout}");
    assert!(stdout.contains(&format!("  id: {task_id}")), "{stdout}");
    assert!(stdout.contains("  status: ready"), "{stdout}");
    assert!(stdout.contains("  title: detailed task title"), "{stdout}");
    assert!(stdout.contains("  labels: -"), "{stdout}");
    assert!(stdout.contains("  assignee: executor"), "{stdout}");
    assert!(stdout.contains("  priority: P1"), "{stdout}");
    assert!(stdout.contains("Plan\n  state: not_required"), "{stdout}");
    assert!(stdout.contains("  required_steps: 0/0"), "{stdout}");
    assert!(stdout.contains("  optional_steps: 0"), "{stdout}");
    assert!(stdout.contains("Schedule\n"), "{stdout}");
    assert!(stdout.contains("  scheduled_at: 1767225600000"), "{stdout}");
    assert!(stdout.contains("  due_at: 1767312000000"), "{stdout}");
    assert!(stdout.contains("Timestamps\n"), "{stdout}");
    assert!(stdout.contains("  created_at: "), "{stdout}");
    assert!(stdout.contains("  updated_at: "), "{stdout}");
    assert!(stdout.contains("Execution\n"), "{stdout}");
    assert!(stdout.contains("Metadata\n"), "{stdout}");
    assert!(
        stdout.contains("Description\n  first detail line\n  second detail line"),
        "{stdout}"
    );
    assert!(!stdout.contains("execution_plan_state:"), "{stdout}");
    Ok(())
}

#[test]
fn task_reopen_requires_reason_and_returns_reopened_task() -> anyhow::Result<()> {
    let temp = TempDb::new("task_reopen_requires_reason_and_returns_reopened_task")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "reopen cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    mark_no_plan_required(&temp.path, task_id)?;
    let claim = kanban(&temp.path, &["--json", "task", "claim", task_id])?.success_json()?;
    let token = claim["data"]["claim_token"]
        .as_str()
        .context("claim token")?;
    complete_task_with_summary_and_result(
        &temp.path,
        "default",
        "cli-task-test",
        task_id,
        Some(token),
        false,
        Some("done once"),
        Some(r#"{"cli":true}"#),
    )?;

    kanban(&temp.path, &["task", "reopen", task_id])?.failure_containing("required")?;
    let reopened = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "reopen",
            task_id,
            "--reason",
            "operator retry",
        ],
    )?
    .success_json()?;

    assert_eq!(reopened["data"]["status"], "ready");
    assert_eq!(reopened["data"]["completed_at"], serde_json::Value::Null);
    assert_eq!(reopened["data"]["result_summary"], "done once");
    assert_eq!(reopened["data"]["result"], json!({"cli":true}));
    Ok(())
}

#[test]
fn task_create_label_requires_existing_vocabulary() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_label_requires_existing_vocabulary")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "must not create missing label",
            "--description",
            "ready spec",
            "--label",
            "missing",
        ],
    )?
    .failure_containing("label missing does not exist")?;

    let failed = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "json missing label",
            "--description",
            "ready spec",
            "--label",
            "missing",
        ],
    )?;
    assert!(!failed.output.status.success());
    assert!(failed.output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&failed.output.stdout)
        .context("failed to parse JSON error stdout")?;
    let message = json["error"]["message"].as_str().context("error message")?;
    assert!(message.contains("label missing does not exist"));
    assert!(message.contains("label add --create-missing"));
    assert!(!message.contains("pass --create-missing"));

    let tasks = kanban(&temp.path, &["--json", "task", "list"])?.success_json()?;
    assert!(tasks["data"].as_array().context("tasks")?.is_empty());
    let labels = kanban(&temp.path, &["--json", "label", "list"])?.success_json()?;
    assert!(labels["data"].as_array().context("labels")?.is_empty());
    Ok(())
}

#[test]
fn task_create_and_label_commands_round_trip_labels() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_and_label_commands_round_trip_labels")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "labeled cli task",
            "--description",
            "ready spec",
            "--label",
            "backend",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    assert_eq!(created["data"]["labels"][0]["name"], "backend");

    let label = kanban(
        &temp.path,
        &[
            "--json", "label", "create", "frontend", "--color", "#4477aa",
        ],
    )?
    .success_json()?;
    assert_eq!(label["data"]["name"], "frontend");

    let added_existing =
        kanban(&temp.path, &["--json", "label", "add", task_id, "frontend"])?.success_json()?;
    assert_eq!(
        added_existing["data"]["labels"]
            .as_array()
            .context("labels")?
            .len(),
        2
    );

    kanban(&temp.path, &["label", "add", task_id, "api"])?
        .failure_containing("label api does not exist")?;
    let listed_before_create = kanban(&temp.path, &["--json", "label", "list"])?.success_json()?;
    assert_eq!(
        listed_before_create["data"]
            .as_array()
            .context("labels before create")?
            .len(),
        2
    );

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "add",
            "--create-missing",
            task_id,
            "api",
            "frontend",
        ],
    )?
    .success_json()?;
    assert_eq!(added["data"]["created_labels"][0]["name"], "api");
    assert_eq!(
        added["data"]["task"]["labels"]
            .as_array()
            .context("labels")?
            .len(),
        3
    );

    let listed = kanban(&temp.path, &["--json", "label", "list"])?.success_json()?;
    let names: Vec<_> = listed["data"]
        .as_array()
        .context("labels")?
        .iter()
        .map(|label| label["name"].clone())
        .collect();
    assert_eq!(
        names,
        [
            serde_json::json!("api"),
            serde_json::json!("backend"),
            serde_json::json!("frontend")
        ]
    );
    let listed_human = kanban(&temp.path, &["label", "list"])?.success_stdout()?;
    assert!(listed_human.contains("backend "), "{listed_human}");
    assert!(listed_human.contains(" color=-"), "{listed_human}");
    assert!(listed_human.contains("frontend "), "{listed_human}");
    assert!(listed_human.contains(" color=#4477aa"), "{listed_human}");

    let human = kanban(&temp.path, &["task", "show", task_id])?.success_stdout()?;
    assert!(human.contains("[api,backend,frontend]"), "{human}");

    let removed = kanban(
        &temp.path,
        &["--json", "label", "remove", task_id, "frontend"],
    )?
    .success_json()?;
    assert_eq!(removed["data"]["labels"][0]["name"], "api");
    assert_eq!(removed["data"]["labels"][1]["name"], "backend");
    Ok(())
}

#[test]
fn label_suggest_returns_degraded_json_without_vector_provider() -> anyhow::Result<()> {
    let temp = TempDb::new("label_suggest_returns_degraded_json_without_vector_provider")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "suggestion cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    let suggestions = kanban(
        &temp.path,
        &["--json", "label", "suggest", task_id, "--limit", "3"],
    )?
    .success_json()?;

    assert_eq!(suggestions["data"]["task_id"], task_id);
    assert_eq!(suggestions["data"]["degraded"], true);
    assert_eq!(suggestions["data"]["needs_new_label"], false);
    assert_eq!(
        suggestions["data"]["reason_codes"],
        json!(["degraded_result", "vector_store_disabled"])
    );
    assert!(
        suggestions["data"]["selected_labels"]
            .as_array()
            .context("selected labels")?
            .is_empty()
    );
    assert!(
        suggestions["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .iter()
            .any(|value| value == "vector_store_disabled")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn label_suggest_uses_vector_helper_adapter_successfully() -> anyhow::Result<()> {
    let temp = TempDb::new("label_suggest_uses_vector_helper_adapter_successfully")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "backend helper suggestion",
            "--description",
            "touches rust service code",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
cmd = args[0]
if cmd in ("status", "label-atoms-status"):
    payload = {"backend":"test-vector-helper","enabled":True,"message":"ok","diagnostics":[],"dirty":False,"board_dirty":False}
elif cmd == "embed-query":
    payload = [1.0, 0.0]
elif cmd == "query-label-atoms":
    model = args[args.index("--embedding-model") + 1] if "--embedding-model" in args else ""
    if model != "review-model":
        print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps({"code":"unexpected_model","message":"expected review-model, got " + model})}))
        sys.exit(1)
    polarity = args[args.index("--polarity") + 1] if "--polarity" in args else "positive"
    if polarity == "positive":
        hit = {
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
            "distance":0.0,
        }
        payload = [{"hit": hit, "vector": [1.0, 0.0]}] if "--include-vector" in args else [hit]
    else:
        payload = []
else:
    payload = []
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;
    let vector_config = temp.dir.join("vector.toml");
    std::fs::write(
        &vector_config,
        r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "review-model"
dimensions = 2
"#,
    )?;

    let suggestions = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "label",
            "suggest",
            task_id,
            "--limit",
            "3",
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    assert_eq!(suggestions["data"]["degraded"], false);
    assert_eq!(
        suggestions["data"]["selected_labels"][0]["label_name"],
        "backend"
    );
    assert!(
        !suggestions["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .iter()
            .any(|value| value == "vector_store_disabled")
    );
    Ok(())
}

#[test]
fn label_semantics_and_atoms_commands_round_trip_json() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_and_atoms_commands_round_trip_json")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;

    let semantics = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "semantics",
            "upsert",
            "backend",
            "--description",
            "Backend service work",
            "--applies-when",
            "touches Rust service code",
            "--excludes-when",
            "CSS-only",
            "--positive-example",
            "add API handler",
            "--negative-example",
            "adjust spacing",
        ],
    )?
    .success_json()?;

    assert_eq!(semantics["data"]["label_name"], "backend");
    assert_eq!(semantics["data"]["description"], "Backend service work");
    assert_eq!(
        semantics["data"]["applies_when"],
        json!(["touches Rust service code"])
    );
    assert_eq!(semantics["data"]["excludes_when"], json!(["CSS-only"]));
    assert!(
        semantics["data"]["atoms"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["polarity"] == "negative" && atom["text"] == "CSS-only")
    );

    let listed = kanban(&temp.path, &["--json", "label", "semantics", "list"])?.success_json()?;
    assert_eq!(listed["data"].as_array().context("semantics")?.len(), 1);

    let shown = kanban(
        &temp.path,
        &["--json", "label", "semantics", "show", "backend"],
    )?
    .success_json()?;
    assert_eq!(shown["data"]["label_name"], "backend");
    let seed_hash = shown["data"]["semantics_hash"]
        .as_str()
        .context("semantics hash")?
        .to_owned();

    let patched = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "semantics",
            "upsert",
            "backend",
            "--expected-semantics-hash",
            &seed_hash,
            "--applies-when",
            "emits machine-readable CLI output",
            "--remove-excludes-when",
            "CSS-only",
            "--reason",
            "CLI patch guardrail test",
        ],
    )?
    .success_json()?;
    assert_eq!(
        patched["data"]["applies_when"],
        json!([
            "touches Rust service code",
            "emits machine-readable CLI output"
        ])
    );
    assert_eq!(patched["data"]["excludes_when"], json!([]));
    assert_eq!(
        patched["data"]["positive_examples"],
        json!(["add API handler"])
    );
    assert_eq!(
        patched["data"]["negative_examples"],
        json!(["adjust spacing"])
    );
    let patched_hash = patched["data"]["semantics_hash"]
        .as_str()
        .context("patched semantics hash")?
        .to_owned();
    kanban(
        &temp.path,
        &[
            "label",
            "semantics",
            "upsert",
            "backend",
            "--expected-semantics-hash",
            &seed_hash,
            "--applies-when",
            "stale writer addition",
        ],
    )?
    .failure_containing("hash mismatch")?;

    let replaced = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "semantics",
            "upsert",
            "backend",
            "--expected-semantics-hash",
            &patched_hash,
            "--replace",
            "--description",
            "Backend replacement semantics",
        ],
    )?
    .success_json()?;
    assert_eq!(
        replaced["data"]["description"],
        "Backend replacement semantics"
    );
    assert_eq!(replaced["data"]["applies_when"], json!([]));
    assert_eq!(replaced["data"]["positive_examples"], json!([]));
    let replacement_hash = replaced["data"]["semantics_hash"]
        .as_str()
        .context("replacement semantics hash")?;

    let atoms = kanban(&temp.path, &["--json", "label", "atoms", "list"])?.success_json()?;
    assert!(
        atoms["data"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["kind"] == "description"
                && atom["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("Backend replacement semantics")))
    );

    let unavailable_helper = temp.dir.join("missing-vector-helper");
    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "label", "atom-index", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", unavailable_helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(status["data"]["enabled"], false);

    let vector_config = temp.dir.join("vector.toml");
    std::fs::write(
        &vector_config,
        r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "offline-cli-test-model"
dimensions = 3
"#,
    )?;
    kanban_in_dir_envs(
        &temp.path,
        &[
            "label",
            "atom-index",
            "rebuild",
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", unavailable_helper.as_path())],
    )?
    .failure_containing("vector helper unavailable")?;
    kanban_in_dir_envs(
        &temp.path,
        &[
            "label",
            "atom-index",
            "query",
            "backend",
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", unavailable_helper.as_path())],
    )?
    .failure_containing("vector helper unavailable")?;

    kanban(
        &temp.path,
        &[
            "label",
            "semantics",
            "delete",
            "backend",
            "--expected-semantics-hash",
            replacement_hash,
        ],
    )?
    .failure_containing("required")?;
    kanban(
        &temp.path,
        &[
            "label",
            "semantics",
            "delete",
            "backend",
            "--expected-semantics-hash",
            "not-the-current-semantics-hash",
            "--reason",
            "Stale clear should fail",
        ],
    )?
    .failure_containing("hash mismatch")?;
    let atoms_before_delete =
        kanban(&temp.path, &["--json", "label", "atoms", "list"])?.success_json()?;
    assert!(
        !atoms_before_delete["data"]
            .as_array()
            .context("atoms before delete")?
            .is_empty()
    );

    let deleted = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "semantics",
            "delete",
            "backend",
            "--expected-semantics-hash",
            replacement_hash,
            "--reason",
            "Clear backend semantics in CLI round trip",
        ],
    )?
    .success_json()?;
    assert_eq!(deleted["data"]["deleted"], true);
    let atoms_after = kanban(&temp.path, &["--json", "label", "atoms", "list"])?.success_json()?;
    assert!(atoms_after["data"].as_array().context("atoms")?.is_empty());
    Ok(())
}

#[test]
fn label_bootstrap_command_attaches_task_and_returns_semantics() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_command_attaches_task_and_returns_semantics")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bootstrap cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    let bootstrapped = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "bootstrap",
            task_id,
            "database",
            "--description",
            "Database persistence work",
            "--applies-when",
            "touches SQLite migrations",
            "--positive-example",
            "new table migration",
        ],
    )?
    .success_json()?;

    assert_eq!(bootstrapped["data"]["task"]["id"], task_id);
    assert_eq!(
        bootstrapped["data"]["verification"],
        serde_json::Value::Null
    );
    assert_eq!(
        bootstrapped["data"]["task"]["labels"][0]["name"],
        "database"
    );
    assert_eq!(bootstrapped["data"]["semantics"]["label_name"], "database");
    assert_eq!(
        bootstrapped["data"]["semantics"]["description"],
        "Database persistence work"
    );
    assert!(
        bootstrapped["data"]["semantics"]["atoms"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["kind"] == "applies_when")
    );
    Ok(())
}

#[test]
fn label_bootstrap_verify_requires_vector_provider_before_mutating() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_verify_requires_vector_provider_before_mutating")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bootstrap verify cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    kanban(
        &temp.path,
        &[
            "label",
            "bootstrap",
            task_id,
            "database",
            "--description",
            "Database persistence work",
            "--applies-when",
            "touches SQLite migrations",
            "--positive-example",
            "new table migration",
            "--verify",
        ],
    )?
    .failure_containing("vector helper unavailable for bootstrap verification")?;

    let shown = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(
        shown["data"]["labels"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn label_delete_force_removes_canonical_label_and_task_bindings() -> anyhow::Result<()> {
    let temp = TempDb::new("label_delete_force_removes_canonical_label_and_task_bindings")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "delete label cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let bootstrapped = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "bootstrap",
            task_id,
            "database",
            "--description",
            "Database persistence work",
            "--positive-example",
            "new table migration",
        ],
    )?
    .success_json()?;
    let semantics_hash = bootstrapped["data"]["semantics"]["semantics_hash"]
        .as_str()
        .context("semantics hash")?;

    kanban(&temp.path, &["label", "delete", "database"])?
        .failure_containing("attached to 1 task(s)")?;
    kanban(&temp.path, &["label", "delete", "database", "--force"])?
        .failure_containing("has semantics or atoms")?;

    kanban(
        &temp.path,
        &[
            "label",
            "semantics",
            "delete",
            "database",
            "--expected-semantics-hash",
            semantics_hash,
            "--reason",
            "Clear semantics before deleting label identity",
        ],
    )?
    .success()?;

    let deleted = kanban(
        &temp.path,
        &["--json", "label", "delete", "database", "--force"],
    )?
    .success_json()?;
    assert_eq!(deleted["data"]["label"]["name"], "database");
    assert_eq!(deleted["data"]["forced"], true);
    assert_eq!(deleted["data"]["removed_task_bindings"], 1);
    assert_eq!(deleted["data"]["removed_semantics"], false);
    assert_eq!(deleted["data"]["removed_atoms"], 0);

    let labels = kanban(&temp.path, &["--json", "label", "list"])?.success_json()?;
    assert!(labels["data"].as_array().context("labels")?.is_empty());
    let shown = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(
        shown["data"]["labels"]
            .as_array()
            .context("task labels")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn label_propose_without_provider_returns_degraded_without_polluting_labels() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_propose_without_provider_returns_degraded_without_polluting_labels")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "proposal degraded cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    let attempt = kanban(&temp.path, &["--json", "label", "propose", task_id])?.success_json()?;

    assert_eq!(attempt["data"]["proposal"], serde_json::Value::Null);
    assert_eq!(attempt["data"]["degraded"], true);
    assert!(
        attempt["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .iter()
            .any(|value| value == "label_proposal_provider_unavailable")
    );
    let labels = kanban(&temp.path, &["--json", "label", "list"])?.success_json()?;
    assert!(labels["data"].as_array().context("labels")?.is_empty());
    Ok(())
}

#[test]
fn metadata_label_proposal_candidate_input_fixture_is_consumed_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("label_propose_with_proposal_json_degrades_without_polluting_truth")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "proposal json degraded cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let proposal_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json"
    );

    let attempt = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "propose",
            task_id,
            "--proposal-json",
            proposal_path,
        ],
    )?
    .success_json()?;

    assert_eq!(attempt["data"]["proposal"], serde_json::Value::Null);
    assert_eq!(attempt["data"]["degraded"], true);
    let diagnostics = attempt["data"]["diagnostics"]
        .as_array()
        .context("diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|value| value == "label_proposal_residual_validation_unavailable"),
        "{diagnostics:?}"
    );
    let conn = kanban_test_support::connect_file(&temp.path)?;
    let proposal_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_semantic_proposals", [], |row| {
            row.get(0)
        })?;
    assert_eq!(proposal_count, 0);
    assert!(
        kanban(&temp.path, &["--json", "label", "list"])?.success_json()?["data"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn label_bootstrap_verify_uses_vector_helper_adapter_successfully() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_verify_uses_vector_helper_adapter_successfully")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bootstrap helper task",
            "--description",
            "touches rust service code",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
cmd = args[0]
if cmd in ("status", "label-atoms-status"):
    payload = {"backend":"test-vector-helper","enabled":True,"message":"ok","diagnostics":[],"dirty":False,"board_dirty":False}
elif cmd == "embed-query":
    payload = [1.0, 0.0]
else:
    payload = []
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;

    let result = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "label",
            "bootstrap",
            task_id,
            "backend",
            "--description",
            "Backend work",
            "--applies-when",
            "touches rust service code",
            "--verify",
            "--min-verify-score",
            "0.0",
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    assert_eq!(result["data"]["verification"]["label_name"], "backend");
    assert_eq!(result["data"]["task"]["labels"][0]["name"], "backend");
    Ok(())
}

#[test]
fn label_proposals_json_accept_reject_list_show_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposals_json_accept_reject_list_show_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "proposal json cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let proposal_id = seed_proposed_label_proposal(
        &temp.path,
        task_id,
        LabelProposalCandidate {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            excludes_when: vec!["UI-only polish".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            negative_examples: vec!["CSS tweak".to_owned()],
        },
    )?;

    let listed = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "proposals",
            "list",
            "--status",
            "proposed",
        ],
    )?
    .success_json()?;
    assert_eq!(listed["data"].as_array().context("listed")?.len(), 1);
    let shown = kanban(
        &temp.path,
        &["--json", "label", "proposals", "show", &proposal_id],
    )?
    .success_json()?;
    assert_eq!(shown["data"]["name"], "database");

    let accepted = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "proposals",
            "accept",
            &proposal_id,
            "--reason",
            "覆盖不足，接受",
        ],
    )?
    .success_json()?;
    assert_eq!(accepted["data"]["status"], "accepted");
    assert!(accepted["data"]["resolved_label_id"].as_str().is_some());
    let task_after = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(
        task_after["data"]["labels"]
            .as_array()
            .context("task labels")?
            .is_empty(),
        "accept must not auto-bind task labels"
    );
    let label_id = accepted["data"]["resolved_label_id"]
        .as_str()
        .context("resolved label id")?;
    let semantics = kanban_sqlite::api::get_label_semantics(&temp.path, "default", label_id)?;
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    let explained = kanban(
        &temp.path,
        &["--json", "label", "atom", "explain", &atom.id],
    )?
    .success_json()?;
    assert_eq!(explained["data"]["legacy_untracked"], false);
    assert!(
        explained["data"]["provenance_actions"]
            .as_array()
            .context("provenance actions")?
            .iter()
            .any(
                |provenance| provenance["action"]["action_type"] == "bootstrap_label"
                    && provenance["action"]["result_proposal_id"] == proposal_id
            ),
        "{explained}"
    );

    let reject_id = seed_proposed_label_proposal(
        &temp.path,
        task_id,
        LabelProposalCandidate {
            name: "release".to_owned(),
            description: Some("Release workflow".to_owned()),
            applies_when: vec!["packaging".to_owned()],
            ..LabelProposalCandidate::default()
        },
    )?;
    let rejected = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "proposals",
            "reject",
            &reject_id,
            "--reason",
            "不采用",
        ],
    )?
    .success_json()?;
    assert_eq!(rejected["data"]["status"], "rejected");
    Ok(())
}

#[test]
fn metadata_ontology_record_input_fixture_is_consumed_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_record_list_show_review_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology cli task",
            "--description",
            "ready spec for ontology CLI capture",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let input_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/metadata/ontology-record-input.v1.valid.json"
    );

    let observation = kanban(
        &temp.path,
        &[
            "--json", "label", "ontology", "record", task_id, "--input", input_path,
        ],
    )?
    .success_json()?;
    assert!(
        observation["data"]["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("lor_")
    );
    assert_eq!(observation["data"]["signals"][0]["kind"], "false_negative");
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("expected signal id")?;

    let listed = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "list",
            "--status",
            "open",
            "--kind",
            "false_negative",
            "--task",
            task_id,
            "--label",
            "cli",
        ],
    )?
    .success_json()?;
    assert_eq!(listed["data"].as_array().context("signals")?.len(), 1);
    assert_eq!(listed["data"][0]["id"], signal_id);
    assert_eq!(listed["data"][0]["target_label_name_snapshot"], "cli");

    let shown = kanban(
        &temp.path,
        &["--json", "label", "ontology", "show", signal_id],
    )?
    .success_json()?;
    assert_eq!(shown["data"]["signal"]["id"], signal_id);
    assert_eq!(shown["data"]["observation"]["task_id"], task_id);

    let review = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "review",
            "--group-by",
            "label",
        ],
    )?
    .success_json()?;
    let groups = review["data"].as_array().context("review groups")?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["group_by"], "label");
    assert_eq!(groups[0]["label_name"], "cli");
    assert_eq!(groups[0]["task_count"], 1);
    assert_eq!(groups[0]["signal_count"], 1);
    assert_eq!(groups[0]["open_count"], 1);
    assert_eq!(groups[0]["signal_ids"][0], signal_id);

    let cluster_review = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "review",
            "--group-by",
            "cluster",
        ],
    )?
    .success_json()?;
    let cluster_groups = cluster_review["data"]
        .as_array()
        .context("cluster groups")?;
    assert_eq!(cluster_groups.len(), 1);
    assert_eq!(cluster_groups[0]["group_by"], "cluster");
    assert_eq!(
        cluster_groups[0]["cluster_key"],
        "candidate:kind:false_negative|action:add_positive_atom|target:cli|proposed:none|text:extends cli subcommands arguments help output or json behavior"
    );
    assert_eq!(
        cluster_groups[0]["cluster_reason"],
        "normalized_candidate_text"
    );
    assert_eq!(cluster_groups[0]["task_count"], 1);
    assert_eq!(cluster_groups[0]["signal_ids"][0], signal_id);

    let review = kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "review",
            "--group-by",
            "candidate-atom",
        ],
    )?
    .success_stdout()?;
    assert!(review.contains("candidate_atom"), "{review}");
    assert!(review.contains(signal_id), "{review}");
    assert!(
        review.contains("extends CLI subcommands, arguments, help output, or JSON behavior"),
        "{review}"
    );

    Ok(())
}

#[test]
fn label_ontology_cli_review_candidate_atom_empty_candidate_fallback() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_review_candidate_atom_empty_candidate_fallback")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task_a = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology ledger gap A"),
    )?;
    let task_b = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology ledger gap B"),
    )?;
    let task_c = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Decision comments gap"),
    )?;
    for (task_id, signal_key, proposed_label_name, score) in [
        (&task_a.id, "cli-gap-a", "Ontology Ledger", 0.1),
        (&task_b.id, "cli-gap-b", "Ontology Ledger", 0.12),
        (&task_c.id, "cli-gap-c", "Decision Comments", 0.14),
    ] {
        kanban_sqlite::api::record_label_ontology_observation(
            &temp.path,
            "default",
            task_id,
            LabelOntologyRecordInput {
                actor: LabelOntologyActor {
                    name: "label-agent".to_owned(),
                    actor_type: "agent".to_owned(),
                    agent_type: Some("local".to_owned()),
                },
                agent_candidates_json: json!([]).to_string(),
                suggestion_snapshot_json: json!({"selected_labels": []}).to_string(),
                final_decision_json: json!({"accepted_labels": []}).to_string(),
                suggest_coverage: Some(0.2),
                suggest_coverage_cosine: Some(0.3),
                suggest_residual_norm: Some(0.8),
                suggest_needs_new_label: true,
                suggest_degraded: false,
                diagnostics_json: json!([]).to_string(),
                capture_fingerprint: None,
                signals: vec![LabelOntologySignalInput {
                    kind: LabelOntologySignalKind::VocabularyGap,
                    target_label_ref: None,
                    related_labels_json: json!([]).to_string(),
                    proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                    candidate_atom: None,
                    proposed_label_name: Some(proposed_label_name.to_owned()),
                    proposal_json: json!({
                        "name": proposed_label_name,
                        "description": "review grouping candidate"
                    })
                    .to_string(),
                    agent_selected: false,
                    suggest_state: Some(LabelOntologySuggestState::Absent),
                    suggest_score: Some(score),
                    suggest_rank: None,
                    final_selected: false,
                    rationale: "Existing label vocabulary did not explain this task.".to_owned(),
                    confidence: Some(0.7),
                    signal_key: Some(signal_key.to_owned()),
                }],
            },
        )?;
    }

    let review = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "review",
            "--group-by",
            "candidate-atom",
        ],
    )?
    .success_json()?;
    let groups = review["data"].as_array().context("review groups")?;
    let ontology_gap = groups
        .iter()
        .find(|group| {
            group["key"]
                .as_str()
                .unwrap_or_default()
                .contains("proposed:ontology ledger")
        })
        .context("ontology ledger group")?;
    assert_eq!(ontology_gap["group_by"], "candidate_atom");
    assert_eq!(ontology_gap["proposed_label_name"], "Ontology Ledger");
    assert_eq!(ontology_gap["task_count"], 2);
    assert_eq!(ontology_gap["signal_count"], 2);
    assert!(
        ontology_gap["key"]
            .as_str()
            .unwrap_or_default()
            .contains("vocabulary_gap")
    );
    assert!(
        ontology_gap["key"]
            .as_str()
            .unwrap_or_default()
            .contains("bootstrap_label")
    );

    let other_gap = groups
        .iter()
        .find(|group| {
            group["key"]
                .as_str()
                .unwrap_or_default()
                .contains("proposed:decision comments")
        })
        .context("decision comments group")?;
    assert_eq!(other_gap["task_count"], 1);
    assert_ne!(ontology_gap["key"], other_gap["key"]);

    let human = kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "review",
            "--group-by",
            "candidate-atom",
        ],
    )?
    .success_stdout()?;
    assert!(
        human.contains(
            "key=no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label"
        ),
        "{human}"
    );
    assert!(human.contains("title=Ontology Ledger"), "{human}");
    assert!(human.contains("tasks=2"), "{human}");
    Ok(())
}

#[test]
fn label_ontology_cli_record_accepts_simplified_snapshot_capture_input() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_record_accepts_simplified_snapshot_capture_input")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology simplified capture task",
            "--description",
            "ready spec for simplified ontology CLI capture",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let labels_before = list_labels(&temp.path, "default")?;
    let task_labels_before = get_task(&temp.path, "default", task_id)?.labels;
    let atoms_before = list_label_atoms(&temp.path, "default")?;

    let snapshot_path = temp.dir.join("ontology-suggestion-envelope.json");
    fs::write(
        &snapshot_path,
        json!({
            "data": {
                "selected_labels": [],
                "candidates": [],
                "coverage": 0.42,
                "coverage_cosine": 0.37,
                "residual_norm": 0.58,
                "needs_new_label": true,
                "degraded": true,
                "diagnostics": ["vector_store_disabled"]
            }
        })
        .to_string(),
    )?;
    let input_json = json!({
        "actor": {
            "name": "label-agent",
            "type": "agent",
            "agent_type": "local"
        },
        "agent_candidates": [
            {"label": "cli", "confidence": 0.92}
        ],
        "final_decision": {
            "accepted_labels": ["cli"]
        },
        "capture_fingerprint": "cli-simplified-capture",
        "signals": [{
            "kind": "false_negative",
            "target_label_ref": "cli",
            "related_labels": [],
            "proposed_action": "add_positive_atom",
            "candidate_atom": {
                "polarity": "positive",
                "kind": "applies_when",
                "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
            },
            "agent_selected": true,
            "suggest_state": "absent",
            "final_selected": true,
            "rationale": "The task expands the CLI surface although suggest selected nothing.",
            "confidence": 0.91,
            "signal_key": "cli-simplified-false-negative"
        }]
    })
    .to_string();
    let snapshot_path = snapshot_path
        .to_str()
        .context("temp snapshot path should be valid UTF-8")?;

    let observation = kanban_with_stdin(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            "-",
            "--suggestion-snapshot",
            snapshot_path,
        ],
        &input_json,
    )?
    .success_json()?;

    assert_eq!(observation["data"]["suggest_coverage"], 0.42);
    assert_eq!(observation["data"]["suggest_coverage_cosine"], 0.37);
    assert_eq!(observation["data"]["suggest_residual_norm"], 0.58);
    assert_eq!(observation["data"]["suggest_needs_new_label"], true);
    assert_eq!(observation["data"]["suggest_degraded"], true);
    let diagnostics = observation["data"]["diagnostics"].clone();
    assert_eq!(diagnostics, json!(["vector_store_disabled"]));
    assert_eq!(
        observation["data"]["signals"][0]["signal_key"],
        "cli-simplified-false-negative"
    );

    assert_eq!(list_labels(&temp.path, "default")?, labels_before);
    assert_eq!(
        get_task(&temp.path, "default", task_id)?.labels,
        task_labels_before
    );
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);

    Ok(())
}

fn assert_ontology_record_snapshot_projection_conflict(
    explicit_field: &str,
    snapshot_value: serde_json::Value,
    explicit_value: serde_json::Value,
) -> anyhow::Result<()> {
    let temp = TempDb::new(&format!("ontology_record_{explicit_field}_conflict"))?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology projection conflict",
            "--description",
            "reject conflicting explicit ontology projection fields",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let mut snapshot = json!({
        "needs_new_label": false,
        "degraded": false,
        "diagnostics": []
    });
    let snapshot_field = match explicit_field {
        "suggest_needs_new_label" => "needs_new_label",
        "suggest_degraded" => "degraded",
        "diagnostics" => "diagnostics",
        other => anyhow::bail!("unsupported projection field {other}"),
    };
    snapshot[snapshot_field] = snapshot_value;
    let mut input = json!({
        "actor": {"name": "fixture", "type": "agent", "agent_type": "codex"},
        "suggestion_snapshot": snapshot,
        "signals": [{
            "kind": "false_negative",
            "target_label_ref": "cli",
            "related_labels": [],
            "proposed_action": "add_positive_atom",
            "candidate_atom": {
                "polarity": "positive",
                "kind": "applies_when",
                "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
            },
            "agent_selected": true,
            "suggest_state": "absent",
            "final_selected": true,
            "rationale": "fixture conflict must be rejected before service persistence"
        }]
    });
    input[explicit_field] = explicit_value;
    let input_path = temp.dir.join(format!("{explicit_field}-conflict.json"));
    fs::write(&input_path, serde_json::to_vec_pretty(&input)?)?;

    let result = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            input_path.to_str().context("input path")?,
        ],
    )?;
    assert!(
        !result.output.status.success(),
        "conflicting {explicit_field} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.output.stdout),
        String::from_utf8_lossy(&result.output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.output.stderr), "");
    let error: serde_json::Value = serde_json::from_slice(&result.output.stdout)?;
    assert_eq!(error["error"]["code"], "invalid_input");
    assert_eq!(error["error"]["exit_code"], 2);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains(&format!(
                "input {explicit_field} conflicts with suggestion_snapshot.{snapshot_field}"
            )),
        "{error}"
    );
    let observation_count: i64 = kanban_test_support::connect_file(&temp.path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_observations",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        observation_count, 0,
        "conflict must not persist an observation"
    );
    Ok(())
}

#[test]
fn label_ontology_cli_record_rejects_explicit_needs_new_label_snapshot_conflict()
-> anyhow::Result<()> {
    assert_ontology_record_snapshot_projection_conflict(
        "suggest_needs_new_label",
        json!(false),
        json!(true),
    )
}

#[test]
fn label_ontology_cli_record_rejects_explicit_degraded_snapshot_conflict() -> anyhow::Result<()> {
    assert_ontology_record_snapshot_projection_conflict(
        "suggest_degraded",
        json!(false),
        json!(true),
    )
}

#[test]
fn label_ontology_cli_record_rejects_empty_diagnostics_snapshot_conflict() -> anyhow::Result<()> {
    assert_ontology_record_snapshot_projection_conflict(
        "diagnostics",
        json!(["snapshot diagnostic"]),
        json!([]),
    )
}

fn assert_ontology_record_null_snapshot_projection_uses_explicit(
    explicit_field: &str,
    snapshot_field: &str,
    explicit_value: serde_json::Value,
) -> anyhow::Result<()> {
    let temp = TempDb::new(&format!("ontology_record_{explicit_field}_null_snapshot"))?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology null projection",
            "--description",
            "treat null snapshot projections as absent",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let mut snapshot = json!({});
    snapshot[snapshot_field] = serde_json::Value::Null;
    let mut input = json!({
        "actor": {"name": "fixture", "type": "agent", "agent_type": "codex"},
        "suggestion_snapshot": snapshot,
        "signals": [{
            "kind": "false_negative",
            "target_label_ref": "cli",
            "related_labels": [],
            "proposed_action": "add_positive_atom",
            "candidate_atom": {
                "polarity": "positive",
                "kind": "applies_when",
                "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
            },
            "agent_selected": true,
            "suggest_state": "absent",
            "final_selected": true,
            "rationale": "null snapshot projection defers to the explicit field"
        }]
    });
    input[explicit_field] = explicit_value.clone();
    let input_path = temp
        .dir
        .join(format!("{explicit_field}-null-snapshot.json"));
    fs::write(&input_path, serde_json::to_vec_pretty(&input)?)?;

    let observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            input_path.to_str().context("input path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(observation["data"][explicit_field], explicit_value);
    assert_eq!(
        observation["data"]["suggestion_snapshot"].get(snapshot_field),
        Some(&serde_json::Value::Null),
        "the natural snapshot must retain the explicit null field"
    );
    let observation_count: i64 = kanban_test_support::connect_file(&temp.path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_observations",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(observation_count, 1);
    Ok(())
}

#[test]
fn label_ontology_cli_record_accepts_null_needs_new_label_with_explicit_projection()
-> anyhow::Result<()> {
    assert_ontology_record_null_snapshot_projection_uses_explicit(
        "suggest_needs_new_label",
        "needs_new_label",
        json!(true),
    )
}

#[test]
fn label_ontology_cli_record_accepts_null_diagnostics_with_explicit_projection()
-> anyhow::Result<()> {
    assert_ontology_record_null_snapshot_projection_uses_explicit(
        "diagnostics",
        "diagnostics",
        json!([]),
    )
}

#[test]
fn label_ontology_cli_lifecycle_apply_and_validate_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_lifecycle_apply_and_validate_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology cli lifecycle task",
            "--description",
            "ready spec for ontology CLI lifecycle actions",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let input_path = write_cli_ontology_record_input(
        &temp,
        "ontology-lifecycle-record.json",
        &[
            ("cli-lifecycle-primary", "adds CLI lifecycle commands"),
            (
                "cli-lifecycle-duplicate",
                "duplicates CLI lifecycle commands",
            ),
            ("cli-lifecycle-reject", "low confidence duplicate"),
            (
                "cli-lifecycle-no-change",
                "already covered by existing docs",
            ),
        ],
    )?;

    let observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            &input_path,
        ],
    )?
    .success_json()?;
    let signals = observation["data"]["signals"]
        .as_array()
        .context("signals")?;
    let primary = signals[0]["id"].as_str().context("primary signal")?;
    let duplicate = signals[1]["id"].as_str().context("duplicate signal")?;
    let rejected_signal = signals[2]["id"].as_str().context("rejected signal")?;
    let no_change_signal = signals[3]["id"].as_str().context("no-change signal")?;

    let confirm_reason =
        "Reviewer confirmed the false negative.\nShell-sensitive `$VAR` remains literal.";
    let confirm_reason_path = temp.dir.join("ontology-confirm-reason.md");
    fs::write(&confirm_reason_path, confirm_reason)?;
    let confirmed = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "confirm",
            primary,
            "--reason-file",
            confirm_reason_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(confirmed["data"]["action_type"], "confirm");
    assert_eq!(confirmed["data"]["signal_ids"][0], primary);
    assert_eq!(confirmed["data"]["reason"], confirm_reason);

    let rejected = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "reject",
            rejected_signal,
            "--reason",
            "Reviewer rejected the weak signal.",
        ],
    )?
    .success_json()?;
    assert_eq!(rejected["data"]["action_type"], "reject");

    let superseded = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "supersede",
            duplicate,
            "--by",
            primary,
            "--reason",
            "Duplicate of the confirmed signal.",
        ],
    )?
    .success_json()?;
    assert_eq!(superseded["data"]["action_type"], "supersede");

    let resolved_no_change = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "resolve",
            no_change_signal,
            "--no-change",
            "--reason",
            "Existing ontology already covers this signal.",
        ],
    )?
    .success_json()?;
    assert_eq!(
        resolved_no_change["data"]["action_type"],
        "resolve_no_change"
    );

    let atom_text = "extends CLI subcommands, arguments, help output, or JSON behavior\nwithout shell expansion of $(date)";
    let atom_text_path = temp.dir.join("ontology-atom-text.md");
    fs::write(&atom_text_path, atom_text)?;
    let apply_reason =
        "Confirmed false-negative support for CLI surface changes.\nPreserve `code` literally.";
    let apply_reason_path = temp.dir.join("ontology-apply-reason.md");
    fs::write(&apply_reason_path, apply_reason)?;
    let applied = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "apply-agent",
            "label",
            "ontology",
            "apply",
            "atom",
            primary,
            "--label",
            "cli",
            "--kind",
            "applies-when",
            "--text-file",
            atom_text_path.to_str().context("utf-8 path")?,
            "--reason-file",
            apply_reason_path.to_str().context("utf-8 path")?,
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
        ],
    )?
    .success_json()?;
    assert_eq!(applied["data"]["action_type"], "add_positive_atom");
    assert_eq!(applied["data"]["reason"], apply_reason);
    assert_eq!(applied["data"]["validation_status"], "pending");
    assert_eq!(applied["data"]["created_by"], "apply-agent");
    assert_eq!(applied["data"]["created_by_type"], "agent");
    assert_eq!(applied["data"]["agent_type"], "codex");
    let apply_action_id = applied["data"]["id"].as_str().context("apply action id")?;
    let target_label_id = applied["data"]["target_label_id"]
        .as_str()
        .context("target label id")?;
    let result_atom_id = applied["data"]["result_atom_id"]
        .as_str()
        .context("result atom id")?;
    let result_atom_content_hash = applied["data"]["result_atom_content_hash"]
        .as_str()
        .context("result atom content hash")?;

    let validation_path = temp.dir.join("ontology-validation.json");
    fs::write(
        &validation_path,
        json!({
            "evidence_type": "trusted_automated",
            "embedding_model": "test-embedding-v1",
            "solver_options": {"candidate_limit": 24, "atom_limit": 64},
            "index": {"status": "ready", "dirty": false, "generation": 7},
            "cases": [{
                "signal_id": primary,
                "case_type": "positive_atom",
                "passed": true,
                "target_label_id": target_label_id,
                "before": {
                    "target": {
                        "label_id": target_label_id,
                        "selected": false,
                        "score": 0.08
                    },
                    "coverage": 0.61
                },
                "after": {
                    "degraded": false,
                    "target": {
                        "label_id": target_label_id,
                        "selected": true,
                        "score": 0.74
                    },
                    "coverage": 0.79,
                    "evidence_atoms": [{
                        "id": result_atom_id,
                        "content_hash": result_atom_content_hash,
                        "label_id": target_label_id
                    }]
                }
            }]
        })
        .to_string(),
    )?;
    let validation_path = validation_path
        .to_str()
        .context("temp path should be valid UTF-8")?;
    let validate_reason =
        "The source task now selects cli with the new atom as evidence.\nValidation from file.";
    let validate_reason_path = temp.dir.join("ontology-validate-reason.md");
    fs::write(&validate_reason_path, validate_reason)?;
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "validate-agent",
            "label",
            "ontology",
            "validate",
            apply_action_id,
            "--status",
            "passed",
            "--reason-file",
            validate_reason_path.to_str().context("utf-8 path")?,
            "--input",
            validation_path,
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
        ],
    )?
    .json_failure_containing("trusted evidence collected by the kanban tool")?;

    let primary_detail = kanban(
        &temp.path,
        &["--json", "label", "ontology", "show", primary],
    )?
    .success_json()?;
    assert_eq!(primary_detail["data"]["signal"]["status"], "confirmed");
    assert_eq!(
        primary_detail["data"]["actions"]
            .as_array()
            .context("primary actions")?
            .len(),
        2
    );
    let duplicate_detail = kanban(
        &temp.path,
        &["--json", "label", "ontology", "show", duplicate],
    )?
    .success_json()?;
    assert_eq!(duplicate_detail["data"]["signal"]["status"], "superseded");
    assert_eq!(
        duplicate_detail["data"]["signal"]["superseded_by_signal_id"],
        primary
    );

    let gap_input_path = temp.dir.join("ontology-proposal-gap.json");
    fs::write(
        &gap_input_path,
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates": [],
            "suggestion_snapshot": {},
            "final_decision": {},
            "suggest_coverage": 0.2,
            "suggest_coverage_cosine": 0.3,
            "suggest_residual_norm": 0.8,
            "suggest_needs_new_label": true,
            "suggest_degraded": false,
            "diagnostics": [],
            "capture_fingerprint": "cli-proposal-gap",
            "signals": [{
                "kind": "vocabulary_gap",
                "target_label_ref": null,
                "related_labels": [],
                "proposed_action": "bootstrap_label",
                "candidate_atom": null,
                "proposed_label_name": "ontology-ledger",
                "proposal": {"name":"ontology-ledger"},
                "agent_selected": true,
                "suggest_state": "absent",
                "suggest_score": null,
                "suggest_rank": null,
                "final_selected": true,
                "rationale": "Existing labels do not express ontology ledger storage.",
                "confidence": 0.86,
                "signal_key": "cli-proposal-gap"
            }]
        })
        .to_string(),
    )?;
    let gap_input_path = gap_input_path
        .to_str()
        .context("temp path should be valid UTF-8")?;
    let gap_observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            gap_input_path,
        ],
    )?
    .success_json()?;
    let gap_signal = gap_observation["data"]["signals"][0]["id"]
        .as_str()
        .context("gap signal")?;
    let confirmed = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "codex",
            "label",
            "ontology",
            "confirm",
            gap_signal,
            "--reason",
            "Reviewer confirmed the vocabulary gap.",
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
        ],
    )?
    .success_json()?;
    assert_eq!(confirmed["data"]["created_by"], "codex");
    assert_eq!(confirmed["data"]["created_by_type"], "agent");
    assert_eq!(confirmed["data"]["agent_type"], "codex");
    let proposal_id = seed_proposed_label_proposal(
        &temp.path,
        task_id,
        LabelProposalCandidate {
            name: "ontology-ledger".to_owned(),
            description: Some("Label ontology ledger work".to_owned()),
            applies_when: vec!["records ontology observations and signals".to_owned()],
            positive_examples: vec!["creates label ontology ledger tables".to_owned()],
            ..LabelProposalCandidate::default()
        },
    )?;
    let accepted = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "ontology-agent",
            "label",
            "proposals",
            "accept",
            &proposal_id,
            "--reason",
            "Bootstrap from confirmed vocabulary-gap signal.",
            "--source-signal",
            gap_signal,
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
            "--allow-retarget",
            "--retarget-reason",
            "Reviewer explicitly audited proposal source signal retarget.",
        ],
    )?
    .success_json()?;
    assert_eq!(accepted["data"]["status"], "accepted");
    assert!(accepted["data"]["resolved_label_id"].as_str().is_some());
    let gap_detail = kanban(
        &temp.path,
        &["--json", "label", "ontology", "show", gap_signal],
    )?
    .success_json()?;
    assert_eq!(gap_detail["data"]["signal"]["status"], "confirmed");
    let bootstrap = gap_detail["data"]["actions"]
        .as_array()
        .context("gap actions")?
        .iter()
        .find(|action| action["action_type"] == "bootstrap_label")
        .context("bootstrap action")?;
    assert_eq!(bootstrap["created_by"], "ontology-agent");
    assert_eq!(bootstrap["created_by_type"], "agent");
    assert_eq!(bootstrap["agent_type"], "codex");
    let change = &bootstrap["change"];
    assert_eq!(
        change["retarget_override"]["reason"],
        "Reviewer explicitly audited proposal source signal retarget."
    );

    Ok(())
}

#[test]
fn label_ontology_cli_revert_action_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_revert_action_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    kanban_sqlite::api::upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let before_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    let task = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("ontology CLI revert task"),
    )?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            actor: LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([{"label": "cli", "confidence": 0.92}]).to_string(),
            suggestion_snapshot_json: json!({"selected_labels": []}).to_string(),
            final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
            suggest_coverage: Some(0.61),
            suggest_coverage_cosine: Some(0.74),
            suggest_residual_norm: Some(0.39),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: json!([]).to_string(),
            capture_fingerprint: Some("cli-revert-action-round-trip".to_owned()),
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: json!([]).to_string(),
                proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "extends CLI subcommands, arguments, help output, or JSON behavior"
                        .to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: json!({}).to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.08),
                suggest_rank: Some(4),
                final_selected: true,
                rationale: "The task expands the CLI surface although suggest scored cli weakly."
                    .to_owned(),
                confidence: Some(0.91),
                signal_key: Some("cli-revert-action".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyActionInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "Confirmed by reviewer.".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;
    let applied = kanban_sqlite::api::apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;
    let action_count_before_failed_revert = ontology_action_count(&temp.path)?;

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "revert",
            &applied.id,
            "--expected-current-hash",
            "not-the-current-semantics-hash",
            "--reason",
            "This revert should fail before writing a new action.",
        ],
    )?
    .failure_containing("expected_current_hash does not match")?;
    assert_eq!(
        ontology_action_count(&temp.path)?,
        action_count_before_failed_revert
    );
    let after_failed_revert = get_label_semantics(&temp.path, "default", "cli")?;
    assert_ne!(
        after_failed_revert.semantics_hash,
        before_semantics.semantics_hash
    );
    assert!(
        after_failed_revert
            .atoms
            .iter()
            .any(|atom| applied.result_atom_id.as_deref() == Some(atom.id.as_str()))
    );

    let reverted = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "revert",
            &applied.id,
            "--expected-current-hash",
            applied
                .canonical_after_hash
                .as_deref()
                .context("canonical after hash")?,
            "--reason",
            "Revert the test ontology mutation.",
        ],
    )?
    .success_json()?;

    assert_eq!(reverted["data"]["action_type"], "revert_ontology_mutation");
    assert_eq!(reverted["data"]["parent_action_id"], applied.id);
    assert_eq!(reverted["data"]["signal_ids"], json!([signal_id]));
    let restored_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    assert_eq!(
        restored_semantics.semantics_hash,
        before_semantics.semantics_hash
    );
    assert_eq!(restored_semantics.description, before_semantics.description);
    assert_eq!(
        restored_semantics.applies_when,
        before_semantics.applies_when
    );
    assert_eq!(
        restored_semantics.excludes_when,
        before_semantics.excludes_when
    );
    assert_eq!(
        restored_semantics.positive_examples,
        before_semantics.positive_examples
    );
    assert_eq!(
        restored_semantics.negative_examples,
        before_semantics.negative_examples
    );
    assert_eq!(
        restored_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>(),
        before_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>()
    );

    Ok(())
}

#[test]
fn label_ontology_cli_structure_plan_command_is_not_available() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_structure_plan_command_is_not_available")?;
    kanban(&temp.path, &["init"])?.success()?;
    let help = kanban(&temp.path, &["label", "ontology", "--help"])?.success_stdout()?;
    assert!(!help.contains("structure"), "{help}");

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "structure",
            "plan",
            "rename-label",
            "los_missing",
            "--target-label",
            "cli",
            "--proposed-label",
            "command surface",
            "--reason",
            "Structure plans are no longer public write entries.",
        ],
    )?
    .failure_containing("unrecognized subcommand 'structure'")?;
    let action_count: i64 = kanban_test_support::connect_file(&temp.path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(action_count, 0);

    Ok(())
}

#[test]
fn label_ontology_cli_validate_positive_controls_require_trusted() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_validate_positive_controls_require_trusted")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "validate",
            "loa_missing",
            "--status",
            "failed",
            "--reason",
            "Positive controls are trusted collector inputs.",
            "--positive-control",
            "default#1",
        ],
    )?
    .failure_containing("--positive-control and --positive-control-waiver require --trusted")?;

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "validate",
            "loa_missing",
            "--status",
            "failed",
            "--reason",
            "Positive-control waiver is a trusted collector input.",
            "--positive-control-waiver",
            "No stable positive control exists.",
        ],
    )?
    .failure_containing("--positive-control and --positive-control-waiver require --trusted")?;

    Ok(())
}

#[test]
fn label_ontology_cli_validate_positive_controls_are_mutually_exclusive() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_validate_positive_controls_are_mutually_exclusive")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "validate",
            "loa_missing",
            "--trusted",
            "--status",
            "failed",
            "--reason",
            "Only controls or a waiver may be supplied.",
            "--positive-control",
            "default#1",
            "--positive-control-waiver",
            "No stable positive control exists.",
        ],
    )?
    .failure_containing("cannot be used with")?;

    Ok(())
}

#[test]
fn label_ontology_cli_apply_existing_atom_uses_adopt_existing_action() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_apply_existing_atom_uses_adopt_existing_action")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    kanban_sqlite::api::upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            applies_when: vec![
                "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let existing_atom = atoms_before
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("existing applies_when atom")?;
    clear_label_atom_dirty_flags(&temp.path, "default")?;
    let add_action_count_before = add_atom_action_count(&temp.path)?;

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "existing atom adoption CLI task",
            "--description",
            "ready spec for existing atom adoption",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let input_path = write_cli_ontology_record_input(
        &temp,
        "ontology-existing-atom-record.json",
        &[(
            "cli-existing-atom",
            "links a confirmed signal to an existing CLI atom",
        )],
    )?;
    let observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            &input_path,
        ],
    )?
    .success_json()?;
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?;
    kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "confirm",
            signal_id,
            "--reason",
            "Reviewer confirmed existing atom signal.",
        ],
    )?
    .success_json()?;

    let applied = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "apply",
            "atom",
            signal_id,
            "--label",
            "cli",
            "--kind",
            "applies-when",
            "--text",
            "extends CLI subcommands, arguments, help output, or JSON behavior",
            "--reason",
            "Link confirmed signal to existing CLI atom.",
        ],
    )?
    .success_json()?;

    assert_eq!(applied["data"]["action_type"], "adopt_existing_atom");
    assert_eq!(applied["data"]["validation_status"], "not_required");
    assert_eq!(applied["data"]["result_atom_id"], existing_atom.id);
    assert_eq!(
        applied["data"]["result_atom_content_hash"],
        existing_atom.content_hash
    );
    assert_eq!(
        applied["data"]["canonical_before_hash"],
        applied["data"]["canonical_after_hash"]
    );
    let change = &applied["data"]["change"];
    assert_eq!(change["canonical_changed"], false);
    assert_eq!(change["provenance_only"], true);
    assert_eq!(change["requested_action_type"], "add_positive_atom");
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(add_atom_action_count(&temp.path)?, add_action_count_before);
    let status = kanban_sqlite::api::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.dirty, Some(false));
    assert_eq!(status.board_dirty, Some(false));

    Ok(())
}

#[test]
fn label_ontology_cli_apply_atom_retarget_override_records_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_apply_atom_retarget_override_records_reason")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology retarget override task",
            "--description",
            "ready spec for ontology retarget override",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let input_path = temp.dir.join("ontology-retarget-record.json");
    fs::write(
        &input_path,
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates": [],
            "suggestion_snapshot": {},
            "final_decision": {},
            "suggest_coverage": 0.2,
            "suggest_coverage_cosine": 0.3,
            "suggest_residual_norm": 0.8,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics": [],
            "capture_fingerprint": "cli-retarget-override",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "backend",
                "related_labels": [],
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends backend persistence or service APIs"
                },
                "proposed_label_name": null,
                "proposal": {},
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The task was initially classified as backend work.",
                "confidence": 0.91,
                "signal_key": "cli-retarget-override"
            }]
        })
        .to_string(),
    )?;
    let input_path = input_path
        .to_str()
        .context("temp path should be valid UTF-8")?;
    let observation = kanban(
        &temp.path,
        &[
            "--json", "label", "ontology", "record", task_id, "--input", input_path,
        ],
    )?
    .success_json()?;
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?;
    kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "confirm",
            signal_id,
            "--reason",
            "Reviewer confirmed source signal.",
        ],
    )?
    .success_json()?;

    let applied = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "apply",
            "atom",
            signal_id,
            "--label",
            "cli",
            "--kind",
            "applies-when",
            "--text",
            "extends CLI subcommands, arguments, help output, or JSON behavior",
            "--reason",
            "Reviewer retargeted source signal to CLI.",
            "--allow-retarget",
            "--retarget-reason",
            "Signal captures a CLI boundary despite backend wording.",
        ],
    )?
    .success_json()?;
    let change = &applied["data"]["change"];
    assert_eq!(
        change["retarget_override"]["reason"],
        "Signal captures a CLI boundary despite backend wording."
    );
    assert_eq!(change["retarget_override"]["target_label"]["name"], "cli");

    Ok(())
}

#[test]
fn label_ontology_cli_apply_atom_rejects_incompatible_source_signal() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_apply_atom_rejects_incompatible_source_signal")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let task = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("ontology CLI incompatible source signal"),
    )?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            actor: LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([{"label": "cli", "confidence": 0.92}]).to_string(),
            suggestion_snapshot_json: json!({"selected_labels": []}).to_string(),
            final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
            suggest_coverage: Some(0.61),
            suggest_coverage_cosine: Some(0.74),
            suggest_residual_norm: Some(0.39),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: json!([]).to_string(),
            capture_fingerprint: Some("cli-incompatible-source-signal".to_owned()),
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: json!([]).to_string(),
                proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "extends CLI subcommands, arguments, help output, or JSON behavior"
                        .to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: json!({}).to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.08),
                suggest_rank: Some(4),
                final_selected: true,
                rationale: "The task expands the CLI surface.".to_owned(),
                confidence: Some(0.91),
                signal_key: Some("cli-incompatible-source-signal".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyActionInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "Confirmed by reviewer.".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;
    let action_count = ontology_action_count(&temp.path)?;

    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "apply",
            "atom",
            &signal_id,
            "--label",
            "cli",
            "--kind",
            "excludes-when",
            "--text",
            "only updates unrelated release notes",
            "--reason",
            "This should fail because the source signal requested a positive atom.",
        ],
    )?
    .failure_containing(
        "proposed action add_positive_atom does not match apply atom action add_negative_atom",
    )?;
    assert_eq!(ontology_action_count(&temp.path)?, action_count);
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    Ok(())
}

#[test]
fn label_atom_explain_cli_json_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_explain_cli_json_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    let label = kanban_sqlite::api::create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("explain atom CLI provenance"),
    )?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            actor: LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([{"label": "cli", "confidence": 0.92}]).to_string(),
            suggestion_snapshot_json: json!({"selected_labels": []}).to_string(),
            final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
            suggest_coverage: Some(0.61),
            suggest_coverage_cosine: Some(0.74),
            suggest_residual_norm: Some(0.39),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: json!([]).to_string(),
            capture_fingerprint: Some("cli-atom-explain-round-trip".to_owned()),
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: json!([]).to_string(),
                proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "extends CLI subcommands, arguments, help output, or JSON behavior"
                        .to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: json!({}).to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.08),
                suggest_rank: Some(4),
                final_selected: true,
                rationale: "The task expands the CLI surface although suggest scored cli weakly."
                    .to_owned(),
                confidence: Some(0.91),
                signal_key: Some("cli-atom-explain".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyActionInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "Confirmed by reviewer.".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;
    let applied = kanban_sqlite::api::apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: label.id.clone(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;
    let atom_id = applied
        .result_atom_id
        .as_deref()
        .context("result atom id")?;

    let explained =
        kanban(&temp.path, &["--json", "label", "atom", "explain", atom_id])?.success_json()?;

    assert_eq!(explained["data"]["atom"]["id"], atom_id);
    assert_eq!(explained["data"]["atom"]["label_id"], label.id);
    assert_eq!(
        explained["data"]["provenance_actions"][0]["action"]["id"],
        applied.id
    );
    assert_eq!(
        explained["data"]["supporting_signals"][0]["source_task"]["id"],
        task.id
    );
    assert_eq!(explained["data"]["legacy_untracked"], false);

    let human = kanban(&temp.path, &["label", "atom", "explain", atom_id])?.success_stdout()?;
    assert!(human.contains(atom_id), "{human}");
    assert!(human.contains("provenance"), "{human}");
    Ok(())
}

fn clear_label_atom_dirty_flags(path: &Path, board: &str) -> anyhow::Result<()> {
    let board = kanban_sqlite::api::get_board(path, board)?;
    let conn = kanban_test_support::connect_file(path)?;
    conn.execute(
        "UPDATE derived_store_state SET dirty=0, last_error=NULL \
         WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0, last_error=NULL \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [board.id],
    )?;
    Ok(())
}

fn add_atom_action_count(path: &Path) -> anyhow::Result<i64> {
    Ok(kanban_test_support::connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions \
         WHERE action_type IN ('add_positive_atom','add_negative_atom')",
        [],
        |row| row.get(0),
    )?)
}

#[test]
fn label_ontology_cli_record_rejects_double_encoded_json_compatibility_fields() -> anyhow::Result<()>
{
    let temp = TempDb::new("ontology_rejects_double_encoded_json")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "natural ontology input",
            "--description",
            "reject legacy double-encoded metadata fields",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let path = temp.dir.join("legacy-ontology-input.json");
    fs::write(
        &path,
        json!({
            "actor": {"name": "fixture", "type": "agent", "agent_type": "codex"},
            "agent_candidates_json": "[]",
            "suggestion_snapshot": {},
            "signals": []
        })
        .to_string(),
    )?;
    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            path.to_str().context("input path")?,
        ],
    )?
    .failure_containing("unknown field")?;
    Ok(())
}

#[test]
fn metadata_ontology_record_input_rejects_closed_value_domain_fixture() -> anyhow::Result<()> {
    let temp = TempDb::new("ontology_rejects_open_value_domain")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "closed ontology metadata",
            "--description",
            "reject values outside the contract vocabulary",
        ],
    )?
    .success_json()?;
    kanban(
        &temp.path,
        &[
            "label",
            "ontology",
            "record",
            "default#1",
            "--input",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/metadata/ontology-record-input.v1.invalid.json"
            ),
        ],
    )?
    .failure_containing("unknown variant")?;
    Ok(())
}

fn ontology_action_count(path: &Path) -> anyhow::Result<i64> {
    Ok(kanban_test_support::connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions",
        [],
        |row| row.get(0),
    )?)
}

fn write_cli_ontology_record_input(
    temp: &TempDb,
    filename: &str,
    signal_specs: &[(&str, &str)],
) -> anyhow::Result<String> {
    let signals = signal_specs
        .iter()
        .map(|(signal_key, rationale)| {
            json!({
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels": [],
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
                },
                "proposed_label_name": null,
                "proposal": {},
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": rationale,
                "confidence": 0.91,
                "signal_key": signal_key
            })
        })
        .collect::<Vec<_>>();
    let path = temp.dir.join(filename);
    fs::write(
        &path,
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates": [{"label":"cli","confidence":0.92}],
            "suggestion_snapshot": {"selected_labels":[]},
            "final_decision": {"accepted_labels":["cli"]},
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics": [],
            "capture_fingerprint": filename,
            "signals": signals
        })
        .to_string(),
    )?;
    Ok(path
        .to_str()
        .context("temp path should be valid UTF-8")?
        .to_owned())
}

fn seed_proposed_label_proposal(
    db_path: &Path,
    task_id: &str,
    candidate: LabelProposalCandidate,
) -> anyhow::Result<String> {
    let conn = kanban_test_support::connect_file(db_path)?;
    let board_id: String =
        conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
            row.get(0)
        })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let proposal_id = format!("lp_cli_{}_{}", candidate.name, now);
    let applies_when = serde_json::to_string(&candidate.applies_when)?;
    let excludes_when = serde_json::to_string(&candidate.excludes_when)?;
    let positive_examples = serde_json::to_string(&candidate.positive_examples)?;
    let negative_examples = serde_json::to_string(&candidate.negative_examples)?;
    conn.execute(
        "INSERT INTO label_semantic_proposals(
            id, board_id, task_id, status, name, description, applies_when, excludes_when,
            positive_examples, negative_examples, heuristic_coverage, heuristic_residual_norm,
            diagnostics_json, created_by, created_at, updated_at
        ) VALUES (?1, ?2, ?3, 'proposed', ?4, ?5, ?6, ?7, ?8, ?9, 0.0, 1.0, '[]', ?10, ?11, ?11)",
        (
            &proposal_id,
            &board_id,
            task_id,
            &candidate.name,
            candidate.description.as_deref(),
            &applies_when,
            &excludes_when,
            &positive_examples,
            &negative_examples,
            "cli-test-proposer",
            now,
        ),
    )?;
    Ok(proposal_id)
}

#[test]
fn label_suggest_rejects_out_of_bounds_limits() -> anyhow::Result<()> {
    let temp = TempDb::new("label_suggest_rejects_out_of_bounds_limits")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bounded suggestion cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    kanban(
        &temp.path,
        &[
            "--locale", "en", "label", "suggest", task_id, "--limit", "1001",
        ],
    )?
    .failure_containing("limit must be <= 1000")?;
    kanban(
        &temp.path,
        &[
            "--locale",
            "en",
            "label",
            "suggest",
            task_id,
            "--atom-limit",
            "1001",
        ],
    )?
    .failure_containing("limit must be <= 1000")?;
    kanban(
        &temp.path,
        &["label", "suggest", task_id, "--candidate-limit", "0"],
    )?
    .failure_containing("candidate_limit must be >= 1")?;
    kanban(&temp.path, &["label", "suggest", task_id, "--limit", "0"])?
        .failure_containing("limit must be >= 1")?;
    kanban(
        &temp.path,
        &["label", "suggest", task_id, "--atom-limit", "0"],
    )?
    .failure_containing("atom_limit must be >= 1")?;
    kanban(
        &temp.path,
        &["label", "suggest", task_id, "--max-selected-labels", "0"],
    )?
    .failure_containing("max_selected_labels must be >= 1")?;
    Ok(())
}

#[test]
fn label_remove_accepts_l_prefixed_label_name() -> anyhow::Result<()> {
    let temp = TempDb::new("label_remove_accepts_l_prefixed_label_name")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "l_bug"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "l-prefixed label cli task",
            "--description",
            "ready spec",
            "--label",
            "l_bug",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;

    let removed =
        kanban(&temp.path, &["--json", "label", "remove", task_id, "l_bug"])?.success_json()?;
    assert!(
        removed["data"]["labels"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn label_commands_reject_archived_tasks() -> anyhow::Result<()> {
    let temp = TempDb::new("label_commands_reject_archived_tasks")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;

    let add_target = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "archived add label cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let add_task_id = add_target["data"]["id"].as_str().context("task id")?;
    kanban(&temp.path, &["--json", "task", "archive", add_task_id])?.success_json()?;
    kanban(
        &temp.path,
        &[
            "--locale",
            "en",
            "--json",
            "label",
            "add",
            add_task_id,
            "backend",
        ],
    )?
    .json_failure_containing("not found: task")?;

    let remove_target = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "archived remove label cli task",
            "--description",
            "ready spec",
            "--label",
            "backend",
        ],
    )?
    .success_json()?;
    let remove_task_id = remove_target["data"]["id"].as_str().context("task id")?;
    kanban(&temp.path, &["--json", "task", "archive", remove_task_id])?.success_json()?;
    kanban(
        &temp.path,
        &[
            "--locale",
            "en",
            "--json",
            "label",
            "remove",
            remove_task_id,
            "backend",
        ],
    )?
    .json_failure_containing("not found: task")?;

    Ok(())
}

#[test]
fn task_create_rejects_invalid_priority() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_rejects_invalid_priority")?;
    kanban(&temp.path, &["init"])?.success()?;

    let output = kanban(
        &temp.path,
        &[
            "task",
            "create",
            "bad priority",
            "--description",
            "ready spec",
            "--priority",
            "70",
        ],
    )?;

    output.failure_containing("priority must be one of P0, P1, P2, P3")?;
    Ok(())
}

#[test]
fn task_show_details_does_not_change_json_output() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_does_not_change_json_output")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "json stable title",
            "--description",
            "json stable spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let default_json = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    let details_json = kanban(
        &temp.path,
        &["--json", "task", "show", task_id, "--details"],
    )?
    .success_json()?;

    assert!(default_json.get("meta").is_none());
    assert_eq!(details_json["data"], default_json["data"]);
    assert_eq!(details_json["data"]["title"], "json stable title");
    assert_eq!(details_json["data"]["description"], "json stable spec");
    assert!(details_json["meta"]["details"]["ontology_summary"].is_null());
    Ok(())
}

#[test]
fn task_show_details_json_includes_ontology_summary() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_json_includes_ontology_summary")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "ontology summary task",
            "--description",
            "ready spec for task ontology summary",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let input_path = write_cli_ontology_record_input(
        &temp,
        "task-show-ontology-summary.json",
        &[(
            "task-show-summary",
            "task show should expose ontology summary",
        )],
    )?;
    let observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            &input_path,
        ],
    )?
    .success_json()?;
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?;

    let default_json = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(default_json.get("meta").is_none());
    let details_json = kanban(
        &temp.path,
        &["--json", "task", "show", task_id, "--details"],
    )?
    .success_json()?;
    let summary = &details_json["meta"]["details"]["ontology_summary"];
    assert_eq!(summary["signal_count"], 1);
    assert_eq!(summary["open_count"], 1);
    assert_eq!(summary["confirmed_count"], 0);
    assert_eq!(summary["sample_signals"][0]["id"], signal_id);
    assert_eq!(
        summary["sample_signals"][0]["proposed_action"],
        "add_positive_atom"
    );

    let human = kanban(&temp.path, &["task", "show", task_id, "--details"])?.success_stdout()?;
    assert!(human.contains("ontology_summary:"), "{human}");
    assert!(human.contains(signal_id), "{human}");
    Ok(())
}

#[test]
fn task_update_sets_and_clears_schedule_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_sets_and_clears_scheduled_at_and_due_at")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli update dates",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let updated = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )?
    .success_json()?;
    assert_eq!(updated["data"]["scheduled_at"], 1_767_225_600_000_i64);
    assert_eq!(updated["data"]["due_at"], 1_767_312_000_000_i64);

    let cleared = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--clear-scheduled-at",
            "--clear-due-at",
        ],
    )?
    .success_json()?;
    assert!(cleared["data"]["scheduled_at"].is_null());
    assert!(cleared["data"]["due_at"].is_null());
    Ok(())
}

#[test]
fn task_create_with_invalid_max_retries_does_not_persist_task_or_event() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_with_invalid_max_retries_does_not_persist_task_or_event")?;
    kanban(&temp.path, &["init"])?.success()?;
    let before_events = kanban(&temp.path, &["--json", "events"])?.success_json()?;

    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "invalid cli retry create",
            "--description",
            "ready spec",
            "--max-retries",
            "0",
        ],
    )?
    .failure_containing("max_retries must be a positive integer")?;

    let tasks = kanban(&temp.path, &["--json", "task", "list"])?.success_json()?;
    let task_titles = tasks["data"].as_array().context("expected task array")?;
    assert!(
        task_titles
            .iter()
            .all(|task| task["title"] != "invalid cli retry create")
    );
    let after_events = kanban(&temp.path, &["--json", "events"])?.success_json()?;
    assert_eq!(
        after_events["data"]
            .as_array()
            .context("expected event array")?
            .len(),
        before_events["data"]
            .as_array()
            .context("expected event array")?
            .len()
    );
    Ok(())
}

#[test]
fn task_update_with_invalid_max_retries_does_not_persist_fields_or_event() -> anyhow::Result<()> {
    let temp =
        TempDb::new("task_update_with_invalid_max_retries_does_not_persist_fields_or_event")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "before invalid cli retry update",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let before_events = kanban(&temp.path, &["--json", "events", task_id])?.success_json()?;

    kanban(
        &temp.path,
        &[
            "task",
            "update",
            task_id,
            "--title",
            "after invalid cli retry update",
            "--max-retries",
            "0",
        ],
    )?
    .failure_containing("max_retries must be a positive integer")?;

    let fresh = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(fresh["data"]["title"], "before invalid cli retry update");
    assert_eq!(
        fresh["data"]["lock_version"],
        created["data"]["lock_version"]
    );
    let after_events = kanban(&temp.path, &["--json", "events", task_id])?.success_json()?;
    assert_eq!(
        after_events["data"]
            .as_array()
            .context("expected event array")?
            .len(),
        before_events["data"]
            .as_array()
            .context("expected event array")?
            .len()
    );
    Ok(())
}

#[test]
fn task_retry_policy_clear_event_is_consumed_by_contract_payload() -> anyhow::Result<()> {
    let temp = TempDb::new("task_retry_policy_clear_event_contract")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "clear retry policy",
            "--description",
            "ready spec",
            "--max-retries",
            "2",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;

    kanban(
        &temp.path,
        &["--json", "task", "update", task_id, "--clear-max-retries"],
    )?
    .success()?;
    let events = kanban(&temp.path, &["--json", "events", task_id])?.success_json()?;
    let event = events["data"]
        .as_array()
        .context("expected event array")?
        .iter()
        .rev()
        .find(|event| event["kind"] == "task.retry_policy.updated")
        .context("expected retry policy event")?;
    let payload = event["payload"].clone();
    assert_eq!(payload, json!({"max_retries": null}));
    let typed = kanban_contract::event_payload::EventPayload::from_kind_and_value(
        "task.retry_policy.updated",
        payload.clone(),
    )?;
    assert_eq!(serde_json::to_value(typed)?, payload);
    Ok(())
}

#[test]
fn task_complete_alias_finishes_running_task() -> anyhow::Result<()> {
    let temp = TempDb::new("task_complete_alias_finishes_like_done")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli complete alias",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, task_id)?;
    let claim = kanban(&temp.path, &["--json", "task", "claim", task_id])?.success_json()?;
    let token = claim["data"]["claim_token"]
        .as_str()
        .context("expected JSON string")?;

    let completed = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "complete",
            task_id,
            "--claim-token",
            token,
        ],
    )?
    .success_json()?;
    assert_eq!(completed["data"]["status"], "done");
    Ok(())
}

#[test]
fn task_claim_start_and_heartbeat_reject_nonpositive_ttl_ms() -> anyhow::Result<()> {
    let temp = TempDb::new("task_claim_start_and_heartbeat_reject_nonpositive_ttl_ms")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli ttl validation",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, task_id)?;

    for command in ["claim", "start"] {
        for ttl_ms in ["0", "-1"] {
            kanban(&temp.path, &["task", command, task_id, "--ttl-ms", ttl_ms])?
                .failure_containing("ttl_ms must be positive")?;
        }
    }

    let claim = kanban(&temp.path, &["--json", "task", "claim", task_id])?.success_json()?;
    let token = claim["data"]["claim_token"]
        .as_str()
        .context("expected JSON string")?;

    for ttl_ms in ["0", "-1"] {
        kanban(
            &temp.path,
            &[
                "task",
                "heartbeat",
                task_id,
                "--claim-token",
                token,
                "--ttl-ms",
                ttl_ms,
            ],
        )?
        .failure_containing("ttl_ms must be positive")?;
    }
    Ok(())
}

#[test]
fn task_reclaim_expired_alias_matches_default_reclaim() -> anyhow::Result<()> {
    let bare = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_bare")?;
    let explicit = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_explicit")?;

    for temp in [&bare, &explicit] {
        kanban(&temp.path, &["init"])?.success()?;
        let created = kanban(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli reclaim alias",
                "--description",
                "ready spec",
            ],
        )?
        .success_json()?;
        let task_id = created["data"]["id"]
            .as_str()
            .context("expected JSON string")?;
        mark_no_plan_required(&temp.path, task_id)?;
        kanban(
            &temp.path,
            &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
        )?
        .success_json()?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let bare_result = kanban(&bare.path, &["--json", "task", "reclaim"])?.success_json()?;
    let explicit_result =
        kanban(&explicit.path, &["--json", "task", "reclaim", "--expired"])?.success_json()?;

    assert_eq!(bare_result, explicit_result);
    assert_eq!(explicit_result["data"]["reclaimed"], 1);
    Ok(())
}

#[test]
fn task_step_commands_manage_text_steps() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_commands_manage_text_steps")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step parent",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;

    let listed =
        kanban(&temp.path, &["--json", "task", "step", "list", task_id])?.success_json()?;
    assert_eq!(listed["data"]["execution_plan"]["state"], "unplanned");
    assert!(
        listed["data"]["steps"]
            .as_array()
            .context("steps")?
            .is_empty()
    );

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "Draft execution plan",
            "--body",
            "write the concrete path",
        ],
    )?
    .success_json()?;
    assert_eq!(added["data"]["title"], "Draft execution plan");
    assert_eq!(added["data"]["status"], "todo");
    assert_eq!(added["data"]["required"], true);

    let human = kanban(&temp.path, &["task", "step", "list", task_id])?.success_stdout()?;
    assert!(human.contains("Execution plan: planned"), "{human}");
    assert!(
        human.contains("Required steps: 0/1 done-or-skipped"),
        "{human}"
    );
    assert!(human.contains("S1 "), "{human}");
    assert!(human.contains("Draft execution plan"), "{human}");

    let done = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "done",
            task_id,
            "S1",
            "--note",
            "implemented",
        ],
    )?
    .success_json()?;
    assert_eq!(done["data"]["status"], "done");
    assert_eq!(done["data"]["resolution_note"], "implemented");

    let reopened = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "reopen",
            task_id,
            "S1",
            "--reason",
            "needs another pass",
        ],
    )?
    .success_json()?;
    assert_eq!(reopened["data"]["status"], "todo");
    assert!(reopened["data"]["resolution_note"].is_null());

    let updated = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "update",
            task_id,
            "S1",
            "--optional",
            "--position",
            "4096",
        ],
    )?
    .success_json()?;
    assert_eq!(updated["data"]["required"], false);
    assert_eq!(updated["data"]["position"], 4096);

    kanban(&temp.path, &["task", "step", "remove", task_id, "S1"])?.success()?;
    let empty = kanban(&temp.path, &["--json", "task", "step", "list", task_id])?.success_json()?;
    assert_eq!(empty["data"]["execution_plan"]["state"], "unplanned");
    assert!(
        empty["data"]["steps"]
            .as_array()
            .context("steps")?
            .is_empty()
    );

    let plan = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "not-required",
            task_id,
            "--reason",
            "tiny task",
        ],
    )?
    .success_json()?;
    assert_eq!(plan["data"]["state"], "not_required");
    assert_eq!(plan["data"]["reason"], "tiny task");
    Ok(())
}

#[test]
fn task_step_required_accepts_bounded_boolean_value_forms() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_required_accepts_bounded_boolean_value_forms")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step boolean parent",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;

    let explicit_true = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "Explicit true required",
            "--required",
            "true",
        ],
    )?
    .success_json()?;
    assert_eq!(explicit_true["data"]["required"], true);

    let explicit_false = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "Explicit false optional",
            "--required=false",
        ],
    )?
    .success_json()?;
    assert_eq!(explicit_false["data"]["required"], false);

    let updated_false = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "update",
            task_id,
            "S1",
            "--required",
            "false",
        ],
    )?
    .success_json()?;
    assert_eq!(updated_false["data"]["required"], false);

    let updated_true = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "update",
            task_id,
            "S1",
            "--required=true",
        ],
    )?
    .success_json()?;
    assert_eq!(updated_true["data"]["required"], true);

    let title_after_flag = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "--required",
            "Title after flag remains positional",
        ],
    )?
    .success_json()?;
    assert_eq!(
        title_after_flag["data"]["title"],
        "Title after flag remains positional"
    );
    assert_eq!(title_after_flag["data"]["required"], true);

    let uppercase_title_after_flag = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "--required",
            "True",
        ],
    )?
    .success_json()?;
    assert_eq!(uppercase_title_after_flag["data"]["title"], "True");
    assert_eq!(uppercase_title_after_flag["data"]["required"], true);

    let uppercase_false_title_after_flag = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "--required",
            "FALSE",
        ],
    )?
    .success_json()?;
    assert_eq!(uppercase_false_title_after_flag["data"]["title"], "FALSE");
    assert_eq!(uppercase_false_title_after_flag["data"]["required"], true);

    kanban(
        &temp.path,
        &[
            "task",
            "step",
            "add",
            task_id,
            "Invalid boolean value",
            "--required",
            "maybe",
        ],
    )?
    .failure_containing("unexpected argument 'maybe'")?;

    Ok(())
}

#[test]
fn task_create_positional_title_after_delimiter_is_not_normalized_as_required_flag()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "task_create_positional_title_after_delimiter_is_not_normalized_as_required_flag",
    )?;
    kanban(&temp.path, &["init"])?.success()?;

    let created = kanban(
        &temp.path,
        &["--json", "task", "create", "--", "--required=false"],
    )?
    .success_json()?;
    assert_eq!(created["data"]["title"], "--required=false");

    Ok(())
}

#[test]
fn task_step_add_positional_title_after_delimiter_is_not_normalized_as_required_flag()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "task_step_add_positional_title_after_delimiter_is_not_normalized_as_required_flag",
    )?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step delimiter parent",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;

    let step = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "--",
            "--required=false",
        ],
    )?
    .success_json()?;
    assert_eq!(step["data"]["title"], "--required=false");
    assert_eq!(step["data"]["required"], true);

    Ok(())
}

#[test]
fn task_step_linked_task_is_context_only() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_linked_task_is_context_only")?;
    kanban(&temp.path, &["init"])?.success()?;
    let parent = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "parent",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let child = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "child",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let parent_id = parent["data"]["id"].as_str().context("parent id")?;
    let child_id = child["data"]["id"].as_str().context("child id")?;

    let step = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            parent_id,
            "Review child output",
            "--link-task",
            child_id,
        ],
    )?
    .success_json()?;
    assert_eq!(step["data"]["linked_task"]["id"], child_id);
    assert_eq!(step["data"]["status"], "todo");

    mark_no_plan_required(&temp.path, child_id)?;
    let claim = kanban(&temp.path, &["--json", "task", "claim", child_id])?.success_json()?;
    let token = claim["data"]["claim_token"]
        .as_str()
        .context("claim token")?;
    kanban(
        &temp.path,
        &["task", "complete", child_id, "--claim-token", token],
    )?
    .success()?;
    let listed =
        kanban(&temp.path, &["--json", "task", "step", "list", parent_id])?.success_json()?;
    assert_eq!(listed["data"]["steps"][0]["linked_task"]["status"], "done");
    assert_eq!(listed["data"]["steps"][0]["status"], "todo");
    Ok(())
}

#[test]
fn task_create_description_file_preserves_shell_sensitive_text() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_description_file_preserves_shell_sensitive_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    let description = "`code` $VAR $(date) {\"json\":true}\nline \\\\ slash 'single' \"double\" \"nested 'quote'\"";
    let description_path = temp.dir.join("description.md");
    fs::write(&description_path, description)?;

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "file description",
            "--description-file",
            description_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;

    assert_eq!(created["data"]["description"], description);
    Ok(())
}

#[test]
fn task_update_description_stdin_preserves_shell_sensitive_text() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_description_stdin_preserves_shell_sensitive_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "stdin description",
            "--description",
            "initial",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;
    let description =
        "stdin `code` $VAR $(date) {\"json\":true}\nline \\\\ slash 'single' \"double\"";

    let updated = kanban_with_stdin(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--description-file",
            "-",
        ],
        description,
    )?
    .success_json()?;

    assert_eq!(updated["data"]["description"], description);
    Ok(())
}

#[test]
fn task_create_rejects_inline_description_with_description_file() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_rejects_inline_description_with_description_file")?;
    kanban(&temp.path, &["init"])?.success()?;
    let description_path = temp.dir.join("description.md");
    fs::write(&description_path, "from file")?;

    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "bad description",
            "--description",
            "inline",
            "--description-file",
            description_path.to_str().context("utf-8 path")?,
        ],
    )?
    .failure_containing("mutually exclusive")?;
    Ok(())
}

#[test]
fn task_step_body_file_and_stdin_preserve_shell_sensitive_text() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_body_file_and_stdin_preserve_shell_sensitive_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step body",
            "--description",
            "spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;
    let file_body = "file `code` $VAR $(date) {\"json\":true}\nline \\\\ slash 'single' \"double\"";
    let body_path = temp.dir.join("step-body.md");
    fs::write(&body_path, file_body)?;

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "file step",
            "--body-file",
            body_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(added["data"]["body"], file_body);
    let step_id = added["data"]["id"].as_str().context("expected step id")?;

    let stdin_body =
        "stdin `code` $VAR $(date) {\"json\":true}\nline \\\\ slash 'single' \"double\"";
    let updated = kanban_with_stdin(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "update",
            task_id,
            step_id,
            "--body-file",
            "-",
        ],
        stdin_body,
    )?
    .success_json()?;

    assert_eq!(updated["data"]["body"], stdin_body);
    Ok(())
}

#[test]
fn task_step_resolution_and_block_reason_files_preserve_shell_sensitive_text() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("task_step_resolution_and_block_reason_files_preserve_shell_sensitive_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step resolution files",
            "--description",
            "spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;

    let done_step = kanban(
        &temp.path,
        &["--json", "task", "step", "add", task_id, "done step"],
    )?
    .success_json()?;
    let done_step_id = done_step["data"]["id"]
        .as_str()
        .context("expected step id")?;
    let note = "done from file `code` $VAR $(date)\nsecond line";
    let note_path = temp.dir.join("step-note.md");
    fs::write(&note_path, note)?;
    let completed = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "done",
            task_id,
            done_step_id,
            "--note-file",
            note_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(completed["data"]["resolution_note"], note);

    let skip_step = kanban(
        &temp.path,
        &["--json", "task", "step", "add", task_id, "skip step"],
    )?
    .success_json()?;
    let skip_step_id = skip_step["data"]["id"]
        .as_str()
        .context("expected step id")?;
    let reason = "skip from stdin {\"json\":true}\n$VAR remains literal";
    let skipped = kanban_with_stdin(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "skip",
            task_id,
            skip_step_id,
            "--reason-file",
            "-",
        ],
        reason,
    )?
    .success_json()?;
    assert_eq!(skipped["data"]["resolution_note"], reason);

    let blocked_task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "block reason file",
            "--description",
            "spec",
        ],
    )?
    .success_json()?;
    let blocked_task_id = blocked_task["data"]["id"]
        .as_str()
        .context("expected task id")?;
    let block_reason = "blocked by shell-sensitive evidence: `cmd` $(date)\nsecond line";
    let block_reason_path = temp.dir.join("block-reason.md");
    fs::write(&block_reason_path, block_reason)?;
    let blocked = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "block",
            blocked_task_id,
            "--reason-file",
            block_reason_path.to_str().context("utf-8 path")?,
            "--force",
        ],
    )?
    .success_json()?;
    assert_eq!(blocked["data"]["status_reason"], block_reason);
    Ok(())
}

#[test]
fn task_step_done_rejects_inline_note_with_note_file() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_done_rejects_inline_note_with_note_file")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step note conflict",
            "--description",
            "spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;
    let added = kanban(
        &temp.path,
        &["--json", "task", "step", "add", task_id, "conflict step"],
    )?
    .success_json()?;
    let step_id = added["data"]["id"].as_str().context("expected step id")?;
    let note_path = temp.dir.join("step-note.md");
    fs::write(&note_path, "from file")?;

    kanban(
        &temp.path,
        &[
            "task",
            "step",
            "done",
            task_id,
            step_id,
            "--note",
            "inline",
            "--note-file",
            note_path.to_str().context("utf-8 path")?,
        ],
    )?
    .failure_containing("mutually exclusive")?;
    Ok(())
}

#[test]
fn task_step_update_rejects_inline_body_with_body_file() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_update_rejects_inline_body_with_body_file")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "step body conflict",
            "--description",
            "spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("expected task id")?;
    let added = kanban(
        &temp.path,
        &["--json", "task", "step", "add", task_id, "conflict step"],
    )?
    .success_json()?;
    let step_id = added["data"]["id"].as_str().context("expected step id")?;
    let body_path = temp.dir.join("step-body.md");
    fs::write(&body_path, "from file")?;

    kanban(
        &temp.path,
        &[
            "task",
            "step",
            "update",
            task_id,
            step_id,
            "--body",
            "inline",
            "--body-file",
            body_path.to_str().context("utf-8 path")?,
        ],
    )?
    .failure_containing("mutually exclusive")?;
    Ok(())
}

#[test]
fn task_metadata_file_and_stdin_preserve_shell_sensitive_json() -> anyhow::Result<()> {
    let temp = TempDb::new("task_metadata_file_and_stdin_preserve_shell_sensitive_json")?;
    kanban(&temp.path, &["init"])?.success()?;
    let metadata =
        r#"{"source":"file","literal":"$VAR $(date)","nested":{"quote":"'single' \"double\""}}"#;
    let metadata_path = temp.dir.join("task-metadata.json");
    fs::write(&metadata_path, metadata)?;

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "metadata file task",
            "--metadata-file",
            metadata_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(
        created["data"]["metadata"],
        serde_json::from_str::<serde_json::Value>(metadata)?
    );
    let task_id = created["data"]["id"].as_str().context("expected task id")?;

    let stdin_metadata =
        r#"{"source":"stdin","literal":"$VAR $(date)","array":["back\\slash","nested 'quote'"]}"#;
    let updated = kanban_with_stdin(
        &temp.path,
        &["--json", "task", "update", task_id, "--metadata-file", "-"],
        stdin_metadata,
    )?
    .success_json()?;

    assert_eq!(
        updated["data"]["metadata"],
        serde_json::from_str::<serde_json::Value>(stdin_metadata)?
    );
    Ok(())
}
