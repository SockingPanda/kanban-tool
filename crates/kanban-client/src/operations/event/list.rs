use kanban_protocol::{ListEventsQuery, ListEventsResponse};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_events(
        &self,
        query: &ListEventsQuery,
    ) -> Result<ListEventsResponse, ClientError> {
        let board = query.board.trim();
        if board.is_empty() {
            return Err(ClientError::InvalidInput("必须提供 board".to_owned()));
        }
        if query.after < 0 {
            return Err(ClientError::InvalidInput("after 必须是非负数".to_owned()));
        }

        let task_id = query.task_id.as_deref().map(str::trim);
        if task_id.is_some_and(|task_id| !task_id.starts_with("t_") || task_id.len() <= 2) {
            return Err(ClientError::InvalidInput(
                "task_id 必须是全局 t_... ID".to_owned(),
            ));
        }

        let response: kanban_protocol::ListEventsResponse = rpc!(
            self,
            list_events,
            ListEventsRequest,
            (),
            kanban_protocol::ListEventsQuery {
                board: board.to_owned(),
                task_id: task_id.map(str::to_owned),
                after: query.after,
                limit: query.limit
            },
            ()
        )?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use kanban_protocol::ListEventsQuery;

    use super::*;

    #[tokio::test]
    async fn list_events_rejects_empty_board_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "   ".into(),
                task_id: None,
                after: 0,
                limit: 10,
            })
            .await
            .expect_err("empty board must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }

    #[tokio::test]
    async fn list_events_rejects_negative_after_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "default".into(),
                task_id: None,
                after: -1,
                limit: 10,
            })
            .await
            .expect_err("negative cursors must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }

    #[tokio::test]
    async fn list_events_rejects_non_global_task_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "default".into(),
                task_id: Some("default#1".into()),
                after: 0,
                limit: 10,
            })
            .await
            .expect_err("board-local selectors must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }
}
