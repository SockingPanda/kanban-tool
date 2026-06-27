use crate::common::*;

fn step_kinds(events: &[kanban_sqlite::EventRecord]) -> Vec<String> {
    events.iter().map(|event| event.kind.clone()).collect()
}

#[test]
fn create_text_step_plans_task_and_events_without_dependency() -> anyhow::Result<()> {
    let temp = TempDb::new("create_text_step_plans_task_and_events")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("parent execution task"),
    )?;

    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "write rollout checklist".into(),
            body: Some("cover install, smoke, rollback".into()),
            linked_task_ref: None,
            position: Some(2048),
            required: true,
        },
    )?;

    assert_eq!(step.parent_task_id, parent.id);
    assert_eq!(step.position, 2048);
    assert!(step.required);
    assert_eq!(step.status, StepStatus::Todo);
    assert_eq!(step.title, "write rollout checklist");
    assert_eq!(step.body.as_deref(), Some("cover install, smoke, rollback"));
    assert!(step.linked_task.is_none());

    let steps = list_steps(&temp.path, "default", &parent.id)?;
    assert_eq!(steps, vec![step.clone()]);

    let plan = execution_plan(&temp.path, "default", &parent.id)?;
    assert_eq!(plan.state, StepPlanState::Planned);
    assert_eq!(plan.reason, None);

    let dependencies = list_dependencies(&temp.path, "default", &parent.id)?;
    assert!(dependencies.is_empty());

    let events = list_events(&temp.path, "default", Some(&parent.id))?;
    let kinds = step_kinds(&events);
    assert!(kinds.contains(&"task.step.created".to_owned()), "{kinds:?}");
    assert!(
        kinds.contains(&"task.execution_plan.planned".to_owned()),
        "{kinds:?}"
    );
    Ok(())
}

#[test]
fn linked_step_rejects_self_cross_board_and_archived_links() -> anyhow::Result<()> {
    let temp = TempDb::new("linked_step_rejects_invalid_links")?;
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

    let self_err = result_err(create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "bad self link".into(),
            body: None,
            linked_task_ref: Some(parent.id.clone()),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        self_err.to_string().contains("cannot link to its parent"),
        "{self_err}"
    );

    let cross_board_err = result_err(create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "bad board link".into(),
            body: None,
            linked_task_ref: Some(other_child.id.clone()),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        cross_board_err.to_string().contains("cross-board"),
        "{cross_board_err}"
    );

    let linked = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "linked normal task".into(),
            body: None,
            linked_task_ref: Some(child.id.clone()),
            position: Some(1024),
            required: true,
        },
    )?;
    assert_eq!(
        linked.linked_task.as_ref().map(|task| task.id.as_str()),
        Some(child.id.as_str())
    );

    let archived = archive_task(&temp.path, "default", "tester", &child.id, false)?;
    let archived_link_err = result_err(create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "bad archived link".into(),
            body: None,
            linked_task_ref: Some(archived.id.clone()),
            position: None,
            required: true,
        },
    ))?;
    assert!(
        archived_link_err.to_string().contains("archived"),
        "{archived_link_err}"
    );
    Ok(())
}

#[test]
fn mark_execution_plan_not_required_requires_reason_and_no_steps() -> anyhow::Result<()> {
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

    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "required step".into(),
            body: None,
            linked_task_ref: None,
            position: None,
            required: true,
        },
    )?;
    let blocked_err = result_err(mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "already has a step",
    ))?;
    assert!(
        blocked_err.to_string().contains("steps exist"),
        "{blocked_err}"
    );

    remove_step(&temp.path, "default", "tester", &parent.id, &step.id)?;
    let derived_plan = execution_plan(&temp.path, "default", &parent.id)?;
    assert_eq!(derived_plan.state, StepPlanState::Unplanned);
    let plan = mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "no structured steps needed",
    )?;
    assert_eq!(plan.state, StepPlanState::NotRequired);
    Ok(())
}

#[test]
fn step_status_is_independent_from_linked_task_and_gates_completion() -> anyhow::Result<()> {
    let temp = TempDb::new("step_status_gates_completion")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let linked = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("linked task"),
    )?;
    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "verify linked task output".into(),
            body: None,
            linked_task_ref: Some(linked.id.clone()),
            position: None,
            required: true,
        },
    )?;
    let planned = get_task(&temp.path, "default", &parent.id)?;
    assert_eq!(planned.execution_plan_state, StepPlanState::Planned);
    assert_eq!(planned.required_step_count, 1);
    assert_eq!(planned.completed_required_step_count, 0);

    let archive_err = result_err(archive_task(
        &temp.path, "default", "tester", &parent.id, false,
    ))?;
    assert!(
        matches!(archive_err, KanbanError::StepsIncomplete(_)),
        "{archive_err}"
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    let complete_err = result_err(complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    ))?;
    assert!(
        matches!(complete_err, KanbanError::StepsIncomplete(_)),
        "{complete_err}"
    );

    archive_task(&temp.path, "default", "tester", &linked.id, false)?;
    let still_todo = list_steps(&temp.path, "default", &parent.id)?;
    assert_eq!(still_todo[0].status, StepStatus::Todo);

    let completed_step = complete_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &step.id,
        "verified separately",
    )?;
    assert_eq!(completed_step.status, StepStatus::Done);
    assert_eq!(
        completed_step.resolution_note.as_deref(),
        Some("verified separately")
    );

    let completed = complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )?;
    assert_eq!(completed.status, TaskStatus::Done);
    assert_eq!(completed.completed_required_step_count, 1);
    Ok(())
}

#[test]
fn update_remove_skip_and_reopen_step_recompute_plan() -> anyhow::Result<()> {
    let temp = TempDb::new("update_remove_skip_and_reopen_step")?;
    init_database(&temp.path, "tester")?;

    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("parent required toggle"),
    )?;
    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "initial".into(),
            body: None,
            linked_task_ref: None,
            position: None,
            required: true,
        },
    )?;
    assert_eq!(
        get_task(&temp.path, "default", &parent.id)?.status,
        TaskStatus::Ready
    );

    let updated = update_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "S1",
        UpdateStepInput {
            title: Some("updated title".into()),
            body: Some(Some("updated body".into())),
            linked_task_ref: None,
            unlink_task: false,
            position: Some(4096),
            required: Some(false),
        },
    )?;
    assert_eq!(updated.id, step.id);
    assert_eq!(updated.title, "updated title");
    assert_eq!(updated.body.as_deref(), Some("updated body"));
    assert_eq!(updated.position, 4096);
    assert!(!updated.required);
    let still_planned = get_task(&temp.path, "default", &parent.id)?;
    assert_eq!(still_planned.execution_plan_state, StepPlanState::Planned);
    assert_eq!(still_planned.status, TaskStatus::Ready);

    let skipped = skip_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "S1",
        "not needed after scope trim",
    )?;
    assert_eq!(skipped.status, StepStatus::Skipped);
    assert_eq!(
        skipped.resolution_note.as_deref(),
        Some("not needed after scope trim")
    );

    let reopened = reopen_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "S1",
        "need to revisit",
    )?;
    assert_eq!(reopened.status, StepStatus::Todo);
    assert!(reopened.resolution_note.is_none());

    remove_step(&temp.path, "default", "tester", &parent.id, "S1")?;
    let detached = get_task(&temp.path, "default", &parent.id)?;
    assert_eq!(detached.execution_plan_state, StepPlanState::Unplanned);
    assert_eq!(detached.status, TaskStatus::Todo);
    Ok(())
}

#[test]
fn task_neighborhood_includes_linked_step_edges_without_dependency_blocking() -> anyhow::Result<()>
{
    let temp = TempDb::new("task_neighborhood_includes_step_edges")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("linked step task"),
    )?;
    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "linked step".into(),
            body: None,
            linked_task_ref: Some(child.id.clone()),
            position: Some(1024),
            required: true,
        },
    )?;
    create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "text only step".into(),
            body: None,
            linked_task_ref: None,
            position: Some(2048),
            required: true,
        },
    )?;

    let graph = kanban_sqlite::task_neighborhood(
        &temp.path,
        &parent.id,
        kanban_sqlite::TaskNeighborhoodOptions::default(),
    )?;

    assert!(graph.nodes.iter().any(|node| {
        node.task.id == child.id && node.role == kanban_sqlite::TaskGraphNodeRole::StepChild
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.id == format!("step:{}", step.id)
            && edge.source_task_id == parent.id
            && edge.target_task_id == child.id
            && edge.kind == kanban_sqlite::TaskGraphEdgeKind::Step
            && edge.required
            && !edge.blocking
    }));
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == kanban_sqlite::TaskGraphEdgeKind::Step)
            .count(),
        1
    );
    Ok(())
}
