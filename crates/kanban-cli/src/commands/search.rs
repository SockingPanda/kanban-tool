use std::path::PathBuf;

use anyhow::Result;
use kanban_contract::{
    SearchMeta, SearchTaskHit,
    cli_helpers::{CliSearchData, CliSearchOutput},
};
use kanban_search::SearchQuery;
use kanban_sqlite::api::{MAX_SEARCH_LIMIT, search_tasks};

use crate::args::{SearchArgs, SearchOutput, SearchOutputHit};
use crate::commands::common::validate_page_bounds;
use crate::output::{api_task_from_record, print_contract_or_human, print_human, search_hit_line};

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
            let task = kanban_sqlite::api::get_task_by_id_global(db_path, &hit.task_id)?;
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
    if !json {
        return print_human(|| {
            output
                .hits
                .iter()
                .map(search_hit_line)
                .collect::<Vec<_>>()
                .join("\n")
        });
    }
    let output = CliSearchOutput::new(
        CliSearchData {
            hits: output
                .hits
                .iter()
                .map(|hit| {
                    Ok(SearchTaskHit {
                        task_id: hit.task_id.clone(),
                        seq: hit.seq,
                        score: hit.score,
                        snippet: hit.snippet.clone(),
                        task: api_task_from_record(&hit.task)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        },
        SearchMeta {
            backend: results.meta.backend,
            stale: results.meta.stale,
            database_instance_id: results.meta.database_instance_id,
            protocol_version: results.meta.protocol_version,
            generation: results.meta.generation,
            resolved_board_id: results.meta.resolved_board_id,
            fallback_reason: results.meta.fallback_reason,
            index_version: results.meta.index_version,
            last_event_id: results.meta.last_event_id,
            index_lag_events: results.meta.index_lag_events,
        },
    );
    print_contract_or_human(json, &output, String::new)
}
