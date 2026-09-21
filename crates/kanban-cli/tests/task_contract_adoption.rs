use kanban_protocol::{CliTaskBlockOutput, CliTaskDoneOutput};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

#[allow(dead_code)]
mod support;
use support::TestHost;

fn assert_fixture_roundtrip<T>(raw: &str)
where
    T: DeserializeOwned + Serialize,
{
    let expected: Value = serde_json::from_str(raw).expect("CLI task fixture JSON");
    let value: T = serde_json::from_value(expected.clone()).expect("CLI task DTO");
    assert_eq!(
        serde_json::to_value(value).expect("serialize CLI task DTO"),
        expected
    );
}

#[test]
fn task_done_output_contract() {
    assert_fixture_roundtrip::<CliTaskDoneOutput>(include_str!(
        "../../../schemas/fixtures/cli/task-done-output.v1.valid.json"
    ));
}

#[test]
fn task_block_output_contract() {
    assert_fixture_roundtrip::<CliTaskBlockOutput>(include_str!(
        "../../../schemas/fixtures/cli/task-block-output.v1.valid.json"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_cli_manages_membership_hierarchy_and_frozen_cycles() {
    let host = TestHost::start().await;
    let module = host.json(&[
        "--json",
        "module",
        "create",
        "模块",
        "--request-id",
        "cli-module",
    ]);
    let module_id = module["data"]["object"]["id"].as_str().unwrap();
    let cycle = host.json(&[
        "--json",
        "iteration",
        "create",
        "迭代",
        "--starts-at",
        "1",
        "--ends-at",
        "9007199254740993",
        "--request-id",
        "cli-cycle",
    ]);
    let cycle_id = cycle["data"]["object"]["id"].as_str().unwrap();
    assert_eq!(cycle["data"]["object"]["ends_at"], 9007199254740993_i64);
    let create = [
        "--json",
        "task",
        "create",
        "带归属任务",
        "--description",
        "规格",
        "--module",
        module_id,
        "--cycle",
        cycle_id,
        "--idempotency-key",
        "cli-member",
    ];
    let task = host.json(&create);
    let id = task["data"]["id"].as_str().unwrap();
    assert_eq!(task["data"]["module_ids"], serde_json::json!([module_id]));
    assert_eq!(task["data"]["cycle_id"], cycle_id);
    assert!(task["data"]["object_version"].as_i64().unwrap() >= 1);
    assert_eq!(host.json(&create), task);
    let client = kanban_client::KanbanClient::new(host.server_url(), "cli-object-proof").unwrap();
    let board = client.get_board("default").await.unwrap();
    let object = client
        .get_object(kanban_protocol::rpc::extensions::ObjectIdentityInput {
            board_id: board.id.clone(),
            object_id: id.into(),
        })
        .await
        .unwrap();
    let object = kanban_protocol::rpc::decode_json(object.data.unwrap()).unwrap();
    assert_eq!(object["properties"]["task.modules"][0]["value"], module_id);
    assert_eq!(object["properties"]["task.cycle"][0]["value"], cycle_id);
    let list = host.json(&[
        "--json", "task", "list", "--module", module_id, "--cycle", cycle_id,
    ]);
    assert_eq!(list["data"][0]["id"], id);
    let child = host.json(&[
        "--json",
        "module",
        "create",
        "子模块",
        "--parent",
        module_id,
    ]);
    let child_id = child["data"]["object"]["id"].as_str().unwrap();
    let parents = host.json(&["--json", "module", "list", "--parent", module_id]);
    assert_eq!(parents["total"], 1);
    assert!(parents["data"][0].get("body").is_none());
    let changed = host.json(&[
        "--json",
        "module",
        "update",
        child_id,
        "--title",
        "更名",
        "--body",
        "正文",
        "--clear-parent",
        "--request-id",
        "cli-edit",
    ]);
    assert!(changed["data"]["object"]["parent_id"].is_null());
    assert_eq!(changed["data"]["object"]["body"], "正文");
    host.json(&["--json", "cycle", "start", cycle_id]);
    host.json(&["--json", "cycle", "close", cycle_id]);
    let frozen = host.json(&["--json", "cycle", "task", "list", cycle_id, "--limit", "1"]);
    assert_eq!(frozen["source"], "frozen_snapshot");
    assert_eq!(frozen["total"], 1);
    assert_eq!(frozen["data"][0]["id"], id);
    let current = host.json(&["--json", "task", "show", id]);
    assert!(current["data"]["cycle_id"].is_null());
    assert_eq!(current["data"]["status"], task["data"]["status"]);
    host.json(&[
        "--json",
        "task",
        "update",
        id,
        "--clear-modules",
        "--clear-cycle",
        "--request-id",
        "cli-clear",
    ]);
    assert!(
        host.json(&create)["data"]["module_ids"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let bad = host.run(&["--json", "task", "create", "无效", "--module", cycle_id]);
    assert!(!bad.status.success());
    let object = client
        .get_object(kanban_protocol::rpc::extensions::ObjectIdentityInput {
            board_id: board.id,
            object_id: child_id.into(),
        })
        .await
        .unwrap();
    let object = kanban_protocol::rpc::decode_json(object.data.unwrap()).unwrap();
    assert_eq!(object["title"], "更名");
    assert_eq!(object["body"], "正文");
}
