use std::path::PathBuf;

use anyhow::Result;
use kanban_search::SearchQuery;
use kanban_sqlite::{MAX_SEARCH_LIMIT, search_tasks};

use crate::args::{SearchArgs, SearchOutput, SearchOutputHit};
use crate::commands::common::{parse_status, validate_page_bounds};
use crate::output::{print_or_json, search_hit_line};

pub(crate) fn handle_search(
    args: SearchArgs,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    validate_page_bounds(args.limit, MAX_SEARCH_LIMIT, args.offset)?;
    let statuses = args
        .status
        .iter()
        .map(|status| parse_status(status))
        .collect::<Result<Vec<_>>>()?;
    let results = search_tasks(
        db_path,
        SearchQuery {
            board: board.to_owned(),
            q: Some(args.query),
            statuses,
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
    let output = SearchOutput {
        hits,
        meta: results.meta,
    };
    print_or_json(json, &output, || {
        output
            .hits
            .iter()
            .map(search_hit_line)
            .collect::<Vec<_>>()
            .join("\n")
    })
}
