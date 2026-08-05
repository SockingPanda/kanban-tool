use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::list_support::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListSort {
    Seq,
    SeqDesc,
    Title,
    TitleDesc,
    Status,
    StatusDesc,
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    Assignee,
    AssigneeDesc,
    ScheduledAt,
    ScheduledAtDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
    DueAt,
    DueAtDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanFilter {
    PlanNeeded,
    HasSteps,
    IncompleteRequiredSteps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<String>,
    pub priorities: Vec<i64>,
    pub include_archived: bool,
    pub assignee: Option<String>,
    pub q: Option<String>,
    pub plan_filters: Vec<TaskPlanFilter>,
    pub sort: TaskListSort,
    pub limit: usize,
    pub offset: usize,
}

impl Default for TaskListOptions {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            priorities: Vec::new(),
            include_archived: false,
            assignee: None,
            q: None,
            plan_filters: Vec::new(),
            sort: TaskListSort::Position,
            limit: 100,
            offset: 0,
        }
    }
}

impl TursoStore {
    pub async fn list_tasks(
        &self,
        board_selector: &str,
        options: TaskListOptions,
    ) -> Result<TaskListPage, StoreError> {
        validate_task_list_options(&options)?;
        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id, slug FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [board_selector],
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
        let board_slug = text_value(board.get_value(1)?, "boards.slug")?;
        let (where_sql, params) = task_list_where(&board_id, &board_slug, &options);

        let total_row = first_row(
            connection
                .query(
                    &format!("SELECT COUNT(*) {TASK_FROM} {where_sql}"),
                    params.clone(),
                )
                .await?,
        )
        .await?;
        let total = integer_value(total_row.get_value(0)?, "tasks.total")?;
        let total = usize::try_from(total).map_err(|_| StoreError::InvalidStoredValue {
            field: "tasks.total",
        })?;

        let limit = i64::try_from(options.limit)
            .map_err(|_| StoreError::InvalidInput("limit is too large".to_owned()))?;
        let offset = i64::try_from(options.offset)
            .map_err(|_| StoreError::InvalidInput("offset is too large".to_owned()))?;
        let mut page_params = params;
        page_params.push((":limit".to_owned(), Value::Integer(limit)));
        page_params.push((":offset".to_owned(), Value::Integer(offset)));
        let mut rows = connection
            .query(
                &format!(
                    "{TASK_SELECT} {where_sql} ORDER BY {} LIMIT :limit OFFSET :offset",
                    task_order_by(options.sort)
                ),
                page_params,
            )
            .await?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(task_from_row(row)?);
        }
        Ok(TaskListPage { tasks, total })
    }
}
