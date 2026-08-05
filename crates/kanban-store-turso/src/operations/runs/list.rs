use crate::operations::shared::validate_task_id;
use crate::{db::TursoStore, domain::TaskRunRecord, error::StoreError, shared::*};

use super::shared::RUN_SELECT;

impl TursoStore {
    pub async fn list_runs(&self, task_id: &str) -> Result<Vec<TaskRunRecord>, StoreError> {
        let task_id = task_id.trim();
        validate_task_id(task_id)?;

        let connection = self.connection().await?;
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
        let board_id = text_value(task.get_value(0)?, "tasks.board_id")?;

        let mut rows = connection
            .query(
                &format!(
                    "{RUN_SELECT} WHERE board_id = :board_id AND task_id = :task_id ORDER BY started_at DESC, id DESC"
                ),
                [(":board_id", board_id.as_str()), (":task_id", task_id)],
            )
            .await?;
        let mut runs = Vec::new();
        while let Some(row) = rows.next().await? {
            runs.push(run_from_row(row)?);
        }
        Ok(runs)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn list_runs_rejects_non_global_task_ids() {
        let (_directory, store, _path) = store("run-list-invalid-id").await;
        store.initialize().await.expect("initialize");

        let error = store
            .list_runs("default#1")
            .await
            .expect_err("board-local references must be rejected");
        assert!(matches!(
            error,
            StoreError::InvalidInput(message) if message.contains("task id")
        ));
    }

    #[tokio::test]
    async fn list_runs_reports_missing_task() {
        let (_directory, store, _path) = store("run-list-missing-task").await;
        store.initialize().await.expect("initialize");

        let error = store
            .list_runs("t_run_list_missing")
            .await
            .expect_err("missing task must fail");
        assert!(matches!(
            error,
            StoreError::TaskNotFound(task_id) if task_id == "t_run_list_missing"
        ));
    }

    #[tokio::test]
    async fn list_runs_orders_history_stably_and_isolates_tasks() {
        let (_directory, store, _path) = store("run-list-history").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_run_list_history",
            "run-list-history-task",
            "Run list history",
        )
        .await;
        let other = ready_task_for_claim(
            &store,
            "t_run_list_history_other",
            "run-list-history-other",
            "Other run list history",
        )
        .await;

        let mut task = task;
        for (index, now) in [300_i64, 500, 700].into_iter().enumerate() {
            let run_id = format!("r_run_list_{index}");
            let claim_token = format!("run-list-token-{index}");
            let claimed = store
                .claim_task(
                    &task.id,
                    claim_input(
                        task.lock_version,
                        "run-list-worker",
                        &claim_token,
                        &run_id,
                        &format!("e_run_list_claim_{index}"),
                        "{}",
                        now,
                        100,
                    ),
                )
                .await
                .expect("claim task");
            task = store
                .release_task(
                    &task.id,
                    release_input(
                        claimed.task.lock_version,
                        "run-list-worker",
                        &claim_token,
                        &format!("e_run_list_release_{index}"),
                        now + 10,
                    ),
                )
                .await
                .expect("release task");
        }

        let other_claim = store
            .claim_task(
                &other.id,
                claim_input(
                    other.lock_version,
                    "run-list-other-worker",
                    "run-list-other-token",
                    "r_run_list_other",
                    "e_run_list_other_claim",
                    "{}",
                    400,
                    100,
                ),
            )
            .await
            .expect("claim other task");
        store
            .release_task(
                &other.id,
                release_input(
                    other_claim.task.lock_version,
                    "run-list-other-worker",
                    "run-list-other-token",
                    "e_run_list_other_release",
                    410,
                ),
            )
            .await
            .expect("release other task");

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE task_runs SET started_at = 500 WHERE id IN ('r_run_list_0', 'r_run_list_1')",
                (),
            )
            .await
            .expect("set tied start times");

        let runs = store.list_runs(&task.id).await.expect("list task runs");
        assert_eq!(
            runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
            vec!["r_run_list_2", "r_run_list_1", "r_run_list_0"]
        );
        assert!(runs.iter().all(|run| run.task_id == task.id));
        let other_runs = store
            .list_runs(&other.id)
            .await
            .expect("list other task runs");
        assert_eq!(other_runs.len(), 1);
        assert_eq!(other_runs[0].id, "r_run_list_other");
        assert_eq!(other_runs[0].task_id, other.id);
    }

    #[tokio::test]
    async fn list_runs_reads_archived_task_history() {
        let (_directory, store, _path) = store("run-list-archived").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_run_list_archived",
            "run-list-archived-task",
            "Archived run list",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    task.lock_version,
                    "run-list-archived-worker",
                    "run-list-archived-token",
                    "r_run_list_archived",
                    "e_run_list_archived_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim archived history task");
        store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "run-list-archived-worker",
                    "run-list-archived-token",
                    "e_run_list_archived_release",
                    310,
                ),
            )
            .await
            .expect("release archived history task");

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 400 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");

        let runs = store
            .list_runs(&task.id)
            .await
            .expect("archived history remains readable");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, "r_run_list_archived");
        assert_eq!(runs[0].status, "canceled");
    }
}
