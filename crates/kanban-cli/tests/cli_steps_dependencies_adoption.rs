mod support;

use support::{TestHost, assert_contract, assert_fixture_shape};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes() {
    let host = TestHost::start().await;

    host.json(&[
        "--json",
        "task",
        "create",
        "Parent",
        "--description",
        "parent specification",
        "--task-id",
        "t_parent",
    ]);
    host.json(&[
        "--json",
        "task",
        "create",
        "Child",
        "--description",
        "child specification",
        "--task-id",
        "t_child",
    ]);

    let dependency_add = host.json(&["--json", "dep", "add", "t_child", "t_parent"]);
    assert_contract("dep add", "dep-add");
    assert_fixture_shape(&dependency_add, "dep-add");
    assert_eq!(dependency_add["data"]["edge"]["child"]["id"], "t_child");
    assert_eq!(dependency_add["data"]["edge"]["parent"]["id"], "t_parent");

    let dependency_list = host.json(&["--json", "dep", "list", "t_child"]);
    assert_contract("dep list", "dep-list");
    assert_fixture_shape(&dependency_list, "dep-list");
    assert_eq!(dependency_list["data"]["parents"][0]["id"], "t_parent");

    let dependency_remove = host.json(&["--json", "dep", "remove", "t_child", "t_parent"]);
    assert_contract("dep remove", "dep-remove");
    assert_fixture_shape(&dependency_remove, "dep-remove");
    assert!(
        dependency_remove["data"]["dependencies"]["parents"]
            .as_array()
            .expect("dependency remove parents")
            .is_empty()
    );

    host.json(&[
        "--json",
        "task",
        "create",
        "Step parent",
        "--description",
        "step specification",
        "--task-id",
        "t_steps",
    ]);
    let step_add = host.json(&[
        "--json",
        "task",
        "step",
        "add",
        "t_steps",
        "Implement contract",
        "--body",
        "Preserve the public step wire",
        "--position",
        "2048",
        "--optional",
    ]);
    assert_contract("task step add", "task-step-add");
    assert_fixture_shape(&step_add, "task-step-add");
    assert_eq!(step_add["data"]["steps"][0]["title"], "Implement contract");
    assert_eq!(step_add["data"]["steps"][0]["status"], "todo");

    let step_list = host.json(&["--json", "task", "step", "list", "t_steps"]);
    assert_contract("task step list", "task-step-list");
    assert_fixture_shape(&step_list, "task-step-list");
    assert_eq!(step_list["data"]["steps"].as_array().unwrap().len(), 1);
    assert_eq!(step_list["data"]["steps"][0]["title"], "Implement contract");

    let step_update = host.json(&[
        "--json",
        "task",
        "step",
        "update",
        "t_steps",
        "S1",
        "--title",
        "Updated contract",
        "--body",
        "Updated public wire",
        "--position",
        "3072",
        "--required",
    ]);
    assert_contract("task step update", "task-step-update");
    assert_fixture_shape(&step_update, "task-step-update");
    assert_eq!(step_update["data"]["steps"][0]["title"], "Updated contract");
    assert_eq!(step_update["data"]["steps"][0]["required"], true);

    let step_done = host.json(&[
        "--json",
        "task",
        "step",
        "done",
        "t_steps",
        "S1",
        "--note",
        "contract complete",
    ]);
    assert_contract("task step done", "task-step-done");
    assert_fixture_shape(&step_done, "task-step-done");
    assert_eq!(step_done["data"]["status"], "done");

    let step_reopen = host.json(&[
        "--json",
        "task",
        "step",
        "reopen",
        "t_steps",
        "S1",
        "--reason",
        "needs revision",
    ]);
    assert_contract("task step reopen", "task-step-reopen");
    assert_fixture_shape(&step_reopen, "task-step-reopen");
    assert_eq!(step_reopen["data"]["status"], "todo");

    let step_skip = host.json(&[
        "--json",
        "task",
        "step",
        "skip",
        "t_steps",
        "S1",
        "--reason",
        "not needed",
    ]);
    assert_contract("task step skip", "task-step-skip");
    assert_fixture_shape(&step_skip, "task-step-skip");
    assert_eq!(step_skip["data"]["status"], "skipped");

    host.json(&[
        "--json",
        "task",
        "step",
        "reopen",
        "t_steps",
        "S1",
        "--reason",
        "reconsider",
    ]);
    let step_remove = host.json(&["--json", "task", "step", "remove", "t_steps", "S1"]);
    assert_contract("task step remove", "task-step-remove");
    assert_fixture_shape(&step_remove, "task-step-remove");
    assert_eq!(step_remove["data"]["removed"], true);

    let not_required = host.json(&[
        "--json",
        "task",
        "step",
        "not-required",
        "t_steps",
        "--reason",
        "single action",
    ]);
    assert_contract("task step not-required", "task-step-not-required");
    assert_fixture_shape(&not_required, "task-step-not-required");
    assert_eq!(not_required["data"]["state"], "not_required");
}
