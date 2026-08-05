use std::process::Command;

use kanban_contract::{
    CliBoardCurrentOutput, CliBoardUseOutput, CliConfigShowOutput, CliConfigSource, CliInitOutput,
};

fn kanban() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kanban"))
}

fn isolated(command: &mut Command, temp: &tempfile::TempDir) -> &mut Command {
    command
        .current_dir(temp.path())
        .env_remove("KANBAN_DB")
        .env_remove("KB_DB")
        .env_remove("KB_BOARD")
        .env_remove("KANBAN_LOCALE")
        .env("XDG_CONFIG_HOME", temp.path().join("xdg-config"))
        .env("XDG_DATA_HOME", temp.path().join("xdg-data"))
}

#[test]
fn board_use_output_fixture_is_produced_by_real_cli() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = isolated(&mut kanban(), &temp)
        .args(["--json", "board", "use", "fixture"])
        .output()
        .expect("run kanban board use");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: CliBoardUseOutput =
        serde_json::from_slice(&output.stdout).expect("board use contract output");
    assert_eq!(parsed.data.board, "fixture");
    assert!(parsed.data.created);
    assert!(parsed.data.updated);
    assert!(temp.path().join(".kb/config.toml").is_file());
}

#[test]
fn board_use_output_fixture_is_consumed_by_contract_root() {
    let fixture = include_str!("../../../schemas/fixtures/cli/board-use-output.v1.valid.json");
    let parsed: CliBoardUseOutput = serde_json::from_str(fixture).expect("board use fixture");
    assert_eq!(parsed.data.board, "fixture");
}

#[test]
fn board_current_output_fixture_is_produced_by_real_cli() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join(".kb")).expect("config directory");
    std::fs::write(temp.path().join(".kb/config.toml"), "board = \"fixture\"\n").expect("config");
    let output = isolated(&mut kanban(), &temp)
        .args(["--json", "board", "current"])
        .output()
        .expect("run kanban board current");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: CliBoardCurrentOutput =
        serde_json::from_slice(&output.stdout).expect("board current contract output");
    assert_eq!(parsed.data.board, "fixture");
    assert!(!parsed.data.created);
    assert!(!parsed.data.updated);
}

#[test]
fn board_current_output_fixture_is_consumed_by_contract_root() {
    let fixture = include_str!("../../../schemas/fixtures/cli/board-current-output.v1.valid.json");
    let parsed: CliBoardCurrentOutput =
        serde_json::from_str(fixture).expect("board current fixture");
    assert_eq!(parsed.data.board, "fixture");
}

#[test]
fn init_is_config_only_and_idempotent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let first = isolated(&mut kanban(), &temp)
        .args(["--json", "init"])
        .output()
        .expect("run init");
    assert!(
        first.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    let parsed: CliInitOutput = serde_json::from_slice(&first.stdout).expect("init contract");
    assert_eq!(parsed.data.board_slug, "default");
    assert_eq!(parsed.data.board_id, "not_initialized");
    assert_eq!(parsed.data.created, Some(true));
    assert!(!temp.path().join("xdg-data").exists());
    assert!(temp.path().join(".kb/config.toml").is_file());

    let second = isolated(&mut kanban(), &temp)
        .args(["--json", "init"])
        .output()
        .expect("repeat init");
    assert!(
        second.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let parsed: CliInitOutput = serde_json::from_slice(&second.stdout).expect("init contract");
    assert_eq!(parsed.data.created, Some(false));
}

#[test]
fn config_show_resolves_project_relative_db_without_opening_it() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join(".kb")).expect("config directory");
    std::fs::write(
        temp.path().join(".kb/config.toml"),
        "db = \"relative.db\"\nboard = \"fixture\"\n",
    )
    .expect("config");
    let output = isolated(&mut kanban(), &temp)
        .args(["--json", "config", "show"])
        .output()
        .expect("run config show");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: CliConfigShowOutput =
        serde_json::from_slice(&output.stdout).expect("config contract");
    assert_eq!(parsed.data.board.value, "fixture");
    assert_eq!(
        parsed.data.db.value,
        temp.path().join(".kb/relative.db").display().to_string()
    );
    assert!(matches!(
        parsed.data.db.source,
        CliConfigSource::ProjectConfig { .. }
    ));
    assert!(!temp.path().join(".kb/relative.db").exists());
}

#[test]
fn completions_are_static_and_do_not_create_project_or_database_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let db = temp.path().join("missing/kanban.db");
    let output = isolated(&mut kanban(), &temp)
        .args(["--db"])
        .arg(&db)
        .args(["--board", "not-a-domain-board", "completions", "bash"])
        .output()
        .expect("run completions");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("__complete"));
    assert!(!db.exists());
    assert!(!temp.path().join(".kb").exists());
}
