use kanban_service::{
    EventList, EventListOptions as ApplicationEventListOptions,
    EventListPage as ApplicationEventListPage, EventRecord as ApplicationEventRecord,
};
use kanban_core::Result;
use kanban_store_turso::TaskEventRecord as StoreEventRecord;

use crate::adapter::{TursoApplicationStore, store_error};

impl EventList for TursoApplicationStore {
    async fn list_events(
        &self,
        board: &str,
        options: ApplicationEventListOptions,
    ) -> Result<ApplicationEventListPage> {
        let page = self
            .store
            .list_events(
                board,
                options.task_id.as_deref(),
                options.after,
                options.limit,
            )
            .await
            .map_err(store_error)?;
        Ok(ApplicationEventListPage {
            events: page.events.into_iter().map(application_event).collect(),
            next_after: page.next_after,
        })
    }
}

fn application_event(event: StoreEventRecord) -> ApplicationEventRecord {
    ApplicationEventRecord {
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
