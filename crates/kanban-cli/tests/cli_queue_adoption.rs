mod support;

use std::{
    net::{Ipv4Addr, TcpListener},
    process::Command,
};

use serde_json::Value;

use support::{TestHost, assert_contract, assert_fixture_shape};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_cli_uses_real_host_for_config_board_and_task_commands() {
    let host = TestHost::start().await;

    let init = host.json(&["--json", "init"]);
    assert_contract("init", "init");
    assert_fixture_shape(&init, "init");
    assert_eq!(init["data"]["board_slug"], "default");
    assert!(host.project_path().join(".kb/config.toml").is_file());

    std::fs::write(
        host.project_path().join(".kb/config.toml"),
        "db = \"project.db\"\nboard = \"project-board\"\n",
    )
    .expect("写入 project config");
    let config = host.json(&["--json", "--locale", "en", "config", "show"]);
    assert_contract("config show", "config-show");
    assert_fixture_shape(&config, "config-show");
    assert_eq!(config["data"]["board"]["value"], "project-board");
    assert_eq!(config["data"]["locale"]["value"], "en");

    let board_use = host.json(&["--json", "board", "use", "default"]);
    assert_contract("board use", "board-use");
    assert_fixture_shape(&board_use, "board-use");
    assert_eq!(board_use["data"]["board"], "default");
    let board_current = host.json(&["--json", "board", "current"]);
    assert_contract("board current", "board-current");
    assert_fixture_shape(&board_current, "board-current");
    assert_eq!(board_current["data"]["board"], "default");

    let board_create = host.json(&["--json", "board", "create", "fixture", "--name", "Fixture"]);
    assert_contract("board create", "board-create");
    assert_fixture_shape(&board_create, "board-create");
    assert_eq!(board_create["data"]["slug"], "fixture");

    let board_list = host.json(&["--json", "board", "list"]);
    assert_contract("board list", "board-list");
    assert_fixture_shape(&board_list, "board-list");
    assert!(
        board_list["data"]
            .as_array()
            .expect("board list data array")
            .iter()
            .any(|board| board["slug"] == "fixture")
    );

    let board_show = host.json(&["--json", "board", "show", "fixture"]);
    assert_contract("board show", "board-show");
    assert_fixture_shape(&board_show, "board-show");
    assert_eq!(board_show["data"]["name"], "Fixture");

    let create = host.json(&[
        "--json",
        "task",
        "create",
        "Created contract task",
        "--description",
        "initial description",
        "--status",
        "todo",
        "--priority",
        "2",
        "--max-retries",
        "3",
        "--task-id",
        "t_create",
    ]);
    assert_contract("task create", "task-create");
    assert_fixture_shape(&create, "task-create");
    assert_eq!(create["data"]["title"], "Created contract task");
    assert_eq!(create["data"]["description"], "initial description");
    assert_eq!(create["data"]["status"], "todo");
    assert_eq!(create["data"]["priority"], 2);

    let task = host.json(&[
        "--json",
        "task",
        "create",
        "Fixture task",
        "--description",
        "ready spec",
        "--status",
        "todo",
        "--priority",
        "1",
        "--max-retries",
        "2",
        "--task-id",
        "t_queue",
    ]);
    assert_eq!(task["data"]["id"], "t_queue");

    let list = host.json(&["--json", "task", "list"]);
    assert_contract("task list", "task-list");
    assert_fixture_shape(&list, "task-list");
    let tasks = list["data"].as_array().expect("task list data array");
    assert!(tasks.iter().any(|task| task["title"] == "Fixture task"));

    let show = host.json(&["--json", "task", "show", "t_queue"]);
    assert_contract("task show", "task-show");
    assert_fixture_shape(&show, "task-show");
    assert_eq!(show["data"]["title"], "Fixture task");
    assert_eq!(show["data"]["description"], "ready spec");

    let update = host.json(&[
        "--json",
        "task",
        "update",
        "t_queue",
        "--title",
        "Updated contract task",
        "--description",
        "updated description",
        "--priority",
        "1",
        "--max-retries",
        "4",
    ]);
    assert_contract("task update", "task-update");
    assert_fixture_shape(&update, "task-update");
    assert_eq!(update["data"]["title"], "Updated contract task");
    assert_eq!(update["data"]["description"], "updated description");
    assert_eq!(update["data"]["max_retries"], 4);

    let archived_board = host.json(&["--json", "board", "archive", "fixture"]);
    assert_contract("board archive", "board-archive");
    assert_fixture_shape(&archived_board, "board-archive");
    assert!(archived_board["data"]["archived_at"].is_number());
}

#[test]
fn queue_cli_reports_server_unavailable_with_stable_hint() {
    let temp = tempfile::tempdir().expect("创建不可用 host 临时目录");
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("分配不可用 host 端口")
        .local_addr()
        .expect("读取不可用 host 端口")
        .port();
    let output = Command::new(env!("CARGO_BIN_EXE_kanban"))
        .current_dir(temp.path())
        .env_remove("KANBAN_SERVER_URL")
        .env_remove("KANBAN_ACTOR")
        .env_remove("KB_BOARD")
        .env("XDG_CONFIG_HOME", temp.path().join("xdg-config"))
        .env("XDG_DATA_HOME", temp.path().join("xdg-data"))
        .args([
            "--json",
            "--server-url",
            &format!("http://127.0.0.1:{port}"),
            "task",
            "list",
        ])
        .output()
        .expect("运行不可用 host CLI");
    assert_eq!(output.status.code(), Some(9));
    let value: Value = serde_json::from_slice(&output.stdout).expect("解析 server unavailable");
    assert_eq!(value["error"]["code"], "server_unavailable");
    assert!(
        value["error"]["message"]
            .as_str()
            .expect("server unavailable message")
            .contains("server unavailable")
    );
    assert!(!output.stderr.is_empty() || output.status.code() == Some(9));
}
