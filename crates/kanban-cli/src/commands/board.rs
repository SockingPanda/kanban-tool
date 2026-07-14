use std::path::PathBuf;

use anyhow::Result;
use kanban_contract::{
    ApiBoard, ArchiveBoardResponse, CliActiveBoard, CliActiveBoardOutput, CreateBoardResponse,
    GetBoardResponse, ListBoardsResponse,
};
use kanban_sqlite::api::{
    BoardListOptions, BoardRecord, CreateBoard, archive_board, create_board, get_board, list_boards,
};

use crate::args::BoardCommand;
use crate::commands::common::write_board_config;
use crate::output::print_contract_or_human;

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
            let output = ListBoardsResponse {
                data: boards.into_iter().map(api_board).collect(),
            };
            print_contract_or_human(json, &output, || {
                output
                    .data
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
            let output = CreateBoardResponse {
                data: api_board(board),
            };
            print_contract_or_human(json, &output, || {
                format!("Created board {} {}", output.data.slug, output.data.name)
            })?;
        }
        BoardCommand::Show { board } => {
            let output = GetBoardResponse {
                data: api_board(get_board(db_path, &board)?),
            };
            print_contract_or_human(json, &output, || {
                format!("{} {}", output.data.slug, output.data.name)
            })?;
        }
        BoardCommand::Use { board } => {
            let board = get_board(db_path, &board)?;
            write_board_config(&board.slug)?;
            let output = CliActiveBoardOutput {
                data: CliActiveBoard {
                    board: api_board(board),
                },
            };
            print_contract_or_human(json, &output, || {
                format!("Current board: {}", output.data.board.slug)
            })?;
        }
        BoardCommand::Current => {
            let output = CliActiveBoardOutput {
                data: CliActiveBoard {
                    board: api_board(get_board(db_path, active_board)?),
                },
            };
            print_contract_or_human(json, &output, || output.data.board.slug.clone())?;
        }
        BoardCommand::Archive { board } => {
            let output = ArchiveBoardResponse {
                data: api_board(archive_board(db_path, &board, actor)?),
            };
            print_contract_or_human(json, &output, || {
                format!("Archived board {}", output.data.slug)
            })?;
        }
    }
    Ok(())
}

fn api_board(board: BoardRecord) -> ApiBoard {
    ApiBoard {
        id: board.id,
        slug: board.slug,
        name: board.name,
        description: board.description,
        created_at: board.created_at,
        updated_at: board.updated_at,
        archived_at: board.archived_at,
    }
}
