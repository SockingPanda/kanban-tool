mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir_str_envs};
use kanban_contract::cli_operator::{
    CliDispatchOutput, CliDispatchResult, CliExportOutput, CliHookCodexInstallOutput,
    CliHookCodexStatusOutput, CliHookCodexUninstallOutput, CliImportOutput, CliSignalConfirmOutput,
    CliSignalListOutput, CliSignalRecordOutput, CliSignalRejectOutput, CliSignalResolveOutput,
    CliSignalReviewOutput, CliSignalShowOutput, CliSignalSupersedeOutput,
};
use kanban_sqlite::api::lifecycle::{begin_database_replace, publish_staged_database};
use kanban_sqlite::init::init_database;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str, validity: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let version = if operation == "import" { "v2" } else { "v1" };
    let path = root.join(format!(
        "schemas/fixtures/cli/{operation}-output.{version}.{validity}.json"
    ));
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<T> {
    Ok(serde_json::from_value(fixture(operation, "valid")?)?)
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn record_signal(temp: &TempDb, title: &str) -> anyhow::Result<Value> {
    let input = temp.dir.join(format!("{title}.json"));
    std::fs::write(
        &input,
        serde_json::to_vec_pretty(&json!({
            "kind": "operator_feedback",
            "title": title,
            "summary": "Stable operator signal summary",
            "severity": "medium",
            "actor": "fixture",
            "agent_type": "codex",
            "dedupe_key": format!("fixture-{title}"),
            "source": "contract-test",
            "evidence": {"case": "operator"}
        }))?,
    )?;
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "signal",
            "record",
            "--input",
            input.to_str().context("UTF-8 signal input")?,
        ],
    )?
    .success_json()
}

fn normalize_signal(
    signal: &mut Value,
    status: &str,
    reason: Option<&str>,
    superseded: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        signal["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("sig_"))
    );
    anyhow::ensure!(
        signal["observation_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("obs_"))
    );
    anyhow::ensure!(signal["status"] == status);
    signal["id"] = json!("sig_FIXTURE");
    signal["board_id"] = json!("b_BOARD");
    signal["observation_id"] = json!("obs_FIXTURE");
    signal["dedupe_key"] = json!("fixture-signal");
    signal["superseded_by_signal_id"] = if superseded {
        json!("sig_REPLACEMENT")
    } else {
        Value::Null
    };
    signal["reviewed_by"] = reason.map_or(Value::Null, |_| json!("fixture"));
    signal["reviewed_at"] = reason.map_or(Value::Null, |_| json!(200));
    signal["review_reason"] = reason.map_or(Value::Null, |value| json!(value));
    signal["created_at"] = json!(100);
    signal["updated_at"] = if reason.is_some() {
        json!(200)
    } else {
        json!(100)
    };
    let observation = signal
        .get_mut("observation")
        .context("signal observation")?;
    observation["id"] = json!("obs_FIXTURE");
    observation["board_id"] = json!("b_BOARD");
    observation["created_at"] = json!(100);
    Ok(())
}

fn normalized_record_output(mut output: Value) -> anyhow::Result<Value> {
    normalize_signal(&mut output["data"]["signal"], "open", None, false)?;
    output["data"]["signal"]["title"] = json!("signal");
    output["data"]["signal"]["dedupe_key"] = json!("fixture-signal");
    Ok(output)
}

fn normalize_signal_list(
    output: &mut Value,
    status: &str,
    reason: Option<&str>,
    superseded: bool,
) -> anyhow::Result<()> {
    let data = output["data"]
        .as_array_mut()
        .context("signal output data")?;
    anyhow::ensure!(data.len() == 1, "expected one signal, got {}", data.len());
    normalize_signal(&mut data[0], status, reason, superseded)?;
    data[0]["title"] = json!("signal");
    data[0]["dedupe_key"] = json!("fixture-signal");
    Ok(())
}

fn signal_id(output: &Value) -> anyhow::Result<&str> {
    output["data"]["signal"]["id"]
        .as_str()
        .context("recorded signal id")
}

#[test]
fn producer_signal_record_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_record")?;
    let actual = record_signal(&temp, "signal")?;
    serde_json::from_value::<CliSignalRecordOutput>(actual.clone())?;
    assert_eq!(
        normalized_record_output(actual)?,
        fixture("signal-record", "valid")?
    );
    Ok(())
}

#[test]
fn producer_signal_list_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_list")?;
    record_signal(&temp, "signal")?;
    let mut actual = kanban(&temp.path, &["--json", "signal", "list"])?.success_json()?;
    serde_json::from_value::<CliSignalListOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "open", None, false)?;
    assert_eq!(actual, fixture("signal-list", "valid")?);
    Ok(())
}

#[test]
fn producer_signal_show_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_show")?;
    let recorded = record_signal(&temp, "signal")?;
    let mut actual = kanban(
        &temp.path,
        &["--json", "signal", "show", signal_id(&recorded)?],
    )?
    .success_json()?;
    serde_json::from_value::<CliSignalShowOutput>(actual.clone())?;
    normalize_signal(&mut actual["data"], "open", None, false)?;
    actual["data"]["title"] = json!("signal");
    actual["data"]["dedupe_key"] = json!("fixture-signal");
    assert_eq!(actual, fixture("signal-show", "valid")?);
    Ok(())
}

#[test]
fn producer_signal_review_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_review")?;
    record_signal(&temp, "signal")?;
    let mut actual = kanban(&temp.path, &["--json", "signal", "review"])?.success_json()?;
    serde_json::from_value::<CliSignalReviewOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "open", None, false)?;
    assert_eq!(actual, fixture("signal-review", "valid")?);
    Ok(())
}

fn lifecycle_output(temp: &TempDb, operation: &str, extra: &[&str]) -> anyhow::Result<Value> {
    let recorded = record_signal(temp, "signal")?;
    let mut args = vec![
        "--json",
        "--actor",
        "fixture",
        "signal",
        operation,
        signal_id(&recorded)?,
    ];
    args.extend_from_slice(extra);
    kanban(&temp.path, &args)?.success_json()
}

#[test]
fn producer_signal_confirm_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_confirm")?;
    let mut actual = lifecycle_output(&temp, "confirm", &["--reason", "confirmed"])?;
    serde_json::from_value::<CliSignalConfirmOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "confirmed", Some("confirmed"), false)?;
    assert_eq!(actual, fixture("signal-confirm", "valid")?);
    Ok(())
}

#[test]
fn producer_signal_reject_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_reject")?;
    let mut actual = lifecycle_output(&temp, "reject", &["--reason", "rejected"])?;
    serde_json::from_value::<CliSignalRejectOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "rejected", Some("rejected"), false)?;
    assert_eq!(actual, fixture("signal-reject", "valid")?);
    Ok(())
}

#[test]
fn producer_signal_resolve_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_resolve")?;
    let mut actual = lifecycle_output(&temp, "resolve", &["--reason", "resolved"])?;
    serde_json::from_value::<CliSignalResolveOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "resolved", Some("resolved"), false)?;
    assert_eq!(actual, fixture("signal-resolve", "valid")?);
    Ok(())
}

#[test]
fn producer_signal_supersede_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_signal_supersede")?;
    let source = record_signal(&temp, "signal")?;
    let replacement = record_signal(&temp, "replacement")?;
    let mut actual = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "signal",
            "supersede",
            signal_id(&source)?,
            "--by",
            signal_id(&replacement)?,
            "--reason",
            "superseded",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliSignalSupersedeOutput>(actual.clone())?;
    normalize_signal_list(&mut actual, "superseded", Some("superseded"), true)?;
    assert_eq!(actual, fixture("signal-supersede", "valid")?);
    Ok(())
}

fn hook_env(temp: &TempDb) -> anyhow::Result<(String, String)> {
    let codex_home = temp.dir.join("codex-home");
    std::fs::create_dir_all(&codex_home)?;
    Ok((
        codex_home.to_string_lossy().into_owned(),
        temp.dir
            .join(".xdg-config/kanban/codex-hooks.json")
            .to_string_lossy()
            .into_owned(),
    ))
}

fn normalize_hook_paths(output: &mut Value) {
    output["data"]["path"] = json!("/fixture/hooks.json");
    if output["data"].get("prompt_config").is_some() {
        output["data"]["prompt_config"]["path"] = json!("/fixture/codex-hooks.json");
    }
}

fn install_hook(temp: &TempDb, codex_home: &str) -> anyhow::Result<Value> {
    kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "install"],
        &temp.dir,
        &[("CODEX_HOME", codex_home)],
    )?
    .success_json()
}

#[test]
fn producer_hook_codex_install_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = TempDb::new("operator_hook_install")?;
    let (codex_home, _) = hook_env(&temp)?;
    let mut actual = install_hook(&temp, &codex_home)?;
    serde_json::from_value::<CliHookCodexInstallOutput>(actual.clone())?;
    normalize_hook_paths(&mut actual);
    assert_eq!(actual, fixture("hook-codex-install", "valid")?);
    Ok(())
}

#[test]
fn producer_hook_codex_status_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = TempDb::new("operator_hook_status")?;
    let (codex_home, _) = hook_env(&temp)?;
    install_hook(&temp, &codex_home)?;
    let mut actual = kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "status"],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    serde_json::from_value::<CliHookCodexStatusOutput>(actual.clone())?;
    normalize_hook_paths(&mut actual);
    assert_eq!(actual, fixture("hook-codex-status", "valid")?);
    Ok(())
}

#[test]
fn producer_hook_codex_uninstall_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = TempDb::new("operator_hook_uninstall")?;
    let (codex_home, _) = hook_env(&temp)?;
    install_hook(&temp, &codex_home)?;
    let mut actual = kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "uninstall"],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    serde_json::from_value::<CliHookCodexUninstallOutput>(actual.clone())?;
    normalize_hook_paths(&mut actual);
    assert_eq!(actual, fixture("hook-codex-uninstall", "valid")?);
    Ok(())
}

#[test]
fn producer_dispatch_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_dispatch")?;
    let once = kanban(&temp.path, &["--json", "dispatch", "--once"])?.success_json()?;
    let once_typed: CliDispatchOutput = serde_json::from_value(once)?;
    assert!(matches!(once_typed.data, CliDispatchResult::Once(_)));

    let actual = kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--max-iterations",
            "1",
            "--poll-interval-ms",
            "1",
        ],
    )?
    .success_json()?;
    let typed: CliDispatchOutput = serde_json::from_value(actual.clone())?;
    assert!(matches!(typed.data, CliDispatchResult::Loop(_)));
    assert_eq!(actual, fixture("dispatch", "valid")?);
    Ok(())
}

#[test]
fn producer_export_matches_exact_fixture() -> anyhow::Result<()> {
    let temp = setup("operator_export")?;
    let out = temp.dir.join("portable.jsonl");
    let mut actual = kanban(
        &temp.path,
        &[
            "--json",
            "export",
            "--format",
            "jsonl",
            "--out",
            out.to_str().context("UTF-8 export path")?,
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliExportOutput>(actual.clone())?;
    anyhow::ensure!(out.is_file());
    anyhow::ensure!(actual["data"]["records"].as_u64().unwrap_or_default() > 0);
    actual["data"]["out_path"] = json!("/fixture/portable.jsonl");
    actual["data"]["records"] = json!(10);
    assert_eq!(actual, fixture("export", "valid")?);
    Ok(())
}

#[test]
fn import_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let target = TempDb::new("operator_import_resume_target")?;
    kanban(&target.path, &["init"])?.success()?;
    let staged = target.dir.join(".kb.db.restore.fixture");
    let previous = target.dir.join(".kb.db.replaced.fixture");
    let journal = target.dir.join(".kb.db.replace.journal");
    init_database(&staged, "cli-contract-resume")?;
    let mut guard = begin_database_replace(&target.path)?;
    publish_staged_database(&mut guard, &target.path, &staged, &previous, &journal)?;
    drop(guard);
    // Recreate the durable previous-published crash state. The next CLI
    // invocation must resume before it attempts to inspect its input path.
    std::fs::hard_link(&target.path, &staged)?;
    std::fs::remove_file(&target.path)?;
    let mut journal_json: Value = serde_json::from_slice(&std::fs::read(&journal)?)?;
    journal_json["phase"] = json!("previous_published");
    std::fs::write(&journal, serde_json::to_vec_pretty(&journal_json)?)?;
    let ignored_input = target.dir.join("ignored-after-resume.jsonl");
    let mut actual = kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            ignored_input.to_str().context("UTF-8 import path")?,
            "--replace",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliImportOutput>(actual.clone())?;
    assert_eq!(actual["data"]["resumed"], true);
    assert_eq!(actual["data"]["records"], 0);
    assert_eq!(actual["data"]["dry_run"], false);
    actual["data"]["input_path"] = json!("/fixture/ignored-after-resume.jsonl");
    assert_eq!(actual, fixture("import", "valid")?);
    Ok(())
}

macro_rules! consumer_test {
    ($name:ident, $operation:literal, $ty:ty) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            let _: $ty = consume($operation)?;
            assert!(
                serde_json::from_value::<$ty>(fixture($operation, "invalid")?).is_err(),
                "{} invalid fixture must be rejected",
                $operation
            );
            Ok(())
        }
    };
}

consumer_test!(
    signal_confirm_output_fixture_is_consumed_by_public_contract,
    "signal-confirm",
    CliSignalConfirmOutput
);
consumer_test!(
    signal_list_output_fixture_is_consumed_by_public_contract,
    "signal-list",
    CliSignalListOutput
);
consumer_test!(
    signal_record_output_fixture_is_consumed_by_public_contract,
    "signal-record",
    CliSignalRecordOutput
);
consumer_test!(
    signal_reject_output_fixture_is_consumed_by_public_contract,
    "signal-reject",
    CliSignalRejectOutput
);
consumer_test!(
    signal_resolve_output_fixture_is_consumed_by_public_contract,
    "signal-resolve",
    CliSignalResolveOutput
);
consumer_test!(
    signal_review_output_fixture_is_consumed_by_public_contract,
    "signal-review",
    CliSignalReviewOutput
);
consumer_test!(
    signal_show_output_fixture_is_consumed_by_public_contract,
    "signal-show",
    CliSignalShowOutput
);
consumer_test!(
    signal_supersede_output_fixture_is_consumed_by_public_contract,
    "signal-supersede",
    CliSignalSupersedeOutput
);
consumer_test!(
    hook_codex_install_output_fixture_is_consumed_by_public_contract,
    "hook-codex-install",
    CliHookCodexInstallOutput
);
consumer_test!(
    hook_codex_status_output_fixture_is_consumed_by_public_contract,
    "hook-codex-status",
    CliHookCodexStatusOutput
);
consumer_test!(
    hook_codex_uninstall_output_fixture_is_consumed_by_public_contract,
    "hook-codex-uninstall",
    CliHookCodexUninstallOutput
);
consumer_test!(
    dispatch_output_fixture_is_consumed_by_public_contract,
    "dispatch",
    CliDispatchOutput
);
consumer_test!(
    export_output_fixture_is_consumed_by_public_contract,
    "export",
    CliExportOutput
);
consumer_test!(
    import_output_fixture_is_consumed_by_public_contract,
    "import",
    CliImportOutput
);

fn operator_output_ownership_violations() -> Vec<&'static str> {
    let signal = include_str!("../src/commands/signal.rs");
    let hook = include_str!("../src/commands/hook.rs");
    let dispatch = include_str!("../src/commands/app.rs");
    let maintenance = include_str!("../src/commands/maintenance.rs");
    let mut violations = Vec::new();
    for owner in [
        "CliSignalRecordOutput::new",
        "CliSignalListOutput::new",
        "CliSignalShowOutput::new",
        "CliSignalReviewOutput::new",
        "CliSignalConfirmOutput::new",
        "CliSignalRejectOutput::new",
        "CliSignalResolveOutput::new",
        "CliSignalSupersedeOutput::new",
    ] {
        if !signal.contains(owner) {
            violations.push("signal handler is missing a contract-owned output");
        }
    }
    for owner in [
        "CliHookCodexInstallOutput::new",
        "CliHookCodexStatusOutput::new",
        "CliHookCodexUninstallOutput::new",
    ] {
        if !hook.contains(owner) {
            violations.push("hook handler is missing a contract-owned output");
        }
    }
    if !dispatch.contains("CliDispatchOutput::new") {
        violations.push("dispatch handler is missing its contract-owned output");
    }
    if !maintenance.contains("CliExportOutput::new")
        || !maintenance.contains("CliImportOutput::new")
    {
        violations.push("portable maintenance handlers are missing contract-owned outputs");
    }
    violations
}

#[test]
fn operator_handlers_have_fail_closed_contract_output_ownership() {
    assert!(
        operator_output_ownership_violations().is_empty(),
        "operator output ownership violations: {:#?}",
        operator_output_ownership_violations()
    );
}
