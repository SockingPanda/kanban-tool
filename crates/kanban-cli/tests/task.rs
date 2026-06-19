mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_sqlite::{
    CreateLabel, CreateTask, LabelOntologyActionInput, LabelOntologyActionType, LabelOntologyActor,
    LabelOntologyAtomApplyInput, LabelOntologyCandidateAtomInput, LabelOntologyProposedAction,
    LabelOntologyRecordInput, LabelOntologySignalInput, LabelOntologySignalKind,
    LabelOntologySuggestState, LabelProposalCandidate, UpsertLabelSemantics, get_task,
    list_label_atoms, list_labels,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::{fs, path::Path};

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

    let stdout = kanban(&temp.path, &["task", "show", task_id])?.success_stdout()?;

    assert_eq!(
        stdout,
        format!("default#1 {task_id} [ready] show summary title\n")
    );
    assert!(!stdout.contains("line one"), "{stdout}");
    assert_eq!(stdout.lines().count(), 1);
    Ok(())
}

#[test]
fn task_show_details_prints_full_readable_record() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_prints_full_readable_record")?;
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

    let stdout = kanban(&temp.path, &["task", "show", task_id, "--details"])?.success_stdout()?;

    assert!(stdout.contains("ref: default#1"), "{stdout}");
    assert!(stdout.contains(&format!("id: {task_id}")), "{stdout}");
    assert!(stdout.contains("status: ready"), "{stdout}");
    assert!(stdout.contains("title: detailed task title"), "{stdout}");
    assert!(stdout.contains("labels: -"), "{stdout}");
    assert!(stdout.contains("assignee: executor"), "{stdout}");
    assert!(stdout.contains("priority: P1"), "{stdout}");
    assert!(stdout.contains("scheduled_at: 1767225600000"), "{stdout}");
    assert!(stdout.contains("due_at: 1767312000000"), "{stdout}");
    assert!(stdout.contains("created_at: "), "{stdout}");
    assert!(stdout.contains("updated_at: "), "{stdout}");
    assert!(
        stdout.contains("description:\n  first detail line\n  second detail line"),
        "{stdout}"
    );
    Ok(())
}

#[test]
fn task_create_and_label_commands_round_trip_labels() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_and_label_commands_round_trip_labels")?;
    kanban(&temp.path, &["init"])?.success()?;
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

    let status =
        kanban(&temp.path, &["--json", "label", "atom-index", "status"])?.success_json()?;
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
    #[cfg(feature = "vector-lancedb")]
    let expected_index_failure = "Ollama embed request failed";
    #[cfg(not(feature = "vector-lancedb"))]
    let expected_index_failure = "requires a configured label atom vector store";
    kanban(
        &temp.path,
        &[
            "label",
            "atom-index",
            "rebuild",
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
    )?
    .failure_containing(expected_index_failure)?;
    kanban(
        &temp.path,
        &[
            "label",
            "atom-index",
            "query",
            "backend",
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
    )?
    .failure_containing(expected_index_failure)?;

    let deleted = kanban(
        &temp.path,
        &["--json", "label", "semantics", "delete", "backend"],
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
    .failure_containing(
        "label bootstrap verification requires a configured label atom vector store",
    )?;

    let shown = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(
        shown["data"]["labels"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_bootstrap_verify_rebuild_failure_restores_canonical_state() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_verify_rebuild_failure_restores_canonical_state")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bootstrap verify rebuild failure cli task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let vector_config = temp.dir.join("vector.toml");
    fs::write(
        &vector_config,
        r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "offline-cli-test-model"
dimensions = 3
"#,
    )?;

    let attempt = kanban(
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
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
    )?;

    assert!(!attempt.output.status.success());
    let stderr = String::from_utf8_lossy(&attempt.output.stderr);
    assert!(stderr.contains("Ollama embed request failed"), "{stderr}");
    assert!(
        stderr.contains("bootstrap verification compensation restored canonical state"),
        "{stderr}"
    );
    assert!(stderr.contains("label_deleted=true"), "{stderr}");
    let shown = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert!(
        shown["data"]["labels"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    assert!(
        kanban(&temp.path, &["--json", "label", "list"])?.success_json()?["data"]
            .as_array()
            .context("labels")?
            .is_empty()
    );
    let conn = kanban_sqlite::connect_file(&temp.path)?;
    let labels: i64 = conn.query_row("SELECT COUNT(*) FROM labels", [], |row| row.get(0))?;
    let semantics: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_semantics", [], |row| row.get(0))?;
    let atoms: i64 = conn.query_row("SELECT COUNT(*) FROM label_atoms", [], |row| row.get(0))?;
    let bindings: i64 = conn.query_row("SELECT COUNT(*) FROM task_labels", [], |row| row.get(0))?;
    assert_eq!((labels, semantics, atoms, bindings), (0, 0, 0, 0));
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.board_dirty, Some(true));
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
    kanban(
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

    kanban(&temp.path, &["label", "delete", "database"])?
        .failure_containing("attached to 1 task(s)")?;

    let deleted = kanban(
        &temp.path,
        &["--json", "label", "delete", "database", "--force"],
    )?
    .success_json()?;
    assert_eq!(deleted["data"]["label"]["name"], "database");
    assert_eq!(deleted["data"]["forced"], true);
    assert_eq!(deleted["data"]["removed_task_bindings"], 1);
    assert_eq!(deleted["data"]["removed_semantics"], true);
    assert!(
        deleted["data"]["removed_atoms"]
            .as_i64()
            .context("removed atoms")?
            > 0
    );

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
fn label_propose_with_proposal_json_degrades_without_polluting_truth() -> anyhow::Result<()> {
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
    let proposal_path = temp.dir.join("candidate.json");
    std::fs::write(
        &proposal_path,
        json!({
            "name": "workflow",
            "description": "Workflow classification",
            "applies_when": ["classifies execution flow"],
            "excludes_when": ["UI-only polish"],
            "positive_examples": ["triage work queue"],
            "negative_examples": ["CSS tweak"]
        })
        .to_string(),
    )?;

    let attempt = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "propose",
            task_id,
            "--proposal-json",
            proposal_path.to_str().context("proposal json path")?,
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
    let conn = kanban_sqlite::connect_file(&temp.path)?;
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

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_propose_with_vector_config_attempts_configured_store() -> anyhow::Result<()> {
    let temp = TempDb::new("label_propose_with_vector_config_attempts_configured_store")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "proposal configured vector cli task",
            "--description",
            "ready spec",
            "--status",
            "ready",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
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

    let attempt = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "propose",
            task_id,
            "--vector-config",
            vector_config.to_str().context("vector config path")?,
        ],
    )?
    .success_json()?;

    let diagnostics = attempt["data"]["diagnostics"]
        .as_array()
        .context("diagnostics")?;
    assert_eq!(attempt["data"]["proposal"], serde_json::Value::Null);
    assert_eq!(attempt["data"]["degraded"], true);
    assert!(
        diagnostics
            .iter()
            .any(|value| value == "vector_query_error"),
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|value| value == "vector_store_disabled"),
        "{diagnostics:?}"
    );
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
    let semantics = kanban_sqlite::get_label_semantics(&temp.path, "default", label_id)?;
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
fn label_ontology_cli_record_list_show_review_round_trip() -> anyhow::Result<()> {
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

    let input_path = temp.dir.join("ontology-record.json");
    let agent_candidates_json = json!([
        {"label": "cli", "confidence": 0.92}
    ])
    .to_string();
    let suggestion_snapshot_json = json!({
        "selected_labels": []
    })
    .to_string();
    let final_decision_json = json!({
        "accepted_labels": ["cli"]
    })
    .to_string();
    let diagnostics_json = json!([]).to_string();
    let related_labels_json = json!([]).to_string();
    let proposal_json = json!({}).to_string();
    fs::write(
        &input_path,
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates_json": agent_candidates_json,
            "suggestion_snapshot_json": suggestion_snapshot_json,
            "final_decision_json": final_decision_json,
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": diagnostics_json,
            "capture_fingerprint": "cli-ontology-round-trip",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels_json": related_labels_json,
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
                },
                "proposed_label_name": null,
                "proposal_json": proposal_json,
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The task expands the CLI surface although suggest scored cli weakly.",
                "confidence": 0.91,
                "signal_key": "cli-false-negative"
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
    let input_path = temp.dir.join("ontology-simplified-record.json");
    fs::write(
        &input_path,
        json!({
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
        .to_string(),
    )?;
    let input_path = input_path
        .to_str()
        .context("temp input path should be valid UTF-8")?;
    let snapshot_path = snapshot_path
        .to_str()
        .context("temp snapshot path should be valid UTF-8")?;

    let observation = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            task_id,
            "--input",
            input_path,
            "--suggestion-snapshot",
            snapshot_path,
        ],
    )?
    .success_json()?;

    assert_eq!(observation["data"]["suggest_coverage"], 0.42);
    assert_eq!(observation["data"]["suggest_coverage_cosine"], 0.37);
    assert_eq!(observation["data"]["suggest_residual_norm"], 0.58);
    assert_eq!(observation["data"]["suggest_needs_new_label"], true);
    assert_eq!(observation["data"]["suggest_degraded"], true);
    let diagnostics: serde_json::Value = serde_json::from_str(
        observation["data"]["diagnostics_json"]
            .as_str()
            .context("diagnostics_json")?,
    )?;
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

    let confirmed = kanban(
        &temp.path,
        &[
            "--json",
            "label",
            "ontology",
            "confirm",
            primary,
            "--reason",
            "Reviewer confirmed the false negative.",
        ],
    )?
    .success_json()?;
    assert_eq!(confirmed["data"]["action_type"], "confirm");
    assert_eq!(confirmed["data"]["signal_ids"][0], primary);

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
            "--text",
            "extends CLI subcommands, arguments, help output, or JSON behavior",
            "--reason",
            "Confirmed false-negative support for CLI surface changes.",
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
        ],
    )?
    .success_json()?;
    assert_eq!(applied["data"]["action_type"], "add_positive_atom");
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
            "evidence_type": "automated",
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
            "--reason",
            "The source task now selects cli with the new atom as evidence.",
            "--input",
            validation_path,
            "--actor-type",
            "agent",
            "--agent-type",
            "codex",
        ],
    )?
    .failure_containing("trusted evidence collected by the kanban tool")?;

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
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_coverage": 0.2,
            "suggest_coverage_cosine": 0.3,
            "suggest_residual_norm": 0.8,
            "suggest_needs_new_label": true,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "cli-proposal-gap",
            "signals": [{
                "kind": "vocabulary_gap",
                "target_label_ref": null,
                "related_labels_json": "[]",
                "proposed_action": "bootstrap_label",
                "candidate_atom": null,
                "proposed_label_name": "ontology-ledger",
                "proposal_json": "{\"name\":\"ontology-ledger\"}",
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
    let change: serde_json::Value = serde_json::from_str(
        bootstrap["change_json"]
            .as_str()
            .context("bootstrap change_json")?,
    )?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "Reviewer explicitly audited proposal source signal retarget."
    );

    Ok(())
}

#[test]
fn label_ontology_cli_apply_existing_atom_uses_adopt_existing_action() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_cli_apply_existing_atom_uses_adopt_existing_action")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "cli"])?.success()?;
    kanban_sqlite::upsert_label_semantics(
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
    let change: serde_json::Value = serde_json::from_str(
        applied["data"]["change_json"]
            .as_str()
            .context("change_json")?,
    )?;
    assert_eq!(change["canonical_changed"], false);
    assert_eq!(change["provenance_only"], true);
    assert_eq!(change["requested_action_type"], "add_positive_atom");
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(add_atom_action_count(&temp.path)?, add_action_count_before);
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
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
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_coverage": 0.2,
            "suggest_coverage_cosine": 0.3,
            "suggest_residual_norm": 0.8,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "cli-retarget-override",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "backend",
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends backend persistence or service APIs"
                },
                "proposed_label_name": null,
                "proposal_json": "{}",
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
    let change: serde_json::Value = serde_json::from_str(
        applied["data"]["change_json"]
            .as_str()
            .context("change_json")?,
    )?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "Signal captures a CLI boundary despite backend wording."
    );
    assert_eq!(change["retarget_override"]["target_label"]["name"], "cli");

    Ok(())
}

#[test]
fn label_atom_explain_cli_json_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_explain_cli_json_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;
    let label = kanban_sqlite::create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("explain atom CLI provenance"),
    )?;
    let observation = kanban_sqlite::record_label_ontology_observation(
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
    kanban_sqlite::create_label_ontology_action(
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
    let applied = kanban_sqlite::apply_label_ontology_atom(
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
    let board = kanban_sqlite::get_board(path, board)?;
    let conn = kanban_sqlite::connect_file(path)?;
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
    Ok(kanban_sqlite::connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions \
         WHERE action_type IN ('add_positive_atom','add_negative_atom')",
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
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
                },
                "proposed_label_name": null,
                "proposal_json": "{}",
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
            "agent_candidates_json": "[{\"label\":\"cli\",\"confidence\":0.92}]",
            "suggestion_snapshot_json": "{\"selected_labels\":[]}",
            "final_decision_json": "{\"accepted_labels\":[\"cli\"]}",
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
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
    let conn = kanban_sqlite::connect_file(db_path)?;
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
        &["label", "suggest", task_id, "--limit", "1001"],
    )?
    .failure_containing("limit must be <= 1000")?;
    kanban(
        &temp.path,
        &["label", "suggest", task_id, "--atom-limit", "1001"],
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
        &["--json", "label", "add", add_task_id, "backend"],
    )?
    .failure_containing("not found: task")?;

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
        &["--json", "label", "remove", remove_task_id, "backend"],
    )?
    .failure_containing("not found: task")?;

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
