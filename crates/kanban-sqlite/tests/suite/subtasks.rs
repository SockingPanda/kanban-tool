use crate::common::*;

fn subtask_kinds(events: &[kanban_sqlite::EventRecord]) -> Vec<String> {
    events.iter().map(|event| event.kind.clone()).collect()
}

#[test]
fn create_subtask_creates_task_relation_plan_and_events_without_dependency() -> anyhow::Result<()> {
    let temp = TempDb::new("create_subtask_creates_task_relation_plan_and_events")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("parent execution task"),
    )?;

    let relation = create_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateSubtaskInput {
            task: CreateTask::ready("child step task"),
            position: Some(2048),
            required: true,
        },
    )?;

    assert_eq!(relation.parent_task_id, parent.id);
    assert_eq!(relation.position, 2048);
    assert!(relation.required);
    assert_eq!(relation.child_task.title, "child step task");
    assert_eq!(relation.child_task.board_id, parent.board_id);

    let subtasks = list_subtasks(&temp.path, "default", &parent.id)?;
    assert_eq!(subtasks, vec![relation.clone()]);

    let plan = execution_plan(&temp.path, "default", &parent.id)?;
    assert_eq!(plan.state, StepPlanState::Planned);
    assert_eq!(plan.reason, None);

    let dependencies = list_dependencies(&temp.path, "default", &parent.id)?;
    assert!(dependencies.is_empty());
    let child_dependencies = list_dependencies(&temp.path, "default", &relation.child_task.id)?;
    assert!(child_dependencies.is_empty());

    let events = list_events(&temp.path, "default", Some(&parent.id))?;
    let kinds = subtask_kinds(&events);
    assert!(
        kinds.contains(&"task.subtask.created".to_owned()),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&"task.execution_plan.planned".to_owned()),
        "{kinds:?}"
    );
    Ok(())
}

#[test]
fn attach_subtask_rejects_cross_board_self_cycle_and_archived_relations() -> anyhow::Result<()> {
    let temp = TempDb::new("attach_subtask_rejects_invalid_relations")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("child"))?;
    let other_child = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other child"),
    )?;

    let self_err = result_err(attach_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        AttachSubtaskInput {
            child_ref: parent.id.clone(),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        self_err.to_string().contains("cannot be its own subtask"),
        "{self_err}"
    );

    let cross_board_err = result_err(attach_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        AttachSubtaskInput {
            child_ref: other_child.id.clone(),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        cross_board_err
            .to_string()
            .contains("cross-board subtask child"),
        "{cross_board_err}"
    );

    attach_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        AttachSubtaskInput {
            child_ref: child.id.clone(),
            position: Some(1024),
            required: true,
        },
    )?;
    let duplicate = attach_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        AttachSubtaskInput {
            child_ref: child.id.clone(),
            position: Some(9999),
            required: false,
        },
    )?;
    assert_eq!(duplicate.position, 1024);
    assert!(duplicate.required);
    let cycle_err = result_err(attach_subtask(
        &temp.path,
        "default",
        "tester",
        &child.id,
        AttachSubtaskInput {
            child_ref: parent.id.clone(),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        cycle_err.to_string().contains("subtask cycle"),
        "{cycle_err}"
    );

    let archived = archive_task(&temp.path, "default", "tester", &child.id, false)?;
    let archived_child_err = result_err(attach_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        AttachSubtaskInput {
            child_ref: archived.id.clone(),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        archived_child_err.to_string().contains("archived"),
        "{archived_child_err}"
    );
    Ok(())
}

#[test]
fn mark_execution_plan_not_required_requires_reason_and_no_required_subtasks() -> anyhow::Result<()>
{
    let temp = TempDb::new("mark_execution_plan_not_required")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;

    let empty_reason_err = result_err(mark_execution_plan_not_required(
        &temp.path, "default", "tester", &parent.id, "   ",
    ))?;
    assert!(
        empty_reason_err.to_string().contains("reason"),
        "{empty_reason_err}"
    );

    let plan = mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "tiny text-only task",
    )?;
    assert_eq!(plan.state, StepPlanState::NotRequired);
    assert_eq!(plan.reason.as_deref(), Some("tiny text-only task"));

    let relation = create_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateSubtaskInput {
            task: CreateTask::ready("required child"),
            position: None,
            required: true,
        },
    )?;
    let blocked_err = result_err(mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "already has a required step",
    ))?;
    assert!(
        blocked_err.to_string().contains("required subtask"),
        "{blocked_err}"
    );

    update_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &relation.child_task.id,
        UpdateSubtaskInput {
            position: None,
            required: Some(false),
        },
    )?;
    let derived_plan = execution_plan(&temp.path, "default", &parent.id)?;
    assert_eq!(derived_plan.state, StepPlanState::Unplanned);
    let plan = mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "only optional exploration remains",
    )?;
    assert_eq!(plan.state, StepPlanState::NotRequired);
    Ok(())
}

#[test]
fn doctor_reports_subtask_cycles_inserted_outside_service() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_subtask_cycles")?;
    init_database(&temp.path, "tester")?;
    let a = create_task(&temp.path, "default", "tester", CreateTask::ready("a"))?;
    let b = create_task(&temp.path, "default", "tester", CreateTask::ready("b"))?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO task_subtasks(board_id,parent_task_id,child_task_id,position,required,created_by,created_at) VALUES (?1,?2,?3,1024,1,'tester',1)",
        params![a.board_id, a.id, b.id],
    )?;
    conn.execute(
        "INSERT INTO task_subtasks(board_id,parent_task_id,child_task_id,position,required,created_by,created_at) VALUES (?1,?2,?3,2048,1,'tester',2)",
        params![b.board_id, b.id, a.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(
        report
            .consistency_issues
            .iter()
            .any(|issue| issue.code == "task_subtask_cycle"),
        "{:#?}",
        report.consistency_issues
    );
    Ok(())
}

#[test]
fn doctor_reports_subtask_orphan_relations_inserted_outside_service() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_subtask_orphan_relations")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "INSERT INTO task_subtasks(board_id,parent_task_id,child_task_id,position,required,created_by,created_at) VALUES (?1,?2,'t_missing_child',1024,1,'tester',1)",
        params![parent.board_id, parent.id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok, "{report:#?}");
    assert!(
        report.consistency_issues.iter().any(|issue| {
            issue.code == "sqlite_foreign_key_violation"
                && issue.message.contains("table=task_subtasks")
        }),
        "{:#?}",
        report.consistency_issues
    );
    Ok(())
}

#[test]
fn task_neighborhood_includes_subtask_edges_without_dependency_blocking() -> anyhow::Result<()> {
    let temp = TempDb::new("task_neighborhood_includes_subtask_edges")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let relation = create_subtask(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateSubtaskInput {
            task: CreateTask::ready("child step"),
            position: Some(1024),
            required: true,
        },
    )?;

    let graph = kanban_sqlite::task_neighborhood(
        &temp.path,
        &parent.id,
        kanban_sqlite::TaskNeighborhoodOptions::default(),
    )?;

    assert!(graph.nodes.iter().any(|node| {
        node.task.id == relation.child_task.id
            && node.role == kanban_sqlite::TaskGraphNodeRole::SubtaskChild
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source_task_id == parent.id
            && edge.target_task_id == relation.child_task.id
            && edge.kind == kanban_sqlite::TaskGraphEdgeKind::Subtask
            && edge.required
            && !edge.blocking
    }));
    Ok(())
}
