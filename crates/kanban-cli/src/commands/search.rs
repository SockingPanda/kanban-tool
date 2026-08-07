use clap::{Args, ValueEnum};
use kanban_protocol::cli_helpers::{CliSearchData, CliSearchOutput};
use kanban_protocol::{ApiTaskStatus, SearchTasksQuery};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    /// 搜索文本或精确任务 reference，例如 `default#12`。
    pub(crate) query: String,
    #[arg(long, value_enum)]
    pub(crate) status: Vec<SearchStatus>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum SearchStatus {
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

pub(crate) fn run(ctx: &CliContext, args: &SearchArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let query = SearchTasksQuery {
        board: ctx.board.clone(),
        q: Some(args.query.clone()),
        status: args.status.iter().copied().map(Into::into).collect(),
        label: args.labels.clone(),
        include_archived: args.include_archived,
        limit: args.limit,
        offset: args.offset,
        assignee: args.assignee.clone(),
    };
    let response = client.search_tasks(&query)?;
    let data = response.data;
    if ctx.json {
        output::print_json(&CliSearchOutput::new(
            CliSearchData { hits: data.hits },
            data.meta,
        ));
    } else {
        for hit in data.hits {
            let snippet = hit.snippet.as_deref().unwrap_or("");
            println!(
                "{} {} {} [{:.2}] {}",
                hit.task.task_ref,
                hit.task.status.as_str(),
                hit.task.title,
                hit.score,
                snippet
            );
        }
    }
    Ok(())
}

impl From<SearchStatus> for ApiTaskStatus {
    fn from(value: SearchStatus) -> Self {
        match value {
            SearchStatus::Triage => Self::Triage,
            SearchStatus::Todo => Self::Todo,
            SearchStatus::Scheduled => Self::Scheduled,
            SearchStatus::Ready => Self::Ready,
            SearchStatus::Running => Self::Running,
            SearchStatus::Blocked => Self::Blocked,
            SearchStatus::Review => Self::Review,
            SearchStatus::Done => Self::Done,
            SearchStatus::Archived => Self::Archived,
        }
    }
}
