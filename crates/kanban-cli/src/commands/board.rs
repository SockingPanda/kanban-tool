use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{
    BoardListOptions, CreateBoard, archive_board, create_board, get_board, list_boards,
};

use crate::args::BoardCommand;
use crate::commands::common::write_board_config;
use crate::output::print_or_json;

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
            print_or_json(json, &serde_json::json!({ "board": board.slug }), || {
                format!("Current board: {}", board.slug)
            })?;
        }
        BoardCommand::Current => {
            print_or_json(json, &serde_json::json!({ "board": active_board }), || {
                active_board.to_owned()
            })?;
        }
        BoardCommand::Archive { board } => {
            let board = archive_board(db_path, &board, actor)?;
            print_or_json(json, &board, || format!("Archived board {}", board.slug))?;
        }
    }
    Ok(())
}
