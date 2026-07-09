use std::path::Path;

use crate::common::*;
use kanban_sqlite::api;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalCounts {
    tasks: i64,
    labels: i64,
    task_labels: i64,
    label_semantics: i64,
    label_atoms: i64,
    label_ontology_actions: i64,
    task_runs: i64,
}

#[tokio::test]
async fn api_adapter_contract_commits_to_shared_canonical_state() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let (status, label_json) = post_json(
        app.clone(),
        "/api/v1/boards/default/labels",
        json!({"name":"adapter-contract","color":"#335577"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{label_json}");
    let label_id = label_json["data"]["id"]
        .as_str()
        .context("label id")?
        .to_owned();

    let (status, task_json) = post_json(
        app.clone(),
        "/api/v1/boards/default/tasks",
        json!({
            "title": "adapter contract task",
            "description": "exercise API adapter contract",
            "actor": "api-adapter"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{task_json}");
    let task_id = task_json["data"]["id"]
        .as_str()
        .context("task id")?
        .to_owned();
    mark_plan_not_required_for_test(&db_path, "default", "api-adapter", &task_id)?;
    assert_eq!(
        api::get_task(&db_path, "default", &task_id)?.status,
        kanban_core::TaskStatus::Ready
    );

    let (status, label_add_json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{task_id}/labels"),
        json!({"name":"adapter-contract","actor":"api-adapter"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{label_add_json}");
    let task_after_label = api::get_task(&db_path, "default", &task_id)?;
    assert_eq!(task_after_label.labels.len(), 1);
    assert_eq!(task_after_label.labels[0].id, label_id);
    assert_eq!(task_after_label.labels[0].name, "adapter-contract");

    let (status, semantics_json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
        Some(json!({
            "description": "Adapter contract label semantics",
            "applies_when": ["checks CLI/API/dispatcher use-case parity"],
            "positive_examples": ["API route writes canonical SQLite state"],
            "reason": "adapter contract test"
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{semantics_json}");
    let semantics = api::get_label_semantics_by_id(&db_path, "default", &label_id)?;
    assert_eq!(
        semantics.semantics_hash,
        semantics_json["data"]["semantics_hash"]
            .as_str()
            .context("semantics hash")?
    );
    assert_eq!(
        semantics.applies_when,
        vec!["checks CLI/API/dispatcher use-case parity".to_owned()]
    );
    let applies_atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    assert!(
        ontology_atom_effect_count(&db_path, &applies_atom.content_hash, "added")? > 0,
        "new semantics atoms must have ledger effect provenance"
    );

    let (status, observation_json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{task_id}/label-ontology/observations"),
        json!({
            "actor": {"name":"label-agent","type":"agent","agent_type":"contract"},
            "agent_candidates": [],
            "suggestion_snapshot": {
                "coverage": 0.4,
                "coverage_cosine": 0.3,
                "residual_norm": 0.6,
                "needs_new_label": false,
                "degraded": false,
                "diagnostics": []
            },
            "final_decision": {"labels":["adapter-contract"]},
            "diagnostics": [],
            "capture_fingerprint": "adapter-contract-api-observation",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "adapter-contract",
                "related_labels": [],
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "exercises adapter parity for shared use cases"
                },
                "proposal": {},
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.42,
                "suggest_rank": 2,
                "final_selected": true,
                "rationale": "The task exercises adapter parity.",
                "confidence": 0.9,
                "signal_key": "adapter-contract-false-negative"
            }]
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{observation_json}");
    let signal_id = observation_json["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?
        .to_owned();

    let (status, action_json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/actions",
        json!({
            "actor": {"name":"api-reviewer","type":"user","agent_type":null},
            "action_type": "confirm",
            "signal_ids": [signal_id],
            "reason": "adapter contract confirms the signal through the API"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{action_json}");
    let signal_detail = api::get_label_ontology_signal(
        &db_path,
        action_json["data"]["signal_ids"][0]
            .as_str()
            .context("action signal id")?,
    )?;
    assert_eq!(
        signal_detail.signal.status,
        api::LabelOntologySignalStatus::Confirmed
    );
    assert_eq!(signal_detail.actions.len(), 1);
    assert_eq!(signal_detail.actions[0].created_by, "api-reviewer");

    let (status, claim_json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{task_id}/transitions/claim"),
        json!({"actor":"api-worker","ttl_ms":60000,"worker_profile":"contract"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{claim_json}");
    let claim_token = claim_json["data"]["claim_token"]
        .as_str()
        .context("claim token")?
        .to_owned();

    let (status, complete_json) = post_json(
        app,
        &format!("/api/v1/tasks/{task_id}/transitions/complete"),
        json!({"claim_token": claim_token, "summary":"contract complete", "force":false}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{complete_json}");
    let completed = api::get_task(&db_path, "default", &task_id)?;
    assert_eq!(completed.status, kanban_core::TaskStatus::Done);
    assert!(completed.claim_token.is_none());
    assert_eq!(completed.labels.len(), 1);
    let runs = api::list_runs(&db_path, "default", Some(&task_id))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "succeeded");
    assert_eq!(runs[0].worker_profile.as_deref(), Some("contract"));

    Ok(())
}

#[test]
fn dispatcher_adapter_contract_uses_shared_transition_service() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task =
        create_ready_task_for_test(&db_path, "default", "seed", "dispatcher adapter contract")?;

    let result = api::dispatch_once(
        &db_path,
        "default",
        api::DispatchOptions {
            actor: "dispatcher".to_owned(),
            command: "true".to_owned(),
            worker_profile: "contract-dispatcher".to_owned(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 30_000,
            on_success: api::FinishPolicy::Done,
            on_failure: api::FinishPolicy::Blocked,
            log_dir: test.dir_path().join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 1);
    assert_eq!(result.task_id.as_deref(), Some(task.id.as_str()));
    let completed = api::get_task(&db_path, "default", &task.id)?;
    assert_eq!(completed.status, kanban_core::TaskStatus::Done);
    assert!(completed.claim_token.is_none());
    assert_eq!(completed.claim_owner, None);
    assert_eq!(
        completed.current_run_id.as_deref(),
        result.run_id.as_deref()
    );

    let runs = api::list_runs(&db_path, "default", Some(&task.id))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "succeeded");
    assert_eq!(
        runs[0].worker_profile.as_deref(),
        Some("contract-dispatcher")
    );
    assert_eq!(runs[0].id.as_str(), result.run_id.as_deref().unwrap());

    let events = api::list_events_after(
        &db_path,
        "default",
        api::EventListOptions {
            task_ref: Some(task.id.clone()),
            after: 0,
            limit: 20,
        },
    )?;
    assert!(
        events.iter().any(|event| {
            event.kind == "task.claimed" && event.run_id.as_deref() == result.run_id.as_deref()
        }),
        "events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.kind == "task.completed" && event.run_id.as_deref() == result.run_id.as_deref()
        }),
        "events: {events:?}"
    );

    Ok(())
}

#[tokio::test]
async fn derived_adapter_contract_does_not_write_canonical_label_truth() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let label = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "derived-contract".to_owned(),
            color: None,
        },
    )?;
    kanban_sqlite::api::upsert_label_semantics_by_id(
        &db_path,
        "default",
        &label.id,
        kanban_sqlite::api::UpsertLabelSemantics {
            label_ref: label.id.clone(),
            description: Some("Derived contract label".to_owned()),
            applies_when: vec!["checks derived adapters".to_owned()],
            ..kanban_sqlite::api::UpsertLabelSemantics::default()
        },
    )?;
    let before = canonical_counts(&db_path)?;
    let app = test.router();

    let (status, status_json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/status",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{status_json}");
    assert_eq!(status_json["data"]["enabled"], false);
    assert_eq!(canonical_counts(&db_path)?, before);

    let (status, rebuild_json) = post_json(
        app,
        "/api/v1/boards/default/labels/atom-index/rebuild",
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rebuild_json}");
    assert_eq!(rebuild_json["error"]["code"], "invalid_input");
    assert!(
        rebuild_json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("vector helper")
    );
    assert_eq!(
        canonical_counts(&db_path)?,
        before,
        "derived atom-index routes must not mutate canonical label truth"
    );

    Ok(())
}

fn canonical_counts(path: &Path) -> anyhow::Result<CanonicalCounts> {
    let conn = kanban_test_support::connect_file(path)?;
    let count = |table: &str| -> anyhow::Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(conn.query_row(&sql, [], |row| row.get(0))?)
    };
    Ok(CanonicalCounts {
        tasks: count("tasks")?,
        labels: count("labels")?,
        task_labels: count("task_labels")?,
        label_semantics: count("label_semantics")?,
        label_atoms: count("label_atoms")?,
        label_ontology_actions: count("label_ontology_actions")?,
        task_runs: count("task_runs")?,
    })
}

fn ontology_atom_effect_count(
    path: &Path,
    atom_content_hash: &str,
    effect: &str,
) -> anyhow::Result<i64> {
    let conn = kanban_test_support::connect_file(path)?;
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_action_atom_effects \
         WHERE atom_content_hash=?1 AND effect=?2",
        (atom_content_hash, effect),
        |row| row.get(0),
    )?)
}
