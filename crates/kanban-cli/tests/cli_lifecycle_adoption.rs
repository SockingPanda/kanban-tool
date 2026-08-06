mod support;

use serde_json::Value;

use support::{TestHost, assert_contract, assert_fixture_shape};

fn create_task(host: &TestHost, id: &str, title: &str, status: &str) -> Value {
    let task = host.json(&[
        "--json",
        "task",
        "create",
        title,
        "--description",
        "lifecycle specification",
        "--status",
        status,
        "--priority",
        "2",
        "--max-retries",
        "4",
        "--task-id",
        id,
    ]);
    if status != "triage" {
        host.json(&[
            "--json",
            "task",
            "step",
            "not-required",
            id,
            "--reason",
            "single action",
        ]);
        if status == "ready" {
            host.json(&["--json", "task", "promote", id]);
        }
    }
    task
}

fn claim_task(host: &TestHost, id: &str) -> (Value, String) {
    let claim = host.json(&["--json", "task", "claim", id, "--ttl-ms", "300000"]);
    assert_contract("task claim", "task-claim");
    assert_fixture_shape(&claim, "task-claim");
    let token = claim["data"]["claim_token"]
        .as_str()
        .expect("claim token")
        .to_owned();
    (claim, token)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_cli_runs_each_transition_through_localhost_host() {
    let host = TestHost::start().await;

    let specified = create_task(&host, "t_specify", "Specify contract task", "triage");
    assert_eq!(specified["data"]["status"], "triage");
    let specified = host.json(&[
        "--json",
        "task",
        "specify",
        "t_specify",
        "--description",
        "specified description",
    ]);
    assert_eq!(specified["data"]["description"], "specified description");

    let promoted = create_task(&host, "t_promote", "Lifecycle contract task", "todo");
    assert_eq!(promoted["data"]["status"], "todo");
    let promoted = host.json(&["--json", "task", "promote", "t_promote"]);
    assert_contract("task promote", "task-promote");
    assert_fixture_shape(&promoted, "task-promote");
    assert_eq!(promoted["data"]["status"], "ready");

    create_task(&host, "t_claim", "Claim contract task", "ready");
    let (_, claim_token) = claim_task(&host, "t_claim");
    let heartbeat = host.json(&[
        "--json",
        "task",
        "heartbeat",
        "t_claim",
        "--claim-token",
        &claim_token,
        "--ttl-ms",
        "300000",
        "--note",
        "alive",
    ]);
    assert_contract("task heartbeat", "task-heartbeat");
    assert_fixture_shape(&heartbeat, "task-heartbeat");
    assert_eq!(heartbeat["data"]["status"], "running");

    let review = host.json(&[
        "--json",
        "task",
        "review",
        "t_claim",
        "--claim-token",
        &claim_token,
    ]);
    assert_contract("task review", "task-review");
    assert_fixture_shape(&review, "task-review");
    assert_eq!(review["data"]["status"], "review");

    let done = host.json(&[
        "--json",
        "task",
        "done",
        "t_claim",
        "--claim-token",
        &claim_token,
    ]);
    assert_contract("task done", "task-done");
    assert_fixture_shape(&done, "task-done");
    assert_eq!(done["data"]["status"], "done");

    create_task(&host, "t_release", "Release contract task", "ready");
    let (_, release_token) = claim_task(&host, "t_release");
    let release = host.json(&[
        "--json",
        "task",
        "release",
        "t_release",
        "--claim-token",
        &release_token,
    ]);
    assert_contract("task release", "task-release");
    assert_fixture_shape(&release, "task-release");
    assert_eq!(release["data"]["status"], "ready");

    create_task(&host, "t_block", "Block contract task", "ready");
    let (_, block_token) = claim_task(&host, "t_block");
    let block = host.json(&[
        "--json",
        "task",
        "block",
        "t_block",
        "contract blocked",
        "--claim-token",
        &block_token,
    ]);
    assert_contract("task block", "task-block");
    assert_fixture_shape(&block, "task-block");
    assert_eq!(block["data"]["status"], "blocked");

    let unblock = host.json(&["--json", "task", "unblock", "t_block"]);
    assert_contract("task unblock", "task-unblock");
    assert_fixture_shape(&unblock, "task-unblock");
    assert_eq!(unblock["data"]["status"], "ready");

    create_task(&host, "t_reopen", "Reopen contract task", "ready");
    let (_, reopen_token) = claim_task(&host, "t_reopen");
    let _ = host.json(&[
        "--json",
        "task",
        "done",
        "t_reopen",
        "--claim-token",
        &reopen_token,
    ]);
    let reopen = host.json(&[
        "--json",
        "task",
        "reopen",
        "t_reopen",
        "--reason",
        "new evidence",
    ]);
    assert_contract("task reopen", "task-reopen");
    assert_fixture_shape(&reopen, "task-reopen");
    assert_eq!(reopen["data"]["status"], "ready");

    create_task(&host, "t_reclaim", "Reclaim contract task", "ready");
    let (_, _) = claim_task(&host, "t_reclaim");
    let reclaim = host.json(&[
        "--json",
        "task",
        "reclaim",
        "t_reclaim",
        "--force",
        "--to-status",
        "ready",
        "--reason",
        "operator reclaim",
    ]);
    assert_contract("task reclaim", "task-reclaim");
    assert_fixture_shape(&reclaim, "task-reclaim");
    assert_eq!(reclaim["data"]["id"], "t_reclaim");
    assert_eq!(reclaim["data"]["status"], "ready");

    create_task(&host, "t_archive", "Archive contract task", "todo");
    let archive = host.json(&["--json", "task", "archive", "t_archive", "--force"]);
    assert_contract("task archive", "task-archive");
    assert_fixture_shape(&archive, "task-archive");
    assert_eq!(archive["data"]["status"], "archived");
}
