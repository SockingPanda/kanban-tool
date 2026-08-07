use kanban_protocol::{ListEventsQuery, ListEventsResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_events(&self, query: &ListEventsQuery) -> Result<ListEventsResponse, ClientError> {
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

        self.get(&list_events_path(board, task_id, query.after, query.limit))
    }
}

fn list_events_path(board: &str, task_id: Option<&str>, after: i64, limit: usize) -> String {
    let mut pairs = vec![format!("board={}", encode_path_segment(board))];
    if let Some(task_id) = task_id {
        pairs.push(format!("task_id={}", encode_path_segment(task_id)));
    }
    pairs.push(format!("after={}", encode_path_segment(&after.to_string())));
    pairs.push(format!("limit={}", encode_path_segment(&limit.to_string())));
    format!("/api/v1/events?{}", pairs.join("&"))
}

#[cfg(test)]
mod tests {
    use kanban_protocol::ListEventsQuery;

    use super::*;

    #[test]
    fn list_events_path_has_deterministic_order_and_encoding() {
        assert_eq!(
            list_events_path("team/one #", Some("t_event/1"), 7, 0),
            "/api/v1/events?board=team%2Fone%20%23&task_id=t_event%2F1&after=7&limit=0"
        );
    }

    #[test]
    fn list_events_rejects_empty_board_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "   ".into(),
                task_id: None,
                after: 0,
                limit: 10,
            })
            .expect_err("empty board must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn list_events_rejects_negative_after_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "default".into(),
                task_id: None,
                after: -1,
                limit: 10,
            })
            .expect_err("negative cursors must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn list_events_rejects_non_global_task_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_events(&ListEventsQuery {
                board: "default".into(),
                task_id: Some("default#1".into()),
                after: 0,
                limit: 10,
            })
            .expect_err("board-local selectors must be rejected before HTTP");
        assert_eq!(error.code(), "invalid_input");
    }
}
