use anyhow::{Context, Result};
use clap::Parser;
use kanban_sqlite::{begin_database_runtime, dispatch_once, init_database, list_events, list_runs};

use crate::args::*;
use crate::commands::{
    board::handle_board,
    comment::handle_comment,
    common::{active_board, default_actor, default_db_path},
    dep::handle_dep,
    dispatch::{dispatch_loop, dispatch_options},
    index::handle_index,
    maintenance::{
        handle_backup, handle_checkpoint, handle_doctor, handle_export, handle_import,
        handle_stats, handle_vacuum,
    },
    run::handle_run,
    search::handle_search,
    serve::serve,
    substrate::{
        handle_context, handle_derived, handle_entity, handle_graph, handle_outbox, handle_vector,
    },
    task::handle_task,
};
use crate::output::print_or_json;

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.clone().unwrap_or_else(default_db_path);
    let actor = cli.actor.clone().unwrap_or_else(default_actor);
    let board = active_board(cli.board.as_deref())?;
    match cli.command {
        Command::Init { force: _ } => {
            let result = init_database(&db_path, &actor)
                .with_context(|| format!("failed to initialize {}", db_path.display()))?;
            print_or_json(cli.json, &result, || {
                format!(
                    "Initialized Kanban database at {}\nDefault board: {}",
                    result.db_path.display(),
                    result.board_slug
                )
            })?;
        }
        Command::Board { command } => handle_board(command, &db_path, &board, &actor, cli.json)?,
        Command::Task { command } => handle_task(command, &db_path, &board, &actor, cli.json)?,
        Command::Comment { command } => {
            handle_comment(command, &db_path, &board, &actor, cli.json)?
        }
        Command::Dep { command } => handle_dep(command, &db_path, &board, &actor, cli.json)?,
        Command::Events { task_ref } => {
            let events = list_events(&db_path, &board, task_ref.as_deref())?;
            print_or_json(cli.json, &events, || {
                events
                    .iter()
                    .map(|e| format!("{} {} {:?}", e.id, e.kind, e.task_id))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Runs { task_ref } => {
            let runs = list_runs(&db_path, &board, task_ref.as_deref())?;
            print_or_json(cli.json, &runs, || {
                runs.iter()
                    .map(|r| {
                        format!(
                            "{} [{}] task={} exit={:?}",
                            r.id, r.status, r.task_id, r.exit_code
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Run { command } => handle_run(command, &db_path, cli.json)?,
        Command::Search(args) => handle_search(args, &db_path, &board, cli.json)?,
        Command::Index { command } => handle_index(command, &db_path, &board, cli.json)?,
        Command::Entity { command } => handle_entity(command, &db_path, cli.json)?,
        Command::Outbox { command } => handle_outbox(command, &db_path, cli.json)?,
        Command::Derived { command } => handle_derived(command, &db_path, cli.json)?,
        Command::Graph { command } => handle_graph(command, &db_path, &board, cli.json)?,
        Command::Vector { command } => handle_vector(command, &db_path, &board, cli.json)?,
        Command::Context { command } => handle_context(command, &db_path, &board, cli.json)?,
        Command::Dispatch(args) => {
            let options = dispatch_options(&args, actor.clone())?;
            if args.once {
                let result = dispatch_once(&db_path, &board, options)?;
                print_or_json(cli.json, &result, || {
                    format!(
                        "claimed={} task={:?} exit={:?}",
                        result.claimed, result.task_id, result.exit_code
                    )
                })?;
            } else {
                let _runtime_guard = begin_database_runtime(&db_path)?;
                let summary = dispatch_loop(
                    &db_path,
                    &board,
                    options,
                    args.poll_interval_ms,
                    args.max_iterations,
                )?;
                print_or_json(cli.json, &summary, || {
                    format!(
                        "iterations={} claimed={}",
                        summary.iterations, summary.claimed
                    )
                })?;
            }
        }
        Command::Serve(args) => serve(args, db_path, &board, actor)?,
        Command::Doctor => handle_doctor(&db_path, cli.json)?,
        Command::Stats => handle_stats(&db_path, &board, cli.json)?,
        Command::Backup(args) => handle_backup(&db_path, args, cli.json)?,
        Command::Export(args) => handle_export(&db_path, &board, args, cli.json)?,
        Command::Import(args) => handle_import(&db_path, &actor, args, cli.json)?,
        Command::Checkpoint => handle_checkpoint(&db_path, cli.json)?,
        Command::Vacuum => handle_vacuum(&db_path, cli.json)?,
    }
    Ok(())
}
