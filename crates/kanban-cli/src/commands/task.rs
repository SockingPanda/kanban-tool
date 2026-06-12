use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{
    CreateTask, MAX_TASK_LIST_LIMIT, TaskListOptions, TaskListSort, TaskPatch, archive_task,
    block_task, claim_task, complete_task, create_task, get_task, heartbeat_task, list_tasks,
    list_tasks_page, promote_task, reclaim_expired, submit_review_task, unblock_task, update_task,
};

use crate::args::TaskCommand;
use crate::commands::common::{
    optional_clearable, parse_status, parse_task_list_sort, validate_page_bounds,
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
            let task = create_task(
                db_path,
                board,
                actor,
                CreateTask {
                    title: args.title,
                    description: args.description,
                    status: args.status.as_deref().map(parse_status).transpose()?,
                    assignee: args.assignee,
                    priority: args.priority,
                    scheduled_at: args.scheduled_at,
                    due_at: args.due_at,
                    max_retries: args.max_retries,
                    metadata_json: args.metadata,
                },
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
            let uses_page_options = args.search.is_some()
                || args.assignee.is_some()
                || args.limit.is_some()
                || args.offset.is_some()
                || args.sort.is_some();
            let tasks = if uses_page_options {
                list_tasks_page(
                    db_path,
                    board,
                    TaskListOptions {
                        statuses,
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
            print_task_with_details(json, details, &get_task(db_path, board, &task_ref)?)?
        }
        TaskCommand::Update(args) => {
            let task = update_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                TaskPatch {
                    title: args.title,
                    description: args.description.map(Some),
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
                    metadata_json: args.metadata,
                    expected_lock_version: args.expected_lock_version,
                },
            )?;
            print_task(json, &task)?;
        }
        TaskCommand::Promote { task_ref } => {
            print_task(json, &promote_task(db_path, board, actor, &task_ref)?)?
        }
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
    }
    Ok(())
}
