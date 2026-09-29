use kanban_protocol::{
    AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
    CreateBoardLabelRequest, CreateBoardLabelResponse, DeleteBoardLabelPath, DeleteBoardLabelQuery,
    DeleteBoardLabelResponse, ListBoardLabelsResponse, ListTaskLabelsPath, ListTaskLabelsResponse,
    RemoveTaskLabelPath, RemoveTaskLabelResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

// 格式契约保留独立字段预期；不把静态 fixture 反序列化称为真实 router 证据。
fn check_fixture<T>(
    name: &str,
    raw: &str,
    expected_fields: fn(&T) -> bool,
    expectation: &str,
) -> Result<(), String>
where
    T: DeserializeOwned + Serialize,
{
    let expected: Value =
        serde_json::from_str(raw).map_err(|error| format!("{name}: fixture JSON: {error}"))?;
    let value: T =
        serde_json::from_str(raw).map_err(|error| format!("{name}: DTO 解码: {error}"))?;
    if !expected_fields(&value) {
        return Err(format!("{name}: 字段预期不成立: {expectation}"));
    }
    let actual =
        serde_json::to_value(value).map_err(|error| format!("{name}: DTO 编码: {error}"))?;
    if actual != expected {
        return Err(format!(
            "{name}: 往返不一致\nexpected={expected}\nactual={actual}"
        ));
    }
    Ok(())
}

#[test]
fn label_fixture_contracts() {
    macro_rules! check {
        ($ty:ty, $name:literal, $expected:expr) => {
            check_fixture::<$ty>(
                $name,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../schemas/fixtures/api/",
                    $name,
                    ".v1.valid.json"
                )),
                $expected,
                stringify!($expected),
            )
        };
    }

    let results = [
        check!(BoardLabelPath, "list-board-labels-path", |value| {
            value.board == "fixture"
        }),
        check!(
            ListBoardLabelsResponse,
            "list-board-labels-response",
            |value| { value.data.is_empty() }
        ),
        check!(BoardLabelPath, "create-board-label-path", |value| {
            value.board == "fixture"
        }),
        check!(
            CreateBoardLabelRequest,
            "create-board-label-request",
            |value| { value.name == "fixture" }
        ),
        check!(
            CreateBoardLabelResponse,
            "create-board-label-response",
            |value| { value.data.name == "fixture" }
        ),
        check!(DeleteBoardLabelPath, "delete-board-label-path", |value| {
            value.board == "fixture" && value.label_id == "l_fixture"
        }),
        check!(DeleteBoardLabelQuery, "delete-board-label-query", |value| {
            !value.force
        }),
        check!(
            DeleteBoardLabelResponse,
            "delete-board-label-response",
            |value| {
                value.data.label.name == "fixture"
                    && value.data.forced
                    && value.data.removed_task_bindings == 1
            }
        ),
        check!(ListTaskLabelsPath, "list-task-labels-path", |value| {
            value.task_id == "t_fixture"
        }),
        check!(
            ListTaskLabelsResponse,
            "list-task-labels-response",
            |value| {
                value
                    .data
                    .first()
                    .is_some_and(|label| label.name == "后端-api")
            }
        ),
        check!(AddTaskLabelPath, "add-task-label-path", |value| {
            value.task_id == "t_fixture"
        }),
        check!(AddTaskLabelRequest, "add-task-label-request", |value| {
            value
                .label_names()
                .is_ok_and(|names| names == vec!["后端-api"])
        }),
        check!(AddTaskLabelResponse, "add-task-label-response", |value| {
            value.data.labels.len() == 1
                && value
                    .meta
                    .as_ref()
                    .is_some_and(|meta| meta.created_labels.len() == 1)
        }),
        check!(RemoveTaskLabelPath, "remove-task-label-path", |value| {
            value.label_id == "l_fixture"
        }),
        check!(
            RemoveTaskLabelResponse,
            "remove-task-label-response",
            |value| { value.data.labels.is_empty() }
        ),
    ];
    let failures = results
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "label 格式契约失败:\n{}",
        failures.join("\n")
    );
}
