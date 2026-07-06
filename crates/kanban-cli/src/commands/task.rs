use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{
    CreateStepInput, CreateTask, MAX_TASK_LIST_LIMIT, TaskExecutionPlanRecord, TaskListOptions,
    TaskListSort, TaskPatch, TaskPlanFilter, TaskStepRecord, UpdateStepInput, archive_task,
    block_task, claim_task, complete_step, complete_task, create_step, execution_plan, get_task,
    heartbeat_task, list_steps, list_tasks, list_tasks_page, mark_execution_plan_not_required,
    promote_task, reclaim_expired, remove_step, reopen_step, reopen_task, skip_step,
    submit_review_task, task_ontology_summary, unblock_task, update_step, update_task,
};

use crate::args::{ListArgs, TaskCommand, TaskPlanFilterArg, TaskStepCommand};
use crate::commands::common::{
    invalid_input, optional_clearable, parse_status, parse_task_list_sort,
    resolve_optional_text_input, validate_page_bounds,
};
use crate::output::{print_or_json, print_task, print_task_with_details, task_line};

pub(crate) fn handle_task(
    command: TaskCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        TaskCommand::Create(args) => {
            let description = resolve_optional_text_input(
                args.description,
                args.description_file,
                "--description",
                "--description-file",
            )?;
            let metadata_json = resolve_optional_text_input(
                args.metadata,
                args.metadata_file,
                "--metadata",
                "--metadata-file",
            )?
            .unwrap_or_else(|| "{}".to_owned());
            let task = kanban_sqlite::create_task_with_labels(
                db_path,
                board,
                actor,
                CreateTask {
                    title: args.title,
                    description,
                    status: args.status.as_deref().map(parse_status).transpose()?,
                    assignee: args.assignee,
                    priority: args.priority,
                    scheduled_at: args.scheduled_at,
                    due_at: args.due_at,
                    max_retries: args.max_retries,
                    metadata_json,
                },
                &args.labels,
            )?;
            print_task(json, &task)?;
        }
        TaskCommand::List(args) => {
            validate_page_bounds(
                args.limit.unwrap_or(100),
                MAX_TASK_LIST_LIMIT,
                args.offset.unwrap_or(0),
            )?;
            let statuses = args
                .status
                .iter()
                .map(|s| parse_status(s))
                .collect::<Result<Vec<_>>>()?;
            let plan_filters = task_plan_filters(&args)?;
            let uses_page_options = args.search.is_some()
                || args.assignee.is_some()
                || !args.labels.is_empty()
                || args.limit.is_some()
                || args.offset.is_some()
                || args.sort.is_some()
                || !plan_filters.is_empty();
            let tasks = if uses_page_options {
                list_tasks_page(
                    db_path,
                    board,
                    TaskListOptions {
                        statuses,
                        priorities: vec![],
                        labels: args.labels,
                        plan_filters,
                        include_archived: args.include_archived,
                        assignee: args.assignee,
                        search: args.search,
                        sort: args
                            .sort
                            .as_deref()
                            .map(parse_task_list_sort)
                            .transpose()?
                            .unwrap_or(TaskListSort::Position),
                        limit: args.limit.unwrap_or(100),
                        offset: args.offset.unwrap_or(0),
                    },
                )?
                .tasks
            } else {
                list_tasks(db_path, board, &statuses, args.include_archived)?
            };
            print_or_json(json, &tasks, || {
                tasks.iter().map(task_line).collect::<Vec<_>>().join("\n")
            })?;
        }
        TaskCommand::Show { task_ref, details } => {
            let task = get_task(db_path, board, &task_ref)?;
            let ontology_summary = if details {
                task_ontology_summary(db_path, board, &task_ref)?
            } else {
                None
            };
            print_task_with_details(json, details, &task, ontology_summary.as_ref())?
        }
        TaskCommand::Update(args) => {
            let description = resolve_optional_text_input(
                args.description,
                args.description_file,
                "--description",
                "--description-file",
            )?;
            let metadata_json = resolve_optional_text_input(
                args.metadata,
                args.metadata_file,
                "--metadata",
                "--metadata-file",
            )?;
            let task = update_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                TaskPatch {
                    title: args.title,
                    description: description.map(Some),
                    assignee: if args.clear_assignee {
                        Some(None)
                    } else {
                        args.assignee.map(Some)
                    },
                    priority: args.priority,
                    scheduled_at: optional_clearable(args.scheduled_at, args.clear_scheduled_at),
                    due_at: optional_clearable(args.due_at, args.clear_due_at),
                    max_retries: if args.max_retries.is_some() || args.clear_max_retries {
                        Some(if args.clear_max_retries {
                            None
                        } else {
                            args.max_retries
                        })
                    } else {
                        None
                    },
                    metadata_json,
                    expected_lock_version: args.expected_lock_version,
                },
            )?;
            print_task(json, &task)?;
        }
        TaskCommand::Promote { task_ref } => {
            print_task(json, &promote_task(db_path, board, actor, &task_ref)?)?
        }
        TaskCommand::Reopen(args) => print_task(
            json,
            &reopen_task(db_path, board, actor, &args.task_ref, &args.reason)?,
        )?,
        TaskCommand::Start(args) | TaskCommand::Claim(args) => {
            let claim = claim_task(db_path, board, actor, &args.task_ref, args.ttl_ms)?;
            print_or_json(json, &claim, || {
                format!("Claimed {} token={}", claim.task.id, claim.claim_token)
            })?;
        }
        TaskCommand::Heartbeat(args) => {
            print_task(
                json,
                &heartbeat_task(
                    db_path,
                    board,
                    actor,
                    &args.task_ref,
                    &args.claim_token,
                    args.ttl_ms,
                )?,
            )?;
        }
        TaskCommand::Done(args) | TaskCommand::Complete(args) => print_task(
            json,
            &complete_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Review(args) => print_task(
            json,
            &submit_review_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Block(args) => print_task(
            json,
            &block_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.reason,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Unblock { task_ref } => {
            print_task(json, &unblock_task(db_path, board, actor, &task_ref)?)?
        }
        TaskCommand::Reclaim(args) => {
            let _expired_only = args.expired;
            let count = reclaim_expired(db_path, board, actor)?;
            print_or_json(json, &serde_json::json!({"reclaimed": count}), || {
                format!("Reclaimed {count} task(s)")
            })?;
        }
        TaskCommand::Archive { task_ref, force } => print_task(
            json,
            &archive_task(db_path, board, actor, &task_ref, force)?,
        )?,
        TaskCommand::Step { command } => handle_task_step(command, db_path, board, actor, json)?,
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct TaskStepsOutput {
    task_id: String,
    execution_plan: TaskExecutionPlanRecord,
    steps: Vec<TaskStepRecord>,
}

fn handle_task_step(
    command: TaskStepCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        TaskStepCommand::List { task_ref } => {
            let output = task_steps_output(db_path, board, &task_ref)?;
            print_or_json(json, &output, || task_steps_lines(&output))?;
        }
        TaskStepCommand::Add(args) => {
            let required = step_required_for_add(args.required, args.optional)?;
            let body =
                resolve_optional_text_input(args.body, args.body_file, "--body", "--body-file")?;
            let step = create_step(
                db_path,
                board,
                actor,
                &args.task_ref,
                CreateStepInput {
                    title: args.title,
                    body,
                    linked_task_ref: args.linked_task_ref,
                    position: args.position,
                    required,
                },
            )?;
            print_step(json, &step, "Created")?;
        }
        TaskStepCommand::Update(args) => {
            if args.linked_task_ref.is_some() && args.unlink_task {
                return Err(invalid_input(
                    "--link-task and --unlink-task are mutually exclusive",
                ));
            }
            if args.clear_body && args.body_file.is_some() {
                return Err(invalid_input(
                    "--body-file and --clear-body are mutually exclusive",
                ));
            }
            let body =
                resolve_optional_text_input(args.body, args.body_file, "--body", "--body-file")?;
            if body.is_some() && args.clear_body {
                return Err(invalid_input(
                    "--body and --clear-body are mutually exclusive",
                ));
            }
            let step = update_step(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.step_ref,
                UpdateStepInput {
                    title: args.title,
                    body: if args.clear_body {
                        Some(None)
                    } else {
                        body.map(Some)
                    },
                    linked_task_ref: args.linked_task_ref,
                    unlink_task: args.unlink_task,
                    position: args.position,
                    required: step_required_for_update(args.required, args.optional)?,
                },
            )?;
            print_step(json, &step, "Updated")?;
        }
        TaskStepCommand::Done(args) => {
            let step = complete_step(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.step_ref,
                &args.note,
            )?;
            print_step(json, &step, "Completed")?;
        }
        TaskStepCommand::Skip(args) => {
            let step = skip_step(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.step_ref,
                &args.reason,
            )?;
            print_step(json, &step, "Skipped")?;
        }
        TaskStepCommand::Reopen(args) => {
            let step = reopen_step(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.step_ref,
                &args.reason,
            )?;
            print_step(json, &step, "Reopened")?;
        }
        TaskStepCommand::Remove { task_ref, step_ref } => {
            let step = remove_step(db_path, board, actor, &task_ref, &step_ref)?;
            print_or_json(
                json,
                &serde_json::json!({"removed": true, "step": step}),
                || format!("Removed step relation {step_ref} from {task_ref}"),
            )?;
        }
        TaskStepCommand::NotRequired(args) => {
            let plan = mark_execution_plan_not_required(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.reason,
            )?;
            print_or_json(json, &plan, || execution_plan_line(&plan))?;
        }
    }
    Ok(())
}

fn task_steps_output(db_path: &PathBuf, board: &str, task_ref: &str) -> Result<TaskStepsOutput> {
    let execution_plan = execution_plan(db_path, board, task_ref)?;
    let task_id = execution_plan.task_id.clone();
    let steps = list_steps(db_path, board, task_ref)?;
    Ok(TaskStepsOutput {
        task_id,
        execution_plan,
        steps,
    })
}

fn print_step(json: bool, step: &TaskStepRecord, verb: &str) -> Result<()> {
    print_or_json(json, step, || format!("{verb} {}", step_line(1, step)))
}

fn task_steps_lines(output: &TaskStepsOutput) -> String {
    let required_total = output.steps.iter().filter(|step| step.required).count();
    let required_done = output
        .steps
        .iter()
        .filter(|step| {
            step.required
                && matches!(
                    step.status,
                    kanban_sqlite::StepStatus::Done | kanban_sqlite::StepStatus::Skipped
                )
        })
        .count();
    let optional_total = output.steps.iter().filter(|step| !step.required).count();
    let mut lines = vec![execution_plan_line(&output.execution_plan)];
    lines.push(format!(
        "Required steps: {required_done}/{required_total} done-or-skipped"
    ));
    lines.push(format!("Optional steps: {optional_total}"));
    if !output.steps.is_empty() {
        lines.push(String::new());
        lines.extend(
            output
                .steps
                .iter()
                .enumerate()
                .map(|(index, step)| step_line(index + 1, step)),
        );
    }
    lines.join("\n")
}

fn execution_plan_line(plan: &TaskExecutionPlanRecord) -> String {
    match plan.reason.as_deref() {
        Some(reason) if !reason.is_empty() => {
            format!("Execution plan: {} reason={reason}", plan.state.as_str())
        }
        _ => format!("Execution plan: {}", plan.state.as_str()),
    }
}

fn step_line(index: usize, step: &TaskStepRecord) -> String {
    let required = if step.required {
        "required"
    } else {
        "optional"
    };
    let linked = step
        .linked_task
        .as_ref()
        .map(|task| format!(" link={}", task.task_ref))
        .unwrap_or_default();
    format!(
        "S{index} {} [{}] {required} pos={}{} {}",
        step.id,
        step.status.as_str(),
        step.position,
        linked,
        step.title
    )
}

fn task_plan_filters(args: &ListArgs) -> Result<Vec<TaskPlanFilter>> {
    let mut filters = Vec::new();
    for filter in &args.plan_filters {
        push_task_plan_filter(
            &mut filters,
            match filter {
                TaskPlanFilterArg::PlanNeeded => TaskPlanFilter::PlanNeeded,
                TaskPlanFilterArg::HasSteps => TaskPlanFilter::HasSteps,
                TaskPlanFilterArg::IncompleteRequiredSteps => {
                    TaskPlanFilter::IncompleteRequiredSteps
                }
            },
        );
    }
    if args.plan_needed {
        push_task_plan_filter(&mut filters, TaskPlanFilter::PlanNeeded);
    }
    if args.has_steps {
        push_task_plan_filter(&mut filters, TaskPlanFilter::HasSteps);
    }
    if args.incomplete_required_steps {
        push_task_plan_filter(&mut filters, TaskPlanFilter::IncompleteRequiredSteps);
    }
    Ok(filters)
}

fn push_task_plan_filter(filters: &mut Vec<TaskPlanFilter>, filter: TaskPlanFilter) {
    if !filters.contains(&filter) {
        filters.push(filter);
    }
}

fn step_required_for_add(required: bool, optional: bool) -> Result<bool> {
    if required && optional {
        return Err(invalid_input(
            "--required and --optional are mutually exclusive",
        ));
    }
    Ok(!optional)
}

fn step_required_for_update(required: bool, optional: bool) -> Result<Option<bool>> {
    if required && optional {
        return Err(invalid_input(
            "--required and --optional are mutually exclusive",
        ));
    }
    Ok(if required {
        Some(true)
    } else if optional {
        Some(false)
    } else {
        None
    })
}
