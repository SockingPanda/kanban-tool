mod support;

use std::time::Duration;

use serde_json::Value;
use tokio::time::sleep;

use support::{TestHost, assert_contract, assert_fixture_shape, assert_success};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_cli_covers_runs_logs_comments_attachments_events_and_stats() {
    let host = TestHost::start_with_dispatcher().await;
    host.json(&[
        "--json",
        "task",
        "create",
        "History task",
        "--description",
        "history specification",
        "--status",
        "todo",
        "--task-id",
        "t_history",
    ]);
    let plan = host.json(&[
        "--json",
        "task",
        "step",
        "not-required",
        "t_history",
        "--reason",
        "single action",
    ]);
    assert_contract("task step not-required", "task-step-not-required");
    assert_fixture_shape(&plan, "task-step-not-required");
    host.json(&["--json", "task", "promote", "t_history"]);

    let runs = wait_for_run(&host, "t_history").await;
    assert_contract("runs", "runs");
    assert_fixture_shape(&runs, "runs");
    let run_id = runs["data"][0]["id"]
        .as_str()
        .expect("dispatcher run id")
        .to_owned();
    assert_eq!(runs["data"][0]["task_id"], "t_history");

    let run = host.json(&["--json", "run", "show", &run_id]);
    assert_contract("run show", "run-show");
    assert_fixture_shape(&run, "run-show");
    assert_eq!(run["data"]["id"], run_id);

    let log = host.json(&["--json", "run", "logs", &run_id]);
    assert_contract("run logs", "run-logs");
    assert_fixture_shape(&log, "run-logs");
    assert!(
        log["data"]["content"]
            .as_str()
            .expect("run log content")
            .contains("fixture log")
    );

    let comment = host.json(&[
        "--json",
        "comment",
        "add",
        "t_history",
        "line one\nline two",
        "--kind",
        "note",
        "--author",
        "fixture-agent",
        "--author-type",
        "agent",
        "--agent-type",
        "executor",
        "--metadata-json",
        r#"{"source":"fixture"}"#,
        "--idempotency-key",
        "comment-key",
    ]);
    assert_contract("comment add", "comment-add");
    assert_fixture_shape(&comment, "comment-add");
    assert_eq!(comment["data"]["body"], "line one\nline two");
    assert_eq!(comment["data"]["kind"], "note");

    let comments = host.json(&["--json", "comment", "list", "t_history"]);
    assert_contract("comment list", "comment-list");
    assert_fixture_shape(&comments, "comment-list");
    assert_eq!(comments["data"].as_array().unwrap().len(), 1);
    assert_eq!(comments["data"][0]["body"], "line one\nline two");

    let source_path = host.project_path().join("artifact.txt");
    std::fs::write(&source_path, b"ab").expect("写入 attachment fixture");
    let attachment = host
        .command()
        .args([
            "--json",
            "attachment",
            "add",
            "t_history",
            "artifact.txt",
            "--filename",
            "artifact.txt",
            "--content-type",
            "text/plain",
            "--attachment-id",
            "a_history",
        ])
        .output()
        .expect("运行 attachment add");
    assert_success(
        &["--json", "attachment", "add", "t_history", "artifact.txt"],
        &attachment,
    );
    let attachment: Value = serde_json::from_slice(&attachment.stdout).expect("attachment JSON");
    assert_contract("attachment add", "attachment-add");
    assert_fixture_shape(&attachment, "attachment-add");
    assert_eq!(attachment["data"]["filename"], "artifact.txt");
    assert_eq!(attachment["data"]["size_bytes"], 2);

    let attachments = host.json(&["--json", "attachment", "list", "t_history"]);
    assert_contract("attachment list", "attachment-list");
    assert_fixture_shape(&attachments, "attachment-list");
    assert_eq!(attachments["data"][0]["id"], "a_history");

    let downloaded_path = host.project_path().join("downloaded.txt");
    let downloaded = host
        .command()
        .args(["--json", "attachment", "download", "t_history", "a_history"])
        .arg(&downloaded_path)
        .output()
        .expect("运行 attachment download");
    assert_success(
        &["--json", "attachment", "download", "t_history", "a_history"],
        &downloaded,
    );
    assert_eq!(
        std::fs::read(&downloaded_path).expect("读取下载 attachment"),
        b"ab"
    );

    let removed = host.json(&["--json", "attachment", "remove", "t_history", "a_history"]);
    assert_contract("attachment remove", "attachment-remove");
    assert_fixture_shape(&removed, "attachment-remove");
    assert_eq!(removed["data"]["deleted"], true);

    let events = host.json(&["--json", "events", "t_history"]);
    assert_contract("events", "events");
    assert_fixture_shape(&events, "events");
    assert!(events["data"].as_array().unwrap().len() >= 3);

    let stats = host.json(&["--json", "stats"]);
    assert_contract("stats", "stats");
    assert_fixture_shape(&stats, "stats");
    assert!(stats["data"]["status_counts"].is_array());
}

async fn wait_for_run(host: &TestHost, task_ref: &str) -> Value {
    for _ in 0..100 {
        let runs = host.json(&["--json", "runs", task_ref]);
        if runs["data"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            return runs;
        }
        sleep(Duration::from_millis(25)).await;
    }
    panic!("dispatcher 未能为 {task_ref} 创建 run");
}
