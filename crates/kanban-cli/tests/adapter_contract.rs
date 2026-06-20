mod common;

use anyhow::Context;
use common::{TempDb, kanban};

#[test]
fn cli_adapter_contract_commits_to_shared_task_label_and_run_state() -> anyhow::Result<()> {
    let temp = TempDb::new("adapter_contract")?;
    kanban_sqlite::init_database(&temp.path, "tester")?;

    let label_json =
        kanban(&temp.path, &["label", "create", "cli-contract", "--json"])?.success_json()?;
    let label_id = label_json["data"]["id"]
        .as_str()
        .context("label id")?
        .to_owned();

    let task_json = kanban(
        &temp.path,
        &[
            "task",
            "create",
            "CLI adapter contract",
            "--description",
            "ready from CLI",
            "--actor",
            "cli-adapter",
            "--json",
        ],
    )?
    .success_json()?;
    let task_id = task_json["data"]["id"]
        .as_str()
        .context("task id")?
        .to_owned();
    assert_eq!(
        kanban_sqlite::get_task(&temp.path, "default", &task_id)?.status,
        kanban_core::TaskStatus::Ready
    );

    kanban(
        &temp.path,
        &[
            "label",
            "add",
            &task_id,
            "cli-contract",
            "--actor",
            "cli-adapter",
            "--json",
        ],
    )?
    .success()?;
    let labeled = kanban_sqlite::get_task(&temp.path, "default", &task_id)?;
    assert_eq!(labeled.labels.len(), 1);
    assert_eq!(labeled.labels[0].id, label_id);
    assert_eq!(labeled.labels[0].name, "cli-contract");

    let claim_json = kanban(
        &temp.path,
        &["task", "start", &task_id, "--actor", "cli-worker", "--json"],
    )?
    .success_json()?;
    let claim_token = claim_json["data"]["claim_token"]
        .as_str()
        .context("claim token")?
        .to_owned();
    assert_eq!(
        kanban_sqlite::get_task(&temp.path, "default", &task_id)?.status,
        kanban_core::TaskStatus::Running
    );

    kanban(
        &temp.path,
        &[
            "task",
            "complete",
            &task_id,
            "--claim-token",
            &claim_token,
            "--actor",
            "cli-worker",
            "--json",
        ],
    )?
    .success()?;
    let completed = kanban_sqlite::get_task(&temp.path, "default", &task_id)?;
    assert_eq!(completed.status, kanban_core::TaskStatus::Done);
    assert!(completed.claim_token.is_none());
    assert_eq!(completed.labels.len(), 1);
    let runs = kanban_sqlite::list_runs(&temp.path, "default", Some(&task_id))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "succeeded");
    assert_eq!(runs[0].claim_owner, "cli-worker");

    Ok(())
}
