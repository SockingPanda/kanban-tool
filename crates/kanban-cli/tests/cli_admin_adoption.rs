//! 真实 CLI host-admin、dispatcher、Codex hook 和 completion 见证。

mod knowledge_support;

use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use kanban_protocol::{
    BackupResponse, CheckpointResponse, CliDoctorOutput, CliStatsOutput, CliTaskCreateOutput,
    ExportResponse, ImportResponse, MaintenanceRunResponse, MaintenanceStatusResponse,
    VacuumResponse,
};

use knowledge_support::Host;

#[test]
fn maintenance_admin_commands_use_real_host_and_typed_json() {
    let host = Host::new();
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "maintenance adoption task",
        "--task-id",
        "t_cli_maintenance_adoption",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.id, "t_cli_maintenance_adoption");

    let doctor: CliDoctorOutput = host.json(&["doctor"]);
    assert_eq!(doctor.data.integrity_check, "ok");
    assert_eq!(doctor.data.consistency_errors, 0);
    assert!(!doctor.data.derived_stores.is_empty());
    assert!(
        doctor
            .data
            .derived_stores
            .iter()
            .all(|store| !store.store_name.is_empty())
    );

    let stats: CliStatsOutput = host.json(&["stats"]);
    assert_eq!(stats.data.board_id, "b_default");

    let checkpoint: CheckpointResponse = host.json(&["checkpoint"]);
    assert!((0..=1).contains(&checkpoint.data.busy));
    assert!(checkpoint.data.checkpointed_frames >= 0);

    let backup_path = host.temp_path("adoption-backup.sqlite");
    let backup_path_text = backup_path.to_str().expect("backup path is UTF-8");
    let backup: BackupResponse = host.json(&["backup", "--path", backup_path_text]);
    assert_eq!(backup.data.out_path, backup_path_text);
    assert!(backup.data.bytes > 0);
    assert!(backup_path.is_file());
    assert!(backup.data.checksum_sha256.starts_with("sha256:"));

    let export_path = host.temp_path("adoption-export.jsonl");
    let export_path_text = export_path.to_str().expect("export path is UTF-8");
    let exported: ExportResponse = host.json(&["export", "--path", export_path_text]);
    assert_eq!(exported.data.out_path, export_path_text);
    assert!(exported.data.record_count >= 1);
    assert!(export_path.is_file());
    assert!(exported.data.checksum_sha256.starts_with("sha256:"));

    // 通过命令路径导入同一 host 的导出；操作幂等，动态指纹由 typed response 承载，避免静态 Value 比较。
    let imported: ImportResponse = host.json(&["import", "--path", export_path_text, "--replace"]);
    assert_eq!(imported.data.in_path, export_path_text);
    assert_eq!(imported.data.phase, "completed");
    assert!(!imported.data.journal_id.is_empty());

    let vacuum: VacuumResponse = host.json(&["vacuum"]);
    assert!(vacuum.data.ok);
    assert!(vacuum.data.after_bytes > 0);
    assert!(
        !vacuum.data.source_fingerprint.is_empty(),
        "vacuum source fingerprint: {:?}",
        vacuum.data.source_fingerprint
    );

    let status: MaintenanceStatusResponse = host.json(&["maintenance", "status"]);
    assert_eq!(status.data.protocol_version, 2);
    assert!(!status.data.database_instance_id.is_empty());
    let run: MaintenanceRunResponse = host.json(&["maintenance", "run", "--owner", "cli-adoption"]);
    assert_eq!(run.data.owner, "cli-adoption");
    assert_eq!(run.data.mode, "rebuild");
    assert_eq!(run.data.action, "run");
    assert!(run.data.degraded || !run.data.errors.is_empty());
    let rebuild: MaintenanceRunResponse =
        host.json(&["maintenance", "rebuild", "--owner", "cli-adoption"]);
    assert_eq!(rebuild.data.action, "rebuild");
    let cleanup: MaintenanceRunResponse =
        host.json(&["maintenance", "cleanup", "--owner", "cli-adoption"]);
    assert_eq!(cleanup.data.action, "cleanup");
}

#[test]
fn dispatcher_profile_is_consumed_by_real_serve_and_only_claims_ready() {
    let profile = r#"
board = "default"
command = "true"
poll_interval_ms = 20
claim_ttl_ms = 10000
heartbeat_interval_ms = 1000
on_success = "done"
on_failure = "blocked"
log_dir = "runs"
"#;
    let host = Host::with_dispatcher(profile);
    let task: CliTaskCreateOutput = host.json(&[
        "task",
        "create",
        "dispatcher adoption task",
        "--task-id",
        "t_cli_dispatcher_adoption",
        "--description",
        "dispatcher command adoption",
        "--status",
        "todo",
    ]);
    assert_eq!(task.data.status, kanban_protocol::ApiTaskStatus::Todo);
    let task_ref = task.data.task_ref.clone();
    let _: kanban_protocol::MarkExecutionPlanNotRequiredResponse = host.json(&[
        "task",
        "step",
        "not-required",
        task_ref.as_str(),
        "--reason",
        "dispatcher adoption test",
    ]);
    let ready: kanban_protocol::CliTaskPromoteOutput =
        host.json(&["task", "promote", task_ref.as_str()]);
    assert_eq!(ready.data.status, kanban_protocol::ApiTaskStatus::Ready);

    let mut observed = None;
    for _ in 0..100 {
        let shown: kanban_protocol::CliTaskShowOutput =
            host.json(&["task", "show", "t_cli_dispatcher_adoption"]);
        if shown.data.status == kanban_protocol::ApiTaskStatus::Done {
            observed = Some(shown.data);
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let done = observed.expect("dispatcher should claim and complete ready task");
    assert_eq!(done.id, "t_cli_dispatcher_adoption");
    assert_eq!(done.status, kanban_protocol::ApiTaskStatus::Done);
    assert!(done.current_run_id.is_some());
}

#[test]
fn codex_hooks_install_handle_status_and_uninstall_use_real_binary() {
    let temp = tempfile::tempdir().expect("hook tempdir");
    let codex_home = temp.path().join("codex-home");
    let xdg_config = temp.path().join("xdg-config");
    fs::create_dir_all(&codex_home).expect("codex home");

    let mut install = Command::new(env!("CARGO_BIN_EXE_kanban"));
    let install = install
        .current_dir(temp.path())
        .env("CODEX_HOME", &codex_home)
        .env("XDG_CONFIG_HOME", &xdg_config)
        .args(["--json", "hook", "codex", "install", "--record-signals"])
        .output()
        .expect("run codex hook install");
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
    let installed: kanban_protocol::cli_operator::CliHookCodexInstallOutput =
        serde_json::from_slice(&install.stdout).expect("typed hook install output");
    assert!(installed.data.installed);
    assert_eq!(installed.data.managed_hook_count, 2);
    assert!(installed.data.prompt_config.valid);
    assert!(codex_home.join("hooks.json").is_file());

    let mut status = Command::new(env!("CARGO_BIN_EXE_kanban"));
    let status = status
        .current_dir(temp.path())
        .env("CODEX_HOME", &codex_home)
        .env("XDG_CONFIG_HOME", &xdg_config)
        .args(["--json", "hook", "codex", "status"])
        .output()
        .expect("run codex hook status");
    assert!(status.status.success());
    let status: kanban_protocol::cli_operator::CliHookCodexStatusOutput =
        serde_json::from_slice(&status.stdout).expect("typed hook status output");
    assert!(status.data.installed);
    assert_eq!(status.data.managed_hook_count, 2);

    let failure_payload = r#"{
        "hook_event_name":"PostToolUse",
        "tool_name":"Bash",
        "tool_input":{"command":"kanban task show default#1"},
        "tool_response":{"exit_code":1,"stderr":"not found","success":false}
    }"#;
    let mut failure = Command::new(env!("CARGO_BIN_EXE_kanban"));
    let mut failure = failure
        .current_dir(temp.path())
        .env("CODEX_HOME", &codex_home)
        .env("XDG_CONFIG_HOME", &xdg_config)
        .args(["hook", "codex", "handle", "failure"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start codex failure hook");
    failure
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(failure_payload.as_bytes())
        .expect("write hook payload");
    let failure = failure.wait_with_output().expect("collect hook response");
    assert!(failure.status.success());
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct HookResponse {
        system_message: String,
    }
    let response: HookResponse = serde_json::from_slice(&failure.stdout).expect("hook JSON");
    assert!(response.system_message.contains("kanban CLI 命令失败"));
    assert!(response.system_message.contains("default#1"));

    let mut uninstall = Command::new(env!("CARGO_BIN_EXE_kanban"));
    let uninstall = uninstall
        .current_dir(temp.path())
        .env("CODEX_HOME", &codex_home)
        .env("XDG_CONFIG_HOME", &xdg_config)
        .args(["--json", "hook", "codex", "uninstall"])
        .output()
        .expect("run codex hook uninstall");
    assert!(uninstall.status.success());
    let removed: kanban_protocol::cli_operator::CliHookCodexUninstallOutput =
        serde_json::from_slice(&uninstall.stdout).expect("typed hook uninstall output");
    assert_eq!(removed.data.removed_hook_count, 2);
    assert!(!removed.data.installed);
}

#[test]
fn completion_and_hidden_complete_are_local_and_do_not_open_database() {
    let temp = tempfile::tempdir().expect("completion tempdir");
    let missing_db = temp.path().join("not-created").join("kanban.db");
    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(temp.path())
        .args([
            "--json",
            "--db",
            missing_db.to_str().expect("db path is UTF-8"),
            "__complete",
            "status",
            "r",
        ])
        .output()
        .expect("run hidden completion");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ready\nrunning\nreview\n"
    );
    assert!(!missing_db.exists());
    assert!(!temp.path().join(".kb").exists());

    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(temp.path())
        .args(["completions", "bash"])
        .output()
        .expect("run bash completion");
    assert!(output.status.success());
    let script = String::from_utf8_lossy(&output.stdout);
    assert!(script.contains("_kanban_dynamic_completions"));
    assert!(script.contains("__complete"));
    assert!(!missing_db.exists());
}
