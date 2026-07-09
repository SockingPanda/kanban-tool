use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::api::{
    BoardListOptions, CreateBoard, archive_board, create_board, get_board, list_boards,
};
use serde::Serialize;

use crate::args::BoardCommand;
use crate::commands::common::write_board_config;
use crate::output::print_or_json;

#[derive(Debug, Serialize)]
struct ActiveBoardOutput {
    board: kanban_sqlite::api::BoardRecord,
}

pub(crate) fn handle_board(
    command: BoardCommand,
    db_path: &PathBuf,
    active_board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        BoardCommand::List { include_archived } => {
            let boards = list_boards(db_path, BoardListOptions { include_archived })?;
            print_or_json(json, &boards, || {
                boards
                    .iter()
                    .map(|board| {
                        let archived = if board.archived_at.is_some() {
                            " archived"
                        } else {
                            ""
                        };
                        format!("{} {}{}", board.slug, board.name, archived)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        BoardCommand::Create(args) => {
            let board = create_board(
                db_path,
                actor,
                CreateBoard {
                    slug: args.slug,
                    name: args.name,
                    description: args.description,
                },
            )?;
            print_or_json(json, &board, || {
                format!("Created board {} {}", board.slug, board.name)
            })?;
        }
        BoardCommand::Show { board } => {
            let board = get_board(db_path, &board)?;
            print_or_json(json, &board, || format!("{} {}", board.slug, board.name))?;
        }
        BoardCommand::Use { board } => {
            let board = get_board(db_path, &board)?;
            write_board_config(&board.slug)?;
            let output = ActiveBoardOutput { board };
            print_or_json(json, &output, || {
                format!("Current board: {}", output.board.slug)
            })?;
        }
        BoardCommand::Current => {
            let board = get_board(db_path, active_board)?;
            let output = ActiveBoardOutput { board };
            print_or_json(json, &output, || output.board.slug.clone())?;
        }
        BoardCommand::Archive { board } => {
            let board = archive_board(db_path, &board, actor)?;
            print_or_json(json, &board, || format!("Archived board {}", board.slug))?;
        }
    }
    Ok(())
}
