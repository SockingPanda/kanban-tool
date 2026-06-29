mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use serde_json::Value;

#[test]
fn dag_command_is_removed_from_help_and_rejected() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_command_is_removed_from_help_and_rejected")?;

    let stdout = kanban(&temp.path, &["--help"])?.success_stdout()?;
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim_start().starts_with("dag")),
        "{stdout}"
    );

    kanban(&temp.path, &["dag", "show"])?.failure_containing_any(&[
        "unrecognized subcommand",
        "unexpected argument",
        "invalid subcommand",
    ])?;
    Ok(())
}

#[test]
fn dep_add_remove_human_output_is_chinese() -> anyhow::Result<()> {
    let temp = TempDb::new("dep_add_remove_human_output_is_chinese")?;
    kanban(&temp.path, &["init"])?.success()?;

    let parent = create_task(&temp, "parent", "ready")?;
    let child = create_task(&temp, "child", "ready")?;

    let added = kanban(&temp.path, &["dep", "add", &parent.id, &child.id])?.success_stdout()?;
    assert!(added.contains("已添加依赖"));
    assert!(added.contains(&parent.id));
    assert!(added.contains(&child.id));

    let removed =
        kanban(&temp.path, &["dep", "remove", &parent.id, &child.id])?.success_stdout()?;
    assert!(removed.contains("已移除依赖"));
    assert!(removed.contains(&parent.id));
    assert!(removed.contains(&child.id));
    Ok(())
}

#[test]
fn dep_json_outputs_hydrated_dependency_snapshot() -> anyhow::Result<()> {
    let temp = TempDb::new("dep_json_outputs_hydrated_dependency_snapshot")?;
    kanban(&temp.path, &["init"])?.success()?;

    let parent = create_task(&temp, "parent", "ready")?;
    let child = create_task(&temp, "child", "ready")?;

    let add =
        kanban(&temp.path, &["--json", "dep", "add", &parent.id, &child.id])?.success_json()?;
    assert_eq!(add["data"]["edge"]["parent"]["id"], parent.id.as_str());
    assert_eq!(
        add["data"]["edge"]["parent"]["ref"],
        parent.task_ref.as_str()
    );
    assert_eq!(add["data"]["edge"]["parent"]["title"], "parent");
    assert_eq!(add["data"]["edge"]["parent"]["status"], "todo");
    assert_eq!(add["data"]["edge"]["child"]["id"], child.id.as_str());
    assert_eq!(
        add["data"]["dependencies"]["parents"][0]["id"],
        parent.id.as_str()
    );
    assert_eq!(
        add["data"]["dependencies"]["children"]
            .as_array()
            .context("children")?
            .len(),
        0
    );
    let normalized_add = normalize_dependency_json(add, &parent, &child);
    insta::assert_json_snapshot!(normalized_add, @r###"
    {
      "data": {
        "dependencies": {
          "children": [],
          "edges": [
            {
              "child": {
                "board_id": "b_BOARD",
                "board_slug": "default",
                "id": "t_CHILD",
                "ref": "default#2",
                "status": "todo",
                "title": "child"
              },
              "parent": {
                "board_id": "b_BOARD",
                "board_slug": "default",
                "id": "t_PARENT",
                "ref": "default#1",
                "status": "todo",
                "title": "parent"
              }
            }
          ],
          "parents": [
            {
              "board_id": "b_BOARD",
              "board_slug": "default",
              "id": "t_PARENT",
              "ref": "default#1",
              "status": "todo",
              "title": "parent"
            }
          ],
          "task": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_CHILD",
            "ref": "default#2",
            "status": "todo",
            "title": "child"
          }
        },
        "edge": {
          "child": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_CHILD",
            "ref": "default#2",
            "status": "todo",
            "title": "child"
          },
          "parent": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_PARENT",
            "ref": "default#1",
            "status": "todo",
            "title": "parent"
          }
        }
      }
    }
    "###);

    let list = kanban(&temp.path, &["--json", "dep", "list", &child.id])?.success_json()?;
    assert_eq!(list["data"]["task"]["id"], child.id.as_str());
    assert_eq!(list["data"]["parents"][0]["ref"], parent.task_ref.as_str());
    assert_eq!(list["data"]["edges"][0]["parent"]["id"], parent.id.as_str());
    let normalized_list = normalize_dependency_json(list, &parent, &child);
    insta::assert_json_snapshot!(normalized_list, @r###"
    {
      "data": {
        "children": [],
        "edges": [
          {
            "child": {
              "board_id": "b_BOARD",
              "board_slug": "default",
              "id": "t_CHILD",
              "ref": "default#2",
              "status": "todo",
              "title": "child"
            },
            "parent": {
              "board_id": "b_BOARD",
              "board_slug": "default",
              "id": "t_PARENT",
              "ref": "default#1",
              "status": "todo",
              "title": "parent"
            }
          }
        ],
        "parents": [
          {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_PARENT",
            "ref": "default#1",
            "status": "todo",
            "title": "parent"
          }
        ],
        "task": {
          "board_id": "b_BOARD",
          "board_slug": "default",
          "id": "t_CHILD",
          "ref": "default#2",
          "status": "todo",
          "title": "child"
        }
      }
    }
    "###);

    let remove = kanban(
        &temp.path,
        &["--json", "dep", "remove", &parent.id, &child.id],
    )?
    .success_json()?;
    assert_eq!(remove["data"]["edge"]["parent"]["id"], parent.id.as_str());
    assert!(
        remove["data"]["dependencies"]["parents"]
            .as_array()
            .context("parents")?
            .is_empty()
    );
    assert!(
        remove["data"]["dependencies"]["edges"]
            .as_array()
            .context("edges")?
            .is_empty()
    );
    let normalized_remove = normalize_dependency_json(remove, &parent, &child);
    insta::assert_json_snapshot!(normalized_remove, @r###"
    {
      "data": {
        "dependencies": {
          "children": [],
          "edges": [],
          "parents": [],
          "task": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_CHILD",
            "ref": "default#2",
            "status": "todo",
            "title": "child"
          }
        },
        "edge": {
          "child": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_CHILD",
            "ref": "default#2",
            "status": "todo",
            "title": "child"
          },
          "parent": {
            "board_id": "b_BOARD",
            "board_slug": "default",
            "id": "t_PARENT",
            "ref": "default#1",
            "status": "todo",
            "title": "parent"
          }
        }
      }
    }
    "###);
    Ok(())
}

#[test]
fn dep_list_json_handles_empty_snapshot() -> anyhow::Result<()> {
    let temp = TempDb::new("dep_list_json_handles_empty_snapshot")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = create_task(&temp, "solo", "ready")?;

    let list = kanban(&temp.path, &["--json", "dep", "list", &task.id])?.success_json()?;

    assert_eq!(list["data"]["task"]["id"], task.id.as_str());
    assert!(
        list["data"]["parents"]
            .as_array()
            .context("parents")?
            .is_empty()
    );
    assert!(
        list["data"]["children"]
            .as_array()
            .context("children")?
            .is_empty()
    );
    assert!(
        list["data"]["edges"]
            .as_array()
            .context("edges")?
            .is_empty()
    );
    let mut normalized = list;
    replace_dynamic_string(&mut normalized, &task.id, "t_TASK");
    replace_dynamic_prefix(&mut normalized, "b_", "b_BOARD");
    insta::assert_json_snapshot!(normalized, @r###"
    {
      "data": {
        "children": [],
        "edges": [],
        "parents": [],
        "task": {
          "board_id": "b_BOARD",
          "board_slug": "default",
          "id": "t_TASK",
          "ref": "default#1",
          "status": "todo",
          "title": "solo"
        }
      }
    }
    "###);
    Ok(())
}

#[derive(Debug)]
struct CreatedTask {
    id: String,
    task_ref: String,
}

fn create_task(temp: &TempDb, title: &str, status: &str) -> anyhow::Result<CreatedTask> {
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            title,
            "--description",
            "ready spec",
            "--status",
            status,
        ],
    )?
    .success_json()?;
    let id = created["data"]["id"]
        .as_str()
        .context("expected task id")?
        .to_owned();
    let task_ref = created["data"]["ref"]
        .as_str()
        .context("expected task ref")?
        .to_owned();
    Ok(CreatedTask { id, task_ref })
}

fn normalize_dependency_json(mut json: Value, parent: &CreatedTask, child: &CreatedTask) -> Value {
    replace_dynamic_string(&mut json, &parent.id, "t_PARENT");
    replace_dynamic_string(&mut json, &child.id, "t_CHILD");
    replace_dynamic_prefix(&mut json, "b_", "b_BOARD");
    json
}

fn replace_dynamic_string(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(string) if string == from => *string = to.to_owned(),
        Value::Array(items) => {
            for item in items {
                replace_dynamic_string(item, from, to);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                replace_dynamic_string(item, from, to);
            }
        }
        _ => {}
    }
}

fn replace_dynamic_prefix(value: &mut Value, prefix: &str, to: &str) {
    match value {
        Value::String(string) if string.starts_with(prefix) => *string = to.to_owned(),
        Value::Array(items) => {
            for item in items {
                replace_dynamic_prefix(item, prefix, to);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                replace_dynamic_prefix(item, prefix, to);
            }
        }
        _ => {}
    }
}
