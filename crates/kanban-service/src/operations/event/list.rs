use kanban_core::{Clock, KanbanError, Result};

use crate::KanbanService;

pub const MAX_EVENT_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: i64,
    pub event_id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListOptions {
    pub task_id: Option<String>,
    pub after: i64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListPage {
    pub events: Vec<EventRecord>,
    pub next_after: i64,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_events(
        &self,
        board: &str,
        mut options: EventListOptions,
    ) -> Result<EventListPage> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        if options.after < 0 {
            return Err(KanbanError::InvalidInput(
                "after must be non-negative".to_owned(),
            ));
        }
        options.task_id = options.task_id.map(|task_id| task_id.trim().to_owned());
        if let Some(task_id) = options.task_id.as_deref()
            && (!task_id.starts_with("t_") || task_id.len() <= 2)
        {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        options.limit = options.limit.min(MAX_EVENT_LIST_LIMIT);
        let page = self
            .store
            .list_events(
                board,
                options.task_id.as_deref(),
                options.after,
                options.limit,
            )
            .await
            .map_err(crate::error::store_error)?;
        Ok(EventListPage {
            events: page.events.into_iter().map(application_event).collect(),
            next_after: page.next_after,
        })
    }
}

fn application_event(event: crate::domain::TaskEventRecord) -> EventRecord {
    EventRecord {
        id: event.id,
        event_id: event.event_id,
        board_id: event.board_id,
        task_id: event.task_id,
        run_id: event.run_id,
        kind: event.kind,
        actor: event.actor,
        payload_json: event.payload_json,
        created_at: event.created_at,
    }
}
