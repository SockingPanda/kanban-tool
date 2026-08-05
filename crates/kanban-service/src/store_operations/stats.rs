use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

impl TursoStore {
    pub async fn get_stats(
        &self,
        board_selector: &str,
        generated_at: i64,
    ) -> Result<QueueStatsRecord, StoreError> {
        if generated_at < 0 {
            return Err(StoreError::InvalidInput(
                "generated_at must be non-negative".to_owned(),
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

        let mut status_rows = connection
            .query(
                "SELECT status, COUNT(*) FROM tasks WHERE board_id = :board_id GROUP BY status ORDER BY status",
                [(":board_id", board_id.as_str())],
            )
            .await?;
        let mut status_counts = Vec::new();
        while let Some(row) = status_rows.next().await? {
            status_counts.push(StatusCountRecord {
                status: text_value(row.get_value(0)?, "tasks.status")?,
                count: integer_value(row.get_value(1)?, "tasks.status_count")?,
            });
        }

        let mut stale_rows = connection
            .query(
                "SELECT id, seq, title, claim_owner, claim_expires_at, last_heartbeat_at, current_run_id, retry_count, max_retries FROM tasks WHERE board_id = :board_id AND status = 'running' AND claim_expires_at <= :generated_at ORDER BY claim_expires_at ASC, updated_at ASC",
                [
                    (":board_id", Value::Text(board_id.clone())),
                    (":generated_at", Value::Integer(generated_at)),
                ],
            )
            .await?;
        let mut stale_claims = Vec::new();
        while let Some(row) = stale_rows.next().await? {
            stale_claims.push(StaleClaimRecord {
                task_id: text_value(row.get_value(0)?, "tasks.id")?,
                seq: integer_value(row.get_value(1)?, "tasks.seq")?,
                title: text_value(row.get_value(2)?, "tasks.title")?,
                claim_owner: optional_text_value(row.get_value(3)?, "tasks.claim_owner")?,
                claim_expires_at: optional_integer_value(
                    row.get_value(4)?,
                    "tasks.claim_expires_at",
                )?,
                last_heartbeat_at: optional_integer_value(
                    row.get_value(5)?,
                    "tasks.last_heartbeat_at",
                )?,
                current_run_id: optional_text_value(row.get_value(6)?, "tasks.current_run_id")?,
                retry_count: integer_value(row.get_value(7)?, "tasks.retry_count")?,
                max_retries: optional_integer_value(row.get_value(8)?, "tasks.max_retries")?,
            });
        }

        let mut blocked_rows = connection
            .query(
                "SELECT COALESCE(NULLIF(status_reason, ''), 'unspecified') AS reason, COUNT(*) FROM tasks WHERE board_id = :board_id AND status = 'blocked' GROUP BY reason ORDER BY COUNT(*) DESC, reason ASC",
                [(":board_id", board_id.as_str())],
            )
            .await?;
        let mut blocked_reasons = Vec::new();
        while let Some(row) = blocked_rows.next().await? {
            blocked_reasons.push(BlockedReasonCountRecord {
                reason: text_value(row.get_value(0)?, "tasks.status_reason")?,
                count: integer_value(row.get_value(1)?, "tasks.blocked_count")?,
            });
        }

        let unplanned_active_tasks = integer_value(
            first_row(
                connection
                    .query(
                        "SELECT COUNT(*) FROM tasks AS t WHERE t.board_id = :board_id AND t.status NOT IN ('done', 'archived') AND t.archived_at IS NULL AND NOT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id) AND NOT EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = t.board_id AND ep.task_id = t.id AND ep.state = 'not_required')",
                        [(":board_id", board_id.as_str())],
                    )
                    .await?,
            )
            .await?
            .get_value(0)?,
            "tasks.unplanned_active_tasks",
        )?;
        let active_parents_with_incomplete_required_steps = integer_value(
            first_row(
                connection
                    .query(
                        "SELECT COUNT(*) FROM tasks AS t WHERE t.board_id = :board_id AND t.status NOT IN ('done', 'archived') AND t.archived_at IS NULL AND EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1 AND s.status NOT IN ('done', 'skipped'))",
                        [(":board_id", board_id.as_str())],
                    )
                    .await?,
            )
            .await?
            .get_value(0)?,
            "tasks.active_parents_with_incomplete_required_steps",
        )?;

        Ok(QueueStatsRecord {
            board_id,
            generated_at,
            status_counts,
            stale_claims,
            blocked_reasons,
            unplanned_active_tasks,
            active_parents_with_incomplete_required_steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use crate::{StoreError, domain::BlockedReasonCountRecord};

    #[tokio::test]
    async fn stats_resolves_board_and_counts_statuses_and_blocked_reasons() {
        let (_directory, store, _path) = store("stats-query").await;
        store.initialize().await.expect("initialize");
        store
            .create_task("default", create_input("t_stats_todo", None, "todo"))
            .await
            .expect("create todo");
        store
            .create_task("default", create_input("t_stats_blocked", None, "blocked"))
            .await
            .expect("create blocked");
        store
            .block_task(
                "t_stats_blocked",
                block_input(0, "tester", None, true, "waiting", 100, "e_stats_block"),
            )
            .await
            .expect("block task");

        let stats = store.get_stats(" default ", 500).await.expect("stats");
        assert_eq!(stats.board_id, "b_default");
        assert_eq!(stats.generated_at, 500);
        assert_eq!(
            stats
                .status_counts
                .iter()
                .find(|count| count.status == "blocked")
                .map(|count| count.count),
            Some(1)
        );
        assert_eq!(
            stats.blocked_reasons,
            vec![BlockedReasonCountRecord {
                reason: "waiting".to_owned(),
                count: 1,
            }]
        );
    }

    #[tokio::test]
    async fn stats_rejects_missing_or_empty_board() {
        let (_directory, store, _path) = store("stats-query-errors").await;
        store.initialize().await.expect("initialize");

        assert!(matches!(
            store.get_stats("missing", 1).await,
            Err(StoreError::BoardNotFound(board)) if board == "missing"
        ));
        assert!(matches!(
            store.get_stats("  ", 1).await,
            Err(StoreError::InvalidInput(message)) if message == "board is required"
        ));
    }
}
