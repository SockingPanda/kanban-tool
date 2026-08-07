use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{BoardRecord, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveBoardCommand {
    pub board: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveBoardRecord {
    pub actor: String,
    pub event_id: String,
    pub archived_at: i64,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn archive_board(&self, command: ArchiveBoardCommand) -> Result<BoardRecord> {
        let board = command.board.trim().to_owned();
        let actor = command.actor.trim().to_owned();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("看板不能为空".to_owned()));
        }
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("操作人不能为空".to_owned()));
        }

        let _mutation = self.mutation_gate.lock().await;
        self.store
            .archive_board(
                &board,
                crate::ArchiveBoardInput {
                    actor,
                    event_id: new_event_id(),
                    archived_at: self.clock.now_ms(),
                },
            )
            .await
            .map(super::application_board)
            .map_err(crate::error::store_error)
    }
}
