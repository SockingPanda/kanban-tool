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
    let search_limit = if args.labels.is_empty() {
        args.limit
    } else {
        MAX_SEARCH_LIMIT
    };
    let search_offset = if args.labels.is_empty() {
        args.offset
    } else {
        0
    };
    let results = search_tasks(
        db_path,
        SearchQuery {
            board: board.to_owned(),
            q: Some(args.query),
            statuses,
            assignee: args.assignee,
            include_archived: args.include_archived,
            limit: search_limit,
            offset: search_offset,
        },
    )?;
    let labels = args
        .labels
        .iter()
        .map(|label| label.trim().to_owned())
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    let mut hits = results
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
    if !labels.is_empty() {
        hits.retain(|hit| {
            labels.iter().all(|label| {
                hit.task
                    .labels
                    .iter()
                    .any(|task_label| task_label.name == *label || task_label.id == *label)
            })
        });
        hits = hits
            .into_iter()
            .skip(args.offset)
            .take(args.limit)
            .collect();
    }
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
