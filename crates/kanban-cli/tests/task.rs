mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_sqlite::LabelProposalCandidate;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;

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

    let added =
        kanban(&temp.path, &["--json", "label", "add", task_id, "frontend"])?.success_json()?;
    assert_eq!(
        added["data"]["labels"].as_array().context("labels")?.len(),
        2
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
        [serde_json::json!("backend"), serde_json::json!("frontend")]
    );
    let listed_human = kanban(&temp.path, &["label", "list"])?.success_stdout()?;
    assert!(listed_human.contains("backend "), "{listed_human}");
    assert!(listed_human.contains(" color=-"), "{listed_human}");
    assert!(listed_human.contains("frontend "), "{listed_human}");
    assert!(listed_human.contains(" color=#4477aa"), "{listed_human}");

    let human = kanban(&temp.path, &["task", "show", task_id])?.success_stdout()?;
    assert!(human.contains("[backend,frontend]"), "{human}");

    let removed = kanban(
        &temp.path,
        &["--json", "label", "remove", task_id, "frontend"],
    )?
    .success_json()?;
    assert_eq!(removed["data"]["labels"][0]["name"], "backend");
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

    let atoms = kanban(&temp.path, &["--json", "label", "atoms", "list"])?.success_json()?;
    assert!(
        atoms["data"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["kind"] == "positive_example" && atom["text"] == "add API handler")
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

    assert_eq!(details_json, default_json);
    assert_eq!(details_json["data"]["title"], "json stable title");
    assert_eq!(details_json["data"]["description"], "json stable spec");
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
