mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{
    CliMaintenanceLegacyCleanupApplyOutput, CliMaintenanceLegacyCleanupInventoryOutput,
    CliMaintenanceLegacyCleanupRestoreOutput, CliMaintenanceLegacyCleanupVerifyOutput,
    CliMaintenanceRebuildOutput, CliMaintenanceRunOutput, CliMaintenanceStatusOutput,
};
use kanban_sqlite::db::connect_file;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::PathBuf;

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        format!("schemas/fixtures/cli/{operation}-output.v2.valid.json"),
    ))?)?)
}

fn cleanup_fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        format!("schemas/fixtures/cli/maintenance-cleanup-legacy-{operation}-output.v1.valid.json"),
    ))?)?)
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

struct CleanupScenario {
    temp: TempDb,
    _backup_parent: tempfile::TempDir,
    backup_dir: PathBuf,
    inventory_digest: String,
}

impl CleanupScenario {
    fn apply(&self) -> anyhow::Result<Value> {
        kanban(
            &self.temp.path,
            &[
                "--actor",
                "fixture-owner",
                "--json",
                "maintenance",
                "cleanup-legacy",
                "apply",
                "--backup-dir",
                self.backup_dir
                    .to_str()
                    .context("UTF-8 cleanup backup path")?,
                "--expected-inventory-digest",
                &self.inventory_digest,
            ],
        )?
        .success_json()
    }

    fn verify(&self) -> anyhow::Result<Value> {
        kanban(
            &self.temp.path,
            &[
                "--actor",
                "fixture-owner",
                "--json",
                "maintenance",
                "cleanup-legacy",
                "verify",
                "--backup-dir",
                self.backup_dir
                    .to_str()
                    .context("UTF-8 cleanup backup path")?,
            ],
        )?
        .success_json()
    }

    fn restore(&self) -> anyhow::Result<Value> {
        kanban(
            &self.temp.path,
            &[
                "--actor",
                "fixture-owner",
                "--json",
                "maintenance",
                "cleanup-legacy",
                "restore",
                "--backup-dir",
                self.backup_dir
                    .to_str()
                    .context("UTF-8 cleanup backup path")?,
            ],
        )?
        .success_json()
    }
}

fn setup_cleanup_scenario(name: &str) -> anyhow::Result<CleanupScenario> {
    let temp = setup(name)?;
    let legacy_file = temp.dir.join("index/v1/tasks/segment/doc");
    std::fs::create_dir_all(
        legacy_file
            .parent()
            .context("legacy fixture file must have a parent")?,
    )?;
    std::fs::write(legacy_file, b"legacy-task-index")?;

    let backup_parent = tempfile::Builder::new()
        .prefix("kb-cli-cleanup-contract-backup-")
        .tempdir()
        .context("create cleanup contract backup parent")?;
    let backup_dir = backup_parent.path().join("projection-v1-backup");
    let inventory = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "inventory",
        ],
    )?
    .success_json()?;
    let inventory_digest = inventory["data"]["inventory_digest"]
        .as_str()
        .context("cleanup inventory digest")?
        .to_owned();

    Ok(CleanupScenario {
        temp,
        _backup_parent: backup_parent,
        backup_dir,
        inventory_digest,
    })
}

fn seed_status_corpus_fixture(temp: &TempDb) -> anyhow::Result<()> {
    connect_file(&temp.path)?.execute(
        "UPDATE projection_store_state
         SET control_plane='v2',
             active_generation='gen_fixture_active',
             active_fingerprint='sha256:fixture-active',
             active_fence_epoch=3,
             active_snapshot_cursor=0,
             active_provider='fixture-lance',
             active_provider_fingerprint='fixture-provider-v2',
             active_canonical_count=0,
             active_canonical_digest='fnv64:fixture-active-canonical',
             active_delivery_count=0,
             active_delivery_digest='fnv64:fixture-active-delivery',
             active_corpus_schema='task-chunks-v2',
             active_corpus_fingerprint='corpus:fixture-active',
             active_embedding_model='fixture-embedding-v2',
             active_embedding_dimensions=384,
             previous_generation='gen_fixture_previous',
             previous_fingerprint='sha256:fixture-previous',
             previous_fence_epoch=2,
             previous_snapshot_cursor=0,
             previous_provider='fixture-lance',
             previous_provider_fingerprint='fixture-provider-v1',
             previous_canonical_count=0,
             previous_canonical_digest='fnv64:fixture-previous-canonical',
             previous_delivery_count=0,
             previous_delivery_digest='fnv64:fixture-previous-delivery',
             previous_corpus_schema='task-chunks-v2',
             previous_corpus_fingerprint='corpus:fixture-previous',
             previous_embedding_model='fixture-embedding-v1',
             previous_embedding_dimensions=256,
             building_generation='gen_fixture_building',
             building_fingerprint='sha256:fixture-building',
             building_fence_epoch=4,
             building_provider='fixture-lance',
             building_provider_fingerprint='fixture-provider-v3',
             building_canonical_count=0,
             building_canonical_digest='fnv64:fixture-building-canonical',
             building_delivery_count=0,
             building_delivery_digest='fnv64:fixture-building-delivery',
             building_phase='prepared',
             building_corpus_schema='task-chunks-v2',
             building_corpus_fingerprint='corpus:fixture-building',
             building_embedding_model='fixture-embedding-v3',
             building_embedding_dimensions=768,
             fence_epoch=4
         WHERE store_name='lancedb_chunks'",
        [],
    )?;
    Ok(())
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?)?;
    Ok(())
}

fn normalize_status(mut output: Value) -> anyhow::Result<Value> {
    let data = output["data"]
        .as_object_mut()
        .context("maintenance status data")?;
    let database_instance_id = data["database_instance_id"]
        .as_str()
        .context("database instance id")?
        .to_owned();
    data.insert("database_instance_id".to_owned(), json!("db_fixture"));
    let stores = data["stores"]
        .as_array_mut()
        .context("maintenance stores")?;
    let mut lance = stores
        .iter()
        .find(|store| store["store_name"] == "lancedb_chunks")
        .cloned()
        .context("LanceDB chunks status")?;
    anyhow::ensure!(lance["database_instance_id"] == database_instance_id);
    anyhow::ensure!(lance["active_corpus"].is_object());
    anyhow::ensure!(lance["previous_corpus"].is_object());
    anyhow::ensure!(lance["building_corpus"].is_object());
    lance["database_instance_id"] = json!("db_fixture");
    lance["lifecycle_status"] = json!("rebuilding");
    lance["runtime_availability"] = json!("available");
    lance["last_error"] = Value::Null;
    lance["fallback_reason"] = json!("generation_rebuild");
    lance["updated_at"] = json!(0);
    *stores = vec![lance];
    Ok(output)
}

fn normalize_report(mut output: Value) -> anyhow::Result<Value> {
    output["data"]["database_instance_id"] = json!("db_fixture");
    Ok(output)
}

fn normalize_run_report(mut output: Value) -> anyhow::Result<Value> {
    output["data"]["database_instance_id"] = json!("db_fixture");
    let stores = output["data"]["stores"]
        .as_array_mut()
        .context("maintenance run stores")?;
    anyhow::ensure!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_label_atoms")
    );
    anyhow::ensure!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_chunks")
    );
    stores.retain(|store| {
        store["store_name"] == "tantivy_tasks"
            || (cfg!(feature = "oxigraph-backend") && store["store_name"] == "oxigraph_relations")
    });
    Ok(output)
}

fn normalize_cleanup_report(mut output: Value) -> anyhow::Result<Value> {
    output["data"]["database_instance_id"] = json!("db_fixture");
    output["data"]["database_path"] = json!("/fixture/kb.db");
    if !output["data"]["backup_dir"].is_null() {
        output["data"]["backup_dir"] = json!("/fixture/backup");
    }
    output["data"]["inventory_digest"] = json!("sha256:fixture-inventory");
    let roots = output["data"]["roots"]
        .as_array_mut()
        .context("cleanup roots")?;
    for root in roots {
        let relative = root["relative_path"]
            .as_str()
            .context("cleanup relative path")?
            .to_owned();
        root["absolute_path"] = json!(format!("/fixture/{relative}"));
        root["digest"] = json!("sha256:fixture-root");
    }
    Ok(output)
}

fn assert_cleanup_fixture(output: Value, operation: &str) -> anyhow::Result<()> {
    decode_cleanup_fixture(output.clone(), operation)?;
    assert_eq!(
        normalize_cleanup_report(output)?,
        cleanup_fixture(operation)?
    );
    Ok(())
}

fn consume_cleanup_fixture(operation: &str) -> anyhow::Result<()> {
    let fixture = cleanup_fixture(operation)?;
    decode_cleanup_fixture(fixture.clone(), operation)?;
    Ok(())
}

fn decode_cleanup_fixture(output: Value, operation: &str) -> anyhow::Result<()> {
    let decoded = match operation {
        "inventory" => serde_json::to_value(serde_json::from_value::<
            CliMaintenanceLegacyCleanupInventoryOutput,
        >(output.clone())?)?,
        "apply" => serde_json::to_value(serde_json::from_value::<
            CliMaintenanceLegacyCleanupApplyOutput,
        >(output.clone())?)?,
        "verify" => serde_json::to_value(serde_json::from_value::<
            CliMaintenanceLegacyCleanupVerifyOutput,
        >(output.clone())?)?,
        "restore" => serde_json::to_value(serde_json::from_value::<
            CliMaintenanceLegacyCleanupRestoreOutput,
        >(output.clone())?)?,
        other => anyhow::bail!("unknown cleanup fixture operation {other}"),
    };
    anyhow::ensure!(decoded == output);
    Ok(())
}

#[test]
fn maintenance_status_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_status_contract")?;
    seed_status_corpus_fixture(&temp)?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "status",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceStatusOutput>(output.clone())?;
    assert_eq!(normalize_status(output)?, fixture("maintenance-status")?);
    Ok(())
}

#[test]
fn maintenance_run_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_run_contract")?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "run",
            "--once",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceRunOutput>(output.clone())?;
    #[cfg(feature = "oxigraph-backend")]
    let expected = {
        let mut expected = fixture("maintenance-run")?;
        expected["data"]["stores"]
            .as_array_mut()
            .expect("maintenance stores fixture")
            .push(serde_json::json!({
                "store_name": "oxigraph_relations",
                "result": {
                    "status": "succeeded",
                    "action": "generation_published",
                    "processed": 0
                },
                "lifecycle_status": "ready",
                "fallback_reason": null
            }));
        expected
    };
    #[cfg(not(feature = "oxigraph-backend"))]
    let expected = fixture("maintenance-run")?;
    assert_eq!(normalize_run_report(output)?, expected);
    Ok(())
}

#[test]
fn maintenance_rebuild_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_rebuild_contract")?;
    kanban(
        &temp.path,
        &["--actor", "fixture-owner", "maintenance", "run", "--once"],
    )?
    .success()?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceRebuildOutput>(output.clone())?;
    assert_eq!(normalize_report(output)?, fixture("maintenance-rebuild")?);
    Ok(())
}

#[test]
fn maintenance_cleanup_legacy_inventory_output_fixture_is_produced_by_real_cli()
-> anyhow::Result<()> {
    let temp = setup("maintenance_cleanup_legacy_inventory_contract")?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "inventory",
        ],
    )?
    .success_json()?;
    assert_cleanup_fixture(output, "inventory")
}

#[test]
fn maintenance_cleanup_legacy_apply_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let scenario = setup_cleanup_scenario("maintenance_cleanup_legacy_apply_contract")?;
    let output = scenario.apply()?;
    assert_cleanup_fixture(output, "apply")
}

#[test]
fn maintenance_cleanup_legacy_verify_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()>
{
    let scenario = setup_cleanup_scenario("maintenance_cleanup_legacy_verify_contract")?;
    scenario.apply()?;
    let output = scenario.verify()?;
    assert_cleanup_fixture(output, "verify")
}

#[test]
fn maintenance_cleanup_legacy_restore_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()>
{
    let scenario = setup_cleanup_scenario("maintenance_cleanup_legacy_restore_contract")?;
    scenario.apply()?;
    let output = scenario.restore()?;
    assert_cleanup_fixture(output, "restore")
}

#[test]
fn maintenance_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    let fixture = fixture("maintenance-status")?;
    let decoded = serde_json::from_value::<CliMaintenanceStatusOutput>(fixture.clone())?;
    anyhow::ensure!(decoded.data.stores[0].active_corpus.is_some());
    anyhow::ensure!(decoded.data.stores[0].previous_corpus.is_some());
    anyhow::ensure!(decoded.data.stores[0].building_corpus.is_some());
    anyhow::ensure!(serde_json::to_value(decoded)? == fixture);
    Ok(())
}

#[test]
fn maintenance_run_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliMaintenanceRunOutput>("maintenance-run")
}

#[test]
fn maintenance_rebuild_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliMaintenanceRebuildOutput>("maintenance-rebuild")
}

#[test]
fn maintenance_cleanup_legacy_inventory_output_fixture_is_consumed_by_public_contract()
-> anyhow::Result<()> {
    consume_cleanup_fixture("inventory")
}

#[test]
fn maintenance_cleanup_legacy_apply_output_fixture_is_consumed_by_public_contract()
-> anyhow::Result<()> {
    consume_cleanup_fixture("apply")
}

#[test]
fn maintenance_cleanup_legacy_verify_output_fixture_is_consumed_by_public_contract()
-> anyhow::Result<()> {
    consume_cleanup_fixture("verify")
}

#[test]
fn maintenance_cleanup_legacy_restore_output_fixture_is_consumed_by_public_contract()
-> anyhow::Result<()> {
    consume_cleanup_fixture("restore")
}

#[test]
fn maintenance_contracts_reject_unknown_and_missing_nullable_fields() -> anyhow::Result<()> {
    let mut status = fixture("maintenance-status")?;
    status["data"]["stores"][0]
        .as_object_mut()
        .context("store object")?
        .remove("fallback_reason");
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceStatusOutput>(status).is_err());

    let mut run = fixture("maintenance-run")?;
    run["data"]["stores"][0]["unexpected"] = json!(true);
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceRunOutput>(run).is_err());

    let mut failed = fixture("maintenance-run")?;
    failed["data"]["stores"][0]["result"] = json!({
        "status": "failed",
        "kind": "backend",
        "message": "fixture backend failure"
    });
    serde_json::from_value::<CliMaintenanceRunOutput>(failed.clone())?;
    failed["data"]["stores"][0]["result"]["kind"] = json!("unknown");
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceRunOutput>(failed).is_err());

    let mut nested_unknown = fixture("maintenance-run")?;
    nested_unknown["data"]["stores"][0]["result"]["unexpected"] = json!(true);
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceRunOutput>(nested_unknown).is_err());
    Ok(())
}
