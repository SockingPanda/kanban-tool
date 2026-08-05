use std::str::FromStr;

use clap::{Args, ValueEnum};
use kanban_client::KanbanClient;
use kanban_contract::{
    ApiTaskPriority, ApiTaskStatus, ListTasksQuery, TaskReadLabel, TaskReadPlanFilter, TaskReadSort,
};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long, value_enum)]
    pub(crate) status: Vec<ListStatus>,
    #[arg(long)]
    pub(crate) priority: Vec<i64>,
    #[arg(long = "label")]
    pub(crate) label: Vec<String>,
    #[arg(long = "plan-filter")]
    pub(crate) plan_filter: Vec<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long = "query", alias = "search")]
    pub(crate) query: Option<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
    #[arg(long, default_value = "position")]
    pub(crate) sort: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ListStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ListArgs,
) -> Result<(), CliFailure> {
    let query = list_tasks_query(args)?;
    let response = client.list_tasks(&ctx.board, &query)?;
    if ctx.json {
        output::print_json(&kanban_contract::CliTaskListOutput::new(response.data));
    } else {
        for task in response.data {
            println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
        }
    }
    Ok(())
}

fn list_tasks_query(args: &ListArgs) -> Result<ListTasksQuery, CliFailure> {
    let priorities = args
        .priority
        .iter()
        .copied()
        .map(|value| {
            ApiTaskPriority::try_from(value).map_err(|value| CliFailure {
                code: "invalid_input",
                message: format!("priority must be between 0 and 3, got {value}"),
                exit_code: 2,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan_filter = args
        .plan_filter
        .iter()
        .map(|value| {
            TaskReadPlanFilter::from_str(value).map_err(|()| CliFailure {
                code: "invalid_input",
                message: format!("unsupported --plan-filter: {value}"),
                exit_code: 2,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sort = TaskReadSort::from_str(&args.sort).map_err(|()| CliFailure {
        code: "invalid_input",
        message: format!("unsupported --sort: {}", args.sort),
        exit_code: 2,
    })?;
    Ok(ListTasksQuery {
        status: args.status.iter().copied().map(api_list_status).collect(),
        priority: priorities,
        label: args
            .label
            .iter()
            .map(|value| {
                TaskReadLabel::new(value.clone()).ok_or_else(|| CliFailure {
                    code: "invalid_input",
                    message: format!("invalid --label: {value}"),
                    exit_code: 2,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        plan_filter,
        assignee: args.assignee.clone(),
        q: args.query.clone(),
        include_archived: args.include_archived,
        limit: args.limit,
        offset: args.offset,
        sort,
    })
}

fn api_list_status(status: ListStatus) -> ApiTaskStatus {
    match status {
        ListStatus::Triage => ApiTaskStatus::Triage,
        ListStatus::Todo => ApiTaskStatus::Todo,
        ListStatus::Scheduled => ApiTaskStatus::Scheduled,
        ListStatus::Ready => ApiTaskStatus::Ready,
        ListStatus::Running => ApiTaskStatus::Running,
        ListStatus::Blocked => ApiTaskStatus::Blocked,
        ListStatus::Review => ApiTaskStatus::Review,
        ListStatus::Done => ApiTaskStatus::Done,
        ListStatus::Archived => ApiTaskStatus::Archived,
    }
}
