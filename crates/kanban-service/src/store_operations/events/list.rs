use crate::store_operations::shared::validate_task_id;
use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

const EVENT_LIST_LIMIT_CAP: usize = 1_000;

impl TursoStore {
    pub async fn list_events(
        &self,
        board_selector: &str,
        task_id: Option<&str>,
        after: i64,
        limit: usize,
    ) -> Result<TaskEventListPage, StoreError> {
        if after < 0 {
            return Err(StoreError::InvalidInput(
                "after must be non-negative".to_owned(),
            ));
        }
        let board_selector = board_selector.trim();
        if board_selector.is_empty() {
            return Err(StoreError::InvalidInput("board is required".to_owned()));
        }

        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id FROM boards WHERE id = :board OR slug = :board LIMIT 1",
                    [(":board", board_selector)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;

        let task_filter = task_id.map(str::trim).map(str::to_owned);
        if let Some(task_id) = task_filter.as_deref() {
            validate_task_id(task_id)?;
            let task = first_row(
                connection
                    .query(
                        "SELECT board_id FROM tasks WHERE id = :task_id LIMIT 1",
                        [(":task_id", task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
                other => StoreError::Turso(other),
            })?;
            let task_board_id = text_value(task.get_value(0)?, "tasks.board_id")?;
            if task_board_id != board_id {
                return Err(StoreError::TaskNotFound(task_id.to_owned()));
            }
        }

        if limit == 0 {
            return Ok(TaskEventListPage {
                events: Vec::new(),
                next_after: after,
            });
        }

        let effective_limit = limit.min(EVENT_LIST_LIMIT_CAP);
        let effective_limit = i64::try_from(effective_limit)
            .map_err(|_| StoreError::InvalidInput("limit is too large".to_owned()))?;
        let mut params = vec![
            (":board_id".to_owned(), Value::Text(board_id)),
            (":after".to_owned(), Value::Integer(after)),
        ];
        let mut sql = "SELECT id, event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE board_id = :board_id AND id > :after".to_owned();
        if let Some(task_id) = task_filter {
            sql.push_str(" AND task_id = :task_id");
            params.push((":task_id".to_owned(), Value::Text(task_id)));
        }
        sql.push_str(" ORDER BY id ASC LIMIT :limit");
        params.push((":limit".to_owned(), Value::Integer(effective_limit)));

        let mut rows = connection.query(&sql, params).await?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            events.push(TaskEventRecord {
                id: integer_value(row.get_value(0)?, "task_events.id")?,
                event_id: text_value(row.get_value(1)?, "task_events.event_id")?,
                board_id: text_value(row.get_value(2)?, "task_events.board_id")?,
                task_id: optional_text_value(row.get_value(3)?, "task_events.task_id")?,
                run_id: optional_text_value(row.get_value(4)?, "task_events.run_id")?,
                kind: text_value(row.get_value(5)?, "task_events.kind")?,
                actor: optional_text_value(row.get_value(6)?, "task_events.actor")?,
                payload_json: text_value(row.get_value(7)?, "task_events.payload_json")?,
                created_at: integer_value(row.get_value(8)?, "task_events.created_at")?,
            });
        }
        let next_after = events.last().map_or(after, |event| event.id);
        Ok(TaskEventListPage { events, next_after })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn list_events_resolves_board_id_or_slug_reads_archived_board_and_keeps_raw_values() {
        let (_directory, store, _path) = store("event-list-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_event_other', 'event-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert archived board");
        let task = store
            .create_task(
                "b_event_other",
                create_input("t_event_board", None, "Event board"),
            )
            .await
            .expect("create task");
        connection
            .execute(
                "UPDATE boards SET archived_at = 2 WHERE id = ?1",
                [task.board_id.as_str()],
            )
            .await
            .expect("archive board");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6)",
                (
                    "e_event_opaque",
                    task.board_id.as_str(),
                    task.id.as_str(),
                    "unknown.future.kind",
                    r#"["opaque",1]"#,
                    777_i64,
                ),
            )
            .await
            .expect("insert opaque event");

        let by_slug = store
            .list_events("event-other", Some(&task.id), 0, 10)
            .await
            .expect("list by slug");
        let by_id = store
            .list_events("b_event_other", Some(&task.id), 0, 10)
            .await
            .expect("list by id");
        assert_eq!(by_slug, by_id);
        assert_eq!(by_slug.events.len(), 2);
        let opaque = by_slug
            .events
            .iter()
            .find(|event| event.event_id == "e_event_opaque")
            .expect("opaque event");
        assert_eq!(opaque.board_id, "b_event_other");
        assert_eq!(opaque.task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(opaque.run_id, None);
        assert_eq!(opaque.kind, "unknown.future.kind");
        assert_eq!(opaque.actor, None);
        assert_eq!(opaque.payload_json, r#"["opaque",1]"#);
        assert_eq!(opaque.created_at, 777);
    }

    #[tokio::test]
    async fn list_events_orders_by_numeric_id_uses_exclusive_cursor_and_caps_limit() {
        let (_directory, store, _path) = store("event-list-page").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        for index in 0..1_005_i64 {
            let event_id = format!("e_event_page_{index}");
            connection
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', NULL, NULL, 'page.event', 'tester', '{\"index\":0}', ?2)",
                    (event_id.as_str(), index),
                )
                .await
                .expect("insert page event");
        }

        let first = store
            .list_events("default", None, 0, 5_000)
            .await
            .expect("list capped page");
        assert_eq!(first.events.len(), 1_000);
        assert_eq!(first.events.first().expect("first event").id, 1);
        assert_eq!(first.events.last().expect("last event").id, 1_000);
        assert_eq!(first.next_after, 1_000);

        let exclusive = store
            .list_events("b_default", None, 1_000, 2)
            .await
            .expect("list after cursor");
        assert_eq!(
            exclusive
                .events
                .iter()
                .map(|event| event.id)
                .collect::<Vec<_>>(),
            vec![1_001, 1_002]
        );
        assert_eq!(exclusive.next_after, 1_002);

        let empty = store
            .list_events("default", None, 10_000, 50)
            .await
            .expect("empty page");
        assert!(empty.events.is_empty());
        assert_eq!(empty.next_after, 10_000);

        let zero = store
            .list_events("default", None, 10_000, 0)
            .await
            .expect("zero limit page");
        assert!(zero.events.is_empty());
        assert_eq!(zero.next_after, 10_000);
    }

    #[tokio::test]
    async fn list_events_filters_tasks_and_rejects_unknown_or_cross_board_tasks() {
        let (_directory, store, _path) = store("event-list-filter").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_event_filter_other', 'event-filter-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let default_task = store
            .create_task(
                "default",
                create_input("t_event_filter_default", None, "Default event"),
            )
            .await
            .expect("create default task");
        let other_task = store
            .create_task(
                "event-filter-other",
                create_input("t_event_filter_other", None, "Other event"),
            )
            .await
            .expect("create other task");

        let filtered = store
            .list_events("default", Some(&format!("  {}  ", default_task.id)), 0, 20)
            .await
            .expect("list task events");
        assert!(!filtered.events.is_empty());
        assert!(
            filtered
                .events
                .iter()
                .all(|event| event.task_id.as_deref() == Some(default_task.id.as_str()))
        );

        let cross_board = store
            .list_events("default", Some(&other_task.id), 0, 20)
            .await
            .expect_err("cross-board task must be rejected");
        assert!(matches!(
            cross_board,
            StoreError::TaskNotFound(id) if id == other_task.id
        ));

        let unknown = store
            .list_events("default", Some("t_event_filter_missing"), 0, 20)
            .await
            .expect_err("unknown task must be rejected");
        assert!(matches!(
            unknown,
            StoreError::TaskNotFound(id) if id == "t_event_filter_missing"
        ));

        let invalid = store
            .list_events("default", Some("default#1"), 0, 20)
            .await
            .expect_err("board-local task reference must be rejected");
        assert!(
            matches!(invalid, StoreError::InvalidInput(message) if message.contains("task id"))
        );
    }

    #[tokio::test]
    async fn list_events_rejects_invalid_cursor_and_board() {
        let (_directory, store, _path) = store("event-list-errors").await;
        store.initialize().await.expect("initialize");

        let invalid_cursor = store
            .list_events("default", None, -1, 10)
            .await
            .expect_err("negative cursor must be rejected");
        assert!(matches!(
            invalid_cursor,
            StoreError::InvalidInput(message) if message.contains("after")
        ));

        let blank_board = store
            .list_events("   ", None, 0, 10)
            .await
            .expect_err("blank board must be rejected");
        assert!(matches!(
            blank_board,
            StoreError::InvalidInput(message) if message.contains("board")
        ));

        let missing_board = store
            .list_events("event-list-missing-board", None, 0, 10)
            .await
            .expect_err("missing board must be rejected");
        assert!(matches!(
            missing_board,
            StoreError::BoardNotFound(selector) if selector == "event-list-missing-board"
        ));
    }
}
