use std::collections::HashSet;

use turso::{Connection, transaction::Transaction};

use crate::{
    domain::*,
    error::StoreError,
    shared::{TASK_SELECT, first_row, task_from_row, text_value},
};

pub(crate) async fn dependency_task_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
        transaction
            .query(
                &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                [(":task_id", task_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    task_from_row(row)
}

pub(crate) async fn dependency_task_in_connection(
    connection: &Connection,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
        connection
            .query(
                &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                [(":task_id", task_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    task_from_row(row)
}

pub(crate) async fn dependency_path_exists(
    transaction: &Transaction<'_>,
    board_id: &str,
    start_task_id: &str,
    target_task_id: &str,
) -> Result<bool, StoreError> {
    // Turso 0.7.x does not implement recursive CTEs. Walk the direct edge
    // relation inside the same immediate transaction instead; the transaction
    // still gives the traversal a stable view and keeps the subsequent insert
    // atomic with the cycle check.
    let mut frontier = vec![start_task_id.to_owned()];
    let mut visited = HashSet::from([start_task_id.to_owned()]);
    while let Some(parent_task_id) = frontier.pop() {
        let mut rows = transaction
            .query(
                "SELECT child_task_id FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id",
                [
                    (":board_id", board_id),
                    (":parent_task_id", parent_task_id.as_str()),
                ],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let child_task_id = text_value(row.get_value(0)?, "task_dependencies.child_task_id")?;
            if child_task_id == target_task_id {
                return Ok(true);
            }
            if visited.insert(child_task_id.clone()) {
                frontier.push(child_task_id);
            }
        }
    }
    Ok(false)
}

pub(crate) fn dependency_parent_satisfied(parent: &TaskRecord) -> bool {
    matches!(parent.status.as_str(), "done" | "archived") || parent.archived_at.is_some()
}

pub(crate) async fn dependency_snapshot_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    task_id: &str,
) -> Result<DependencySnapshotRecord, StoreError> {
    let task = dependency_task_in_transaction(transaction, task_id).await?;
    let mut rows = transaction
        .query(
            "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id = :board_id AND (parent_task_id = :task_id OR child_task_id = :task_id) ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    let mut edges = Vec::new();
    let mut parents = Vec::new();
    let mut children = Vec::new();
    while let Some(row) = rows.next().await? {
        let parent_id = text_value(row.get_value(0)?, "task_dependencies.parent_task_id")?;
        let child_id = text_value(row.get_value(1)?, "task_dependencies.child_task_id")?;
        let parent = dependency_task_in_transaction(transaction, &parent_id).await?;
        let child = dependency_task_in_transaction(transaction, &child_id).await?;
        if child_id == task_id {
            parents.push(parent.clone());
        }
        if parent_id == task_id {
            children.push(child.clone());
        }
        edges.push(DependencyEdgeRecord { parent, child });
    }
    Ok(DependencySnapshotRecord {
        task,
        parents,
        children,
        edges,
    })
}

pub(crate) async fn dependency_snapshot_in_connection(
    connection: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<DependencySnapshotRecord, StoreError> {
    let task = dependency_task_in_connection(connection, task_id).await?;
    let mut rows = connection
        .query(
            "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id = :board_id AND (parent_task_id = :task_id OR child_task_id = :task_id) ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    let mut edges = Vec::new();
    let mut parents = Vec::new();
    let mut children = Vec::new();
    while let Some(row) = rows.next().await? {
        let parent_id = text_value(row.get_value(0)?, "task_dependencies.parent_task_id")?;
        let child_id = text_value(row.get_value(1)?, "task_dependencies.child_task_id")?;
        let parent = dependency_task_in_connection(connection, &parent_id).await?;
        let child = dependency_task_in_connection(connection, &child_id).await?;
        if child_id == task_id {
            parents.push(parent.clone());
        }
        if parent_id == task_id {
            children.push(child.clone());
        }
        edges.push(DependencyEdgeRecord { parent, child });
    }
    Ok(DependencySnapshotRecord {
        task,
        parents,
        children,
        edges,
    })
}
