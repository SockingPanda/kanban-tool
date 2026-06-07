use std::{path::Path, process::Command};

#[test]
fn serve_help_includes_default_localhost_bind_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["serve", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"), "{stdout}");
    assert!(stdout.contains("127.0.0.1"), "{stdout}");
    assert!(stdout.contains("--port"), "{stdout}");
    assert!(stdout.contains("8721"), "{stdout}");
}

#[test]
fn task_update_sets_and_clears_scheduled_at_and_due_at() {
    let temp = TempDb::new("task_update_sets_and_clears_scheduled_at_and_due_at");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli update dates",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();

    let updated = kb(
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
    )
    .success_json();
    assert_eq!(updated["data"]["scheduled_at"], 1_767_225_600_000_i64);
    assert_eq!(updated["data"]["due_at"], 1_767_312_000_000_i64);

    let cleared = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--clear-scheduled-at",
            "--clear-due-at",
        ],
    )
    .success_json();
    assert!(cleared["data"]["scheduled_at"].is_null());
    assert!(cleared["data"]["due_at"].is_null());
}

#[test]
fn task_complete_alias_finishes_like_done() {
    let temp = TempDb::new("task_complete_alias_finishes_like_done");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli complete alias",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let claim = kb(&temp.path, &["--json", "task", "claim", task_id]).success_json();
    let token = claim["data"]["claim_token"].as_str().unwrap();

    let completed = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "complete",
            task_id,
            "--claim-token",
            token,
        ],
    )
    .success_json();
    assert_eq!(completed["data"]["status"], "done");
}

#[test]
fn task_reclaim_expired_alias_matches_bare_reclaim() {
    let bare = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_bare");
    let explicit = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_explicit");

    for temp in [&bare, &explicit] {
        kb(&temp.path, &["init"]).success();
        let created = kb(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli reclaim alias",
                "--description",
                "ready spec",
            ],
        )
        .success_json();
        let task_id = created["data"]["id"].as_str().unwrap();
        kb(
            &temp.path,
            &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
        )
        .success_json();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let bare_result = kb(&bare.path, &["--json", "task", "reclaim"]).success_json();
    let explicit_result =
        kb(&explicit.path, &["--json", "task", "reclaim", "--expired"]).success_json();

    assert_eq!(bare_result, explicit_result);
    assert_eq!(explicit_result["data"]["reclaimed"], 1);
}

#[test]
fn doctor_reports_integrity_migration_and_expired_running_tasks() {
    let temp = TempDb::new("doctor_reports_integrity_migration_and_expired_running_tasks");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "doctor expired",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
    )
    .success_json();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let doctor = kb(&temp.path, &["--json", "doctor"]).success_json();

    assert_eq!(doctor["data"]["integrity_check"], "ok");
    assert_eq!(doctor["data"]["migration_version"], 1);
    assert_eq!(doctor["data"]["user_version"], 1);
    assert_eq!(doctor["data"]["expired_running_tasks"], 1);
    assert_eq!(doctor["data"]["ok"], false);
}

#[test]
fn dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing() {
    let temp =
        TempDb::new("dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing");
    kb(&temp.path, &["init"]).success();
    let backend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "backend work",
            "--description",
            "ready spec",
            "--assignee",
            "backend",
        ],
    )
    .success_json();
    let frontend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "frontend work",
            "--description",
            "ready spec",
            "--assignee",
            "frontend",
        ],
    )
    .success_json();
    let config = temp.dir.join("workers.toml");
    let logs = temp.dir.join("logs");
    std::fs::write(
        &config,
        format!(
            "[workers.backend]\ncommand = \"sh -c 'true'\"\nclaim_ttl_ms = 60000\nheartbeat_interval_ms = 10\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \"{}\"\n",
            logs.display()
        ),
    )
    .unwrap();

    let result = kb(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--worker-profile",
            "backend",
            "--profile-config",
            config.to_str().unwrap(),
            "--max-iterations",
            "1",
        ],
    )
    .success_json();

    assert_eq!(result["data"]["iterations"], 1);
    assert_eq!(result["data"]["claimed"], 1);
    let backend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            backend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    let frontend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            frontend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    assert_eq!(backend_task["data"]["status"], "done");
    assert_eq!(frontend_task["data"]["status"], "ready");
}

fn kb(db_path: &Path, args: &[&str]) -> CmdResult {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .arg("--db")
        .arg(db_path)
        .args(args)
        .output()
        .unwrap();
    CmdResult { output }
}

struct CmdResult {
    output: std::process::Output,
}

impl CmdResult {
    fn success(self) {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
    }

    fn success_json(self) -> serde_json::Value {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        serde_json::from_slice(&self.output.stdout).unwrap()
    }
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-cli-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
