use kanban_core::{Clock, KanbanError, Result, new_board_id, new_event_id};

use crate::{BoardRecord, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoardCommand {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub actor: String,
    pub event_id: String,
    pub created_at: i64,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn create_board(&self, command: CreateBoardCommand) -> Result<BoardRecord> {
        let slug = command.slug.trim().to_owned();
        let name = command.name.trim().to_owned();
        let description = command
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let actor = command.actor.trim().to_owned();
        if slug.is_empty() {
            return Err(KanbanError::InvalidInput("看板 slug 不能为空".to_owned()));
        }
        if name.is_empty() {
            return Err(KanbanError::InvalidInput("看板名称不能为空".to_owned()));
        }
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("操作人不能为空".to_owned()));
        }

        let _mutation = self.mutation_gate.lock().await;
        self.store
            .create_board(crate::CreateBoardInput {
                id: new_board_id(),
                slug,
                name,
                description,
                actor,
                event_id: new_event_id(),
                created_at: self.clock.now_ms(),
            })
            .await
            .map(super::application_board)
            .map_err(crate::error::store_error)
    }
}
