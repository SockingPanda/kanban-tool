use std::path::PathBuf;

use anyhow::Result;
use kanban_search::SearchQuery;
use kanban_sqlite::{MAX_SEARCH_LIMIT, search_tasks};

use crate::args::{SearchArgs, SearchOutput, SearchOutputHit};
use crate::commands::common::validate_page_bounds;
use crate::output::{print_or_json_with_meta, search_hit_line};

pub(crate) fn handle_search(
    args: SearchArgs,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    validate_page_bounds(args.limit, MAX_SEARCH_LIMIT, args.offset)?;
    let statuses = args.status.into_iter().map(Into::into).collect::<Vec<_>>();
    let labels = args
        .labels
        .iter()
        .map(|label| label.trim().to_owned())
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    let results = search_tasks(
        db_path,
        SearchQuery {
            board: board.to_owned(),
            q: Some(args.query),
            statuses,
            labels,
            assignee: args.assignee,
            include_archived: args.include_archived,
            limit: args.limit,
            offset: args.offset,
        },
    )?;
    let hits = results
        .hits
        .into_iter()
        .map(|hit| {
            let task = kanban_sqlite::get_task_by_id_global(db_path, &hit.task_id)?;
            Ok(SearchOutputHit {
                task_id: hit.task_id,
                seq: hit.seq,
                score: hit.score,
                snippet: hit.snippet,
                task,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let output = SearchOutput { hits };
    print_or_json_with_meta(json, &output, &results.meta, || {
        output
            .hits
            .iter()
            .map(search_hit_line)
            .collect::<Vec<_>>()
            .join("\n")
    })
}
