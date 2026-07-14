mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliDoctorOutput, CliStatsOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(
        "schemas/fixtures/cli/{operation}-output.v1.valid.json"
    ));
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?)?;
    Ok(())
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn reject_internal_keys(value: &Value) -> anyhow::Result<()> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                anyhow::ensure!(
                    !matches!(
                        key.as_str(),
                        "db_path" | "log_path" | "claim_token" | "lock_path"
                    ),
                    "internal key leaked: {key}"
                );
                reject_internal_keys(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_internal_keys(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn require_zero_counts(data: &serde_json::Map<String, Value>, keys: &[&str]) -> anyhow::Result<()> {
    for key in keys {
        let count = data[*key]
            .as_i64()
            .with_context(|| format!("{key} count"))?;
        anyhow::ensure!(count == 0, "fresh database {key} must be zero, got {count}");
    }
    Ok(())
}

#[test]
fn doctor_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("doctor_output_fixture_is_produced_by_real_cli")?;
    let expected_store_names = kanban_sqlite::api::doctor_database(&temp.path)?
        .derived_stores
        .into_iter()
        .map(|store| store.store_name)
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        !expected_store_names.is_empty(),
        "fresh database must report its derived stores"
    );
    let mut output =
        kanban(&temp.path, &["--json", "--actor", "fixture", "doctor"])?.success_json()?;
    serde_json::from_value::<CliDoctorOutput>(output.clone())?;
    reject_internal_keys(&output)?;
    let data = output["data"].as_object_mut().context("doctor data")?;
    anyhow::ensure!(data["integrity_check"] == "ok");
    let migration = data["migration_version"]
        .as_i64()
        .context("migration version")?;
    let user = data["user_version"].as_i64().context("user version")?;
    anyhow::ensure!(migration > 0 && migration == user);
    data.insert("migration_version".to_owned(), json!(1));
    data.insert("user_version".to_owned(), json!(1));
    require_zero_counts(
        data,
        &[
            "expired_running_tasks",
            "running_tasks_without_active_run",
            "orphan_running_runs",
            "dependency_cycles",
            "archived_dependency_edges",
            "missing_run_logs",
            "suspicious_run_log_paths",
            "executable_dependency_violations",
            "executable_spec_violations",
            "executable_schedule_violations",
            "unplanned_active_tasks",
            "active_parents_with_incomplete_required_steps",
            "outbox_pending",
            "outbox_running",
            "outbox_failed",
            "derived_dirty_stores",
            "derived_error_stores",
            "consistency_errors",
            "consistency_warnings",
            "ontology_ledger_errors",
            "ontology_ledger_warnings",
        ],
    )?;
    let stores = data["derived_stores"]
        .as_array()
        .context("derived stores")?;
    anyhow::ensure!(
        data["consistency_issues"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "fresh database must have no consistency issues"
    );
    anyhow::ensure!(
        data["ontology_ledger_issues"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "fresh database must have no ontology ledger issues"
    );
    let names = stores
        .iter()
        .map(|store| store["store_name"].as_str().context("store name"))
        .collect::<anyhow::Result<BTreeSet<_>>>()?;
    anyhow::ensure!(names.len() == stores.len(), "duplicate derived store names");
    anyhow::ensure!(
        names
            .iter()
            .copied()
            .eq(expected_store_names.iter().map(String::as_str)),
        "CLI doctor must preserve the complete derived store identity set"
    );
    for store in stores {
        anyhow::ensure!(store["dirty"] == false, "fresh derived store must be clean");
        anyhow::ensure!(
            store["last_error"].is_null(),
            "fresh derived store must have no error"
        );
        for key in [
            "schema_version",
            "last_event_id",
            "pending_outbox",
            "running_outbox",
            "failed_outbox",
        ] {
            let value = store[key]
                .as_i64()
                .with_context(|| format!("store {key}"))?;
            anyhow::ensure!(value >= 0, "store {key} must be non-negative");
            if matches!(key, "pending_outbox" | "running_outbox" | "failed_outbox") {
                anyhow::ensure!(value == 0, "fresh store {key} must be zero");
            }
        }
    }
    data.insert(
        "derived_stores".to_owned(),
        json!([{"store_name":"fixture_store","schema_version":1,"last_event_id":0,"dirty":false,"last_error":null,"pending_outbox":0,"running_outbox":0,"failed_outbox":0}]),
    );
    assert_eq!(output, fixture("doctor")?);
    Ok(())
}

fn create_task(temp: &TempDb, title: &str) -> anyhow::Result<Value> {
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            title,
            "--description",
            "diagnostic fixture",
        ],
    )?
    .success_json()
}

#[test]
fn stats_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("stats_output_fixture_is_produced_by_real_cli")?;
    let stale = create_task(&temp, "Stale claim")?;
    let stale_ref = stale["data"]["ref"].as_str().context("stale ref")?;
    let stale_id = stale["data"]["id"].as_str().context("stale id")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "not-required",
            stale_ref,
            "--reason",
            "diagnostic fixture",
        ],
    )?
    .success()?;
    let claim = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-worker",
            "task",
            "claim",
            stale_ref,
            "--ttl-ms",
            "1",
        ],
    )?
    .success_json()?;
    let run_id = claim["data"]["run"]["id"].as_str().context("run id")?;
    let conn = kanban_sqlite::db::connect_file(&temp.path)?;
    anyhow::ensure!(
        conn.execute(
            "UPDATE tasks SET claim_expires_at=0 WHERE id=?1",
            [stale_id],
        )? == 1,
        "stale claim fixture must update exactly one task"
    );
    drop(conn);
    let blocked = create_task(&temp, "Blocked task")?;
    let blocked_ref = blocked["data"]["ref"].as_str().context("blocked ref")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "block",
            blocked_ref,
            "operator needed",
            "--force",
        ],
    )?
    .success()?;
    let mut output =
        kanban(&temp.path, &["--json", "--actor", "fixture", "stats"])?.success_json()?;
    serde_json::from_value::<CliStatsOutput>(output.clone())?;
    reject_internal_keys(&output)?;
    let data = output["data"].as_object_mut().context("stats data")?;
    anyhow::ensure!(data["generated_at"].as_i64().is_some_and(|value| value > 0));
    let statuses = data["status_counts"].as_array().context("status counts")?;
    let mut names = BTreeSet::new();
    for status in statuses {
        let name = status["status"].as_str().context("status")?;
        anyhow::ensure!(names.insert(name), "duplicate status {name}");
        anyhow::ensure!(status["count"].as_i64().is_some_and(|value| value >= 0));
    }
    anyhow::ensure!(data["blocked_reasons"][0]["reason"] == "operator needed");
    anyhow::ensure!(data["blocked_reasons"][0]["count"] == 1);
    anyhow::ensure!(data["stale_claims"][0]["task_id"] == stale_id);
    anyhow::ensure!(data["stale_claims"][0]["claim_owner"] == "fixture-worker");
    anyhow::ensure!(data["stale_claims"][0]["current_run_id"] == run_id);
    data.insert("board_id".to_owned(), json!("b_fixture"));
    data.insert("generated_at".to_owned(), json!(0));
    let stale = data["stale_claims"][0]
        .as_object_mut()
        .context("stale claim")?;
    stale.insert("task_id".to_owned(), json!("t_fixture"));
    stale.insert("claim_expires_at".to_owned(), json!(0));
    stale.insert("last_heartbeat_at".to_owned(), json!(0));
    stale.insert("current_run_id".to_owned(), json!("r_fixture"));
    assert_eq!(output, fixture("stats")?);
    Ok(())
}

#[test]
fn doctor_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliDoctorOutput>("doctor")
}

#[test]
fn stats_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliStatsOutput>("stats")
}
