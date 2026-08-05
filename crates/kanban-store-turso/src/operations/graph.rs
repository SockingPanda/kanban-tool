use std::collections::{BTreeMap, BTreeSet, VecDeque};

use turso::{Connection, Value};

use crate::{
    db::TursoStore,
    domain::{
        BoardTaskMapRecord, GraphQueryBindingRecord, GraphQueryRowRecord, GraphStatusRecord,
        ProjectionStateRecord, RelationRecord, TaskGraphEdgeRecord, TaskGraphMetaRecord,
        TaskGraphNodeRecord, TaskNeighborhoodRecord, TaskRecord,
    },
    error::StoreError,
    operations::relations::{RelationListOptions, relation_from_row},
    shared::{
        TASK_SELECT, first_row, integer_value, now_ms, optional_integer_value, optional_text_value,
        task_from_row, text_value,
    },
};

pub const MAX_GRAPH_DEPTH: usize = 8;
pub const MAX_GRAPH_NODES: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNeighborsOptions {
    pub board: String,
    pub entity_uri: String,
    pub predicate: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatusOptions {
    pub board: Option<String>,
    pub projection: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQueryOptions {
    pub board: String,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskNeighborhoodOptions {
    pub depth: usize,
    pub limit_nodes: usize,
    pub include_archived_context: bool,
}

impl Default for TaskNeighborhoodOptions {
    fn default() -> Self {
        Self {
            depth: 1,
            limit_nodes: 250,
            include_archived_context: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardTaskMapOptions {
    pub active_only: bool,
    pub context_depth: usize,
    pub limit_nodes: usize,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
}

impl Default for BoardTaskMapOptions {
    fn default() -> Self {
        Self {
            active_only: true,
            context_depth: 1,
            limit_nodes: 250,
            include_done_context: true,
            include_archived_context: false,
            hide_isolated: false,
        }
    }
}

#[derive(Debug, Clone)]
struct TaskGraphData {
    tasks: BTreeMap<String, TaskRecord>,
    edges: Vec<TaskGraphEdgeRecord>,
}

impl TursoStore {
    /// Read canonical relation facts.  The query is intentionally scoped by
    /// the subject entity's board and never follows a derived graph store.
    pub async fn graph_neighbors(
        &self,
        options: GraphNeighborsOptions,
    ) -> Result<Vec<RelationRecord>, StoreError> {
        validate_uri(&options.entity_uri)?;
        validate_limit(options.limit)?;
        let board = options.board.trim();
        if board.is_empty() {
            return Err(StoreError::InvalidInput(
                "graph board is required".to_owned(),
            ));
        }
        // Resolve and verify the entity before listing facts.  A relation row
        // alone is not enough to grant access to another board.
        let entity = self.get_entity(&options.entity_uri).await?;
        let board_id = self.resolve_board(board).await?;
        if entity.board_id.as_deref() != Some(board_id.as_str()) {
            return Err(StoreError::EntityNotFound(options.entity_uri));
        }
        self.list_relations(RelationListOptions {
            board: Some(board_id),
            subject_uri: Some(options.entity_uri),
            object_uri: None,
            predicate: options.predicate,
            limit: options.limit,
        })
        .await
    }

    pub async fn graph_status(&self, board: &str) -> Result<GraphStatusRecord, StoreError> {
        let board_id = self.resolve_board(board.trim()).await?;
        let projection = self
            .projection_status(ProjectionStatusOptions {
                board: Some(board_id.clone()),
                projection: "relations".to_owned(),
            })
            .await?;
        let connection = self.connection().await?;
        let relation_count = scalar_count(
            &connection,
            "SELECT COUNT(*) FROM entity_relations WHERE board_id = :board_id",
            vec![(":board_id".to_owned(), Value::Text(board_id))],
            "entity_relations.count",
        )
        .await?;
        Ok(GraphStatusRecord {
            backend: "turso-canonical".to_owned(),
            enabled: true,
            message: format!(
                "canonical relation facts are available; relations={relation_count}, lifecycle={}, dirty={}, lag_jobs={}",
                projection.lifecycle_status,
                projection.dirty,
                projection.pending_jobs + projection.running_jobs + projection.failed_jobs,
            ),
            projection,
        })
    }

    pub async fn projection_status(
        &self,
        options: ProjectionStatusOptions,
    ) -> Result<ProjectionStateRecord, StoreError> {
        let projection = options.projection.trim();
        if !matches!(
            projection,
            "fts" | "vector_tasks" | "vector_label_atoms" | "relations"
        ) {
            return Err(StoreError::InvalidInput(format!(
                "unsupported projection: {projection}"
            )));
        }
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT projection, lifecycle_status, active_generation, active_fingerprint, last_event_id, dirty, last_success_at, last_error, updated_at FROM projection_state WHERE projection = :projection LIMIT 1",
                    [(":projection", projection)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::InvalidStoredValue { field: "projection_state.projection" }
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = match options.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => Some(self.resolve_board(board).await?),
            _ => None,
        };
        let mut params = vec![(":projection".to_owned(), Value::Text(projection.to_owned()))];
        let board_sql = if let Some(board_id) = board_id {
            params.push((":board_id".to_owned(), Value::Text(board_id)));
            " AND (board_id = :board_id OR board_id IS NULL)"
        } else {
            ""
        };
        let jobs = format!(
            "SELECT status, COUNT(*) FROM projection_jobs WHERE target IN (:projection, 'all'){board_sql} GROUP BY status"
        );
        let mut rows = connection.query(&jobs, params).await?;
        let mut pending_jobs = 0;
        let mut running_jobs = 0;
        let mut failed_jobs = 0;
        while let Some(job) = rows.next().await? {
            let status = text_value(job.get_value(0)?, "projection_jobs.status")?;
            let count = integer_value(job.get_value(1)?, "projection_jobs.count")?;
            match status.as_str() {
                "pending" => pending_jobs = count,
                "running" => running_jobs = count,
                "failed" => failed_jobs = count,
                _ => {}
            }
        }
        Ok(ProjectionStateRecord {
            projection: text_value(row.get_value(0)?, "projection_state.projection")?,
            lifecycle_status: text_value(row.get_value(1)?, "projection_state.lifecycle_status")?,
            active_generation: optional_text_value(
                row.get_value(2)?,
                "projection_state.active_generation",
            )?,
            active_fingerprint: optional_text_value(
                row.get_value(3)?,
                "projection_state.active_fingerprint",
            )?,
            last_event_id: integer_value(row.get_value(4)?, "projection_state.last_event_id")?,
            dirty: integer_value(row.get_value(5)?, "projection_state.dirty")? != 0,
            last_success_at: optional_integer_value(
                row.get_value(6)?,
                "projection_state.last_success_at",
            )?,
            last_error: optional_text_value(row.get_value(7)?, "projection_state.last_error")?,
            updated_at: integer_value(row.get_value(8)?, "projection_state.updated_at")?,
            pending_jobs,
            running_jobs,
            failed_jobs,
        })
    }

    /// The old Oxigraph query surface is kept as a deterministic, read-only
    /// relation query.  It accepts a SPARQL-shaped string for compatibility,
    /// but evaluates only canonical relation facts and never executes SQL or a
    /// recursive query supplied by a caller.
    pub async fn graph_query(
        &self,
        options: GraphQueryOptions,
    ) -> Result<Vec<GraphQueryRowRecord>, StoreError> {
        validate_limit(options.limit)?;
        if options.query.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "graph query is required".to_owned(),
            ));
        }
        let relations = self
            .list_relations(RelationListOptions {
                board: Some(options.board),
                limit: options.limit,
                ..RelationListOptions::default()
            })
            .await?;
        Ok(relations
            .into_iter()
            .map(|relation| GraphQueryRowRecord {
                bindings: vec![
                    GraphQueryBindingRecord {
                        name: "subject".to_owned(),
                        value: relation.subject_uri,
                    },
                    GraphQueryBindingRecord {
                        name: "predicate".to_owned(),
                        value: relation.predicate,
                    },
                    GraphQueryBindingRecord {
                        name: "object".to_owned(),
                        value: relation.object_uri,
                    },
                ],
            })
            .collect())
    }

    pub async fn task_neighborhood(
        &self,
        task_id: &str,
        options: TaskNeighborhoodOptions,
    ) -> Result<TaskNeighborhoodRecord, StoreError> {
        validate_task_id(task_id)?;
        let options = normalize_neighborhood_options(options)?;
        let connection = self.connection().await?;
        let center = task_in_connection(&connection, task_id).await?;
        let graph = graph_data_for_board(&connection, &center.board_id).await?;
        let allowed =
            |task: &TaskRecord| options.include_archived_context || task.archived_at.is_none();
        let (node_ids, distances, mut truncated) = bfs_ids(
            &graph,
            &center.id,
            options.depth,
            options.limit_nodes,
            &allowed,
        );
        let visible = node_ids.iter().cloned().collect::<BTreeSet<_>>();
        let edges = graph
            .edges
            .iter()
            .filter(|edge| {
                visible.contains(&edge.source_task_id) && visible.contains(&edge.target_task_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        for task_id in node_ids {
            let task = graph
                .tasks
                .get(&task_id)
                .cloned()
                .ok_or_else(|| StoreError::TaskNotFound(task_id.clone()))?;
            let role = if task.id == center.id {
                "center"
            } else if distances.get(&task.id).copied().unwrap_or(99) > 1 {
                "context"
            } else {
                edge_role(&graph.edges, &center.id, &task.id)
            };
            nodes.push(TaskGraphNodeRecord {
                task,
                role: role.to_owned(),
                context_only: false,
            });
        }
        truncated |= nodes.len() >= options.limit_nodes && graph.tasks.len() > nodes.len();
        let meta = TaskGraphMetaRecord {
            depth: options.depth,
            context_depth: 0,
            generated_at: now_ms(),
            node_count: nodes.len(),
            edge_count: edges.len(),
            truncated,
            active_statuses: active_statuses(),
            active_only: true,
            include_done_context: true,
            include_archived_context: options.include_archived_context,
            hide_isolated: false,
            limit_nodes: options.limit_nodes,
        };
        Ok(TaskNeighborhoodRecord {
            center_task_id: center.id,
            nodes,
            edges,
            meta,
        })
    }

    pub async fn board_task_map(
        &self,
        board: &str,
        options: BoardTaskMapOptions,
    ) -> Result<BoardTaskMapRecord, StoreError> {
        let options = normalize_board_map_options(options)?;
        let connection = self.connection().await?;
        let board_id = resolve_board_id(&connection, board.trim()).await?;
        let graph = graph_data_for_board(&connection, &board_id).await?;
        let active = graph
            .tasks
            .values()
            .filter(|task| {
                is_active(task) && (options.include_archived_context || task.archived_at.is_none())
            })
            .map(|task| task.id.clone())
            .collect::<BTreeSet<_>>();
        let mut node_ids = BTreeSet::new();
        if options.active_only {
            node_ids.extend(active.iter().cloned());
        } else {
            node_ids.extend(
                graph
                    .tasks
                    .values()
                    .filter(|task| options.include_archived_context || task.archived_at.is_none())
                    .map(|task| task.id.clone()),
            );
        }
        if options.context_depth > 0 {
            let context = expand_context_ids(&graph, &node_ids, options.context_depth, |task| {
                if !options.include_archived_context && task.archived_at.is_some() {
                    return false;
                }
                options.include_done_context || task.status != "done"
            });
            node_ids.extend(context);
        }
        let mut truncated = false;
        if node_ids.len() > options.limit_nodes {
            let mut ids = node_ids.into_iter().collect::<Vec<_>>();
            ids.sort_by_key(|id| {
                graph
                    .tasks
                    .get(id)
                    .map(|task| {
                        (
                            !active.contains(id),
                            task.position,
                            task.priority,
                            task.seq,
                            id.clone(),
                        )
                    })
                    .unwrap_or((true, i64::MAX, i64::MAX, i64::MAX, id.clone()))
            });
            ids.truncate(options.limit_nodes);
            node_ids = ids.into_iter().collect();
            truncated = true;
        }
        let visible = node_ids.iter().cloned().collect::<BTreeSet<_>>();
        let mut edges = graph
            .edges
            .iter()
            .filter(|edge| {
                visible.contains(&edge.source_task_id) && visible.contains(&edge.target_task_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if options.hide_isolated {
            let connected = edges
                .iter()
                .flat_map(|edge| [edge.source_task_id.clone(), edge.target_task_id.clone()])
                .collect::<BTreeSet<_>>();
            node_ids.retain(|id| connected.contains(id));
            let visible = node_ids.iter().cloned().collect::<BTreeSet<_>>();
            edges.retain(|edge| {
                visible.contains(&edge.source_task_id) && visible.contains(&edge.target_task_id)
            });
        }
        let mut nodes = node_ids
            .iter()
            .map(|id| {
                let task = graph
                    .tasks
                    .get(id)
                    .cloned()
                    .ok_or_else(|| StoreError::TaskNotFound(id.clone()))?;
                let active_task = active.contains(id);
                Ok(TaskGraphNodeRecord {
                    task,
                    role: if active_task {
                        "active".to_owned()
                    } else {
                        "context".to_owned()
                    },
                    context_only: !active_task,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        nodes.sort_by_key(|node| (node.task.position, node.task.seq, node.task.id.clone()));
        let meta = TaskGraphMetaRecord {
            depth: 0,
            context_depth: options.context_depth,
            generated_at: now_ms(),
            node_count: nodes.len(),
            edge_count: edges.len(),
            truncated,
            active_statuses: active_statuses(),
            active_only: options.active_only,
            include_done_context: options.include_done_context,
            include_archived_context: options.include_archived_context,
            hide_isolated: options.hide_isolated,
            limit_nodes: options.limit_nodes,
        };
        Ok(BoardTaskMapRecord { nodes, edges, meta })
    }

    async fn resolve_board(&self, selector: &str) -> Result<String, StoreError> {
        let connection = self.connection().await?;
        resolve_board_id(&connection, selector).await
    }
}

fn validate_uri(uri: &str) -> Result<(), StoreError> {
    if !uri.trim().starts_with("kb://") || uri.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "entity URI must start with kb://".to_owned(),
        ));
    }
    Ok(())
}

fn validate_limit(limit: usize) -> Result<(), StoreError> {
    if limit == 0 || limit > MAX_GRAPH_NODES {
        return Err(StoreError::InvalidInput(format!(
            "graph limit must be between 1 and {MAX_GRAPH_NODES}"
        )));
    }
    Ok(())
}

fn validate_task_id(task_id: &str) -> Result<(), StoreError> {
    if !task_id.trim().starts_with("t_") || task_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_neighborhood_options(
    options: TaskNeighborhoodOptions,
) -> Result<TaskNeighborhoodOptions, StoreError> {
    if options.depth > MAX_GRAPH_DEPTH {
        return Err(StoreError::InvalidInput(format!(
            "task neighborhood depth must be <= {MAX_GRAPH_DEPTH}"
        )));
    }
    validate_limit(options.limit_nodes)?;
    Ok(options)
}

fn normalize_board_map_options(
    options: BoardTaskMapOptions,
) -> Result<BoardTaskMapOptions, StoreError> {
    if options.context_depth > MAX_GRAPH_DEPTH {
        return Err(StoreError::InvalidInput(format!(
            "board task map context_depth must be <= {MAX_GRAPH_DEPTH}"
        )));
    }
    validate_limit(options.limit_nodes)?;
    Ok(options)
}

async fn resolve_board_id(connection: &Connection, selector: &str) -> Result<String, StoreError> {
    if selector.is_empty() {
        return Err(StoreError::InvalidInput("board is required".to_owned()));
    }
    let row = first_row(
        connection
            .query(
                "SELECT id FROM boards WHERE id = :selector OR slug = :selector LIMIT 1",
                [(":selector", selector)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn scalar_count(
    connection: &Connection,
    sql: &str,
    params: Vec<(String, Value)>,
    field: &'static str,
) -> Result<i64, StoreError> {
    let row = first_row(connection.query(sql, params).await?).await?;
    integer_value(row.get_value(0)?, field)
}

async fn task_in_connection(
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

async fn graph_data_for_board(
    connection: &Connection,
    board_id: &str,
) -> Result<TaskGraphData, StoreError> {
    let mut rows = connection
        .query(
            &format!("{TASK_SELECT} WHERE t.board_id = :board_id"),
            [(":board_id", board_id)],
        )
        .await?;
    let mut tasks = BTreeMap::new();
    while let Some(row) = rows.next().await? {
        let task = task_from_row(row)?;
        tasks.insert(task.id.clone(), task);
    }
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    let mut append = |edge: TaskGraphEdgeRecord| {
        let key = (
            edge.source_task_id.clone(),
            edge.target_task_id.clone(),
            edge.kind.clone(),
        );
        if seen.insert(key) {
            edges.push(edge);
        }
    };
    let mut dependency_rows = connection
        .query(
            "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id = :board_id ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    while let Some(row) = dependency_rows.next().await? {
        let parent = text_value(row.get_value(0)?, "task_dependencies.parent_task_id")?;
        let child = text_value(row.get_value(1)?, "task_dependencies.child_task_id")?;
        append(TaskGraphEdgeRecord {
            id: format!("dependency:{parent}->{child}"),
            source_task_id: parent,
            target_task_id: child,
            kind: "dependency".to_owned(),
            required: true,
            blocking: true,
        });
    }
    let mut subtask_rows = connection
        .query(
            "SELECT parent_task_id, child_task_id, required FROM task_subtasks WHERE board_id = :board_id ORDER BY position ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    while let Some(row) = subtask_rows.next().await? {
        let parent = text_value(row.get_value(0)?, "task_subtasks.parent_task_id")?;
        let child = text_value(row.get_value(1)?, "task_subtasks.child_task_id")?;
        let required = integer_value(row.get_value(2)?, "task_subtasks.required")? != 0;
        append(TaskGraphEdgeRecord {
            id: format!("step:{parent}->{child}"),
            source_task_id: parent,
            target_task_id: child,
            kind: "step".to_owned(),
            required,
            blocking: required,
        });
    }
    let mut linked_rows = connection
        .query(
            "SELECT parent_task_id, linked_task_id, required FROM task_steps WHERE board_id = :board_id AND linked_task_id IS NOT NULL ORDER BY position ASC, parent_task_id ASC, linked_task_id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    while let Some(row) = linked_rows.next().await? {
        let parent = text_value(row.get_value(0)?, "task_steps.parent_task_id")?;
        let child = text_value(row.get_value(1)?, "task_steps.linked_task_id")?;
        let required = integer_value(row.get_value(2)?, "task_steps.required")? != 0;
        append(TaskGraphEdgeRecord {
            id: format!("step:{parent}->{child}"),
            source_task_id: parent,
            target_task_id: child,
            kind: "step".to_owned(),
            required,
            blocking: required,
        });
    }
    // Relation facts are the canonical graph surface.  Native dependency and
    // step tables above are retained as compatibility facts during migration;
    // deduplication makes the transition deterministic.
    let mut relation_rows = connection
        .query(
            "SELECT r.id, r.subject_uri, r.predicate, r.object_uri, r.graph_uri, r.board_id, r.authoritative_store, r.source_table, r.source_id, r.source_event_id, r.metadata_json, r.created_at, r.updated_at FROM entity_relations r JOIN entities s ON s.uri = r.subject_uri WHERE s.board_id = :board_id AND r.board_id = :board_id ORDER BY r.id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    while let Some(row) = relation_rows.next().await? {
        let relation = relation_from_row(row)?;
        let Some(subject) = task_id_from_uri(&relation.subject_uri) else {
            continue;
        };
        let Some(object) = task_id_from_uri(&relation.object_uri) else {
            continue;
        };
        if !tasks.contains_key(&subject) || !tasks.contains_key(&object) {
            continue;
        }
        match relation.predicate.as_str() {
            "depends_on" => append(TaskGraphEdgeRecord {
                id: format!("relation:{}", relation.id),
                source_task_id: object,
                target_task_id: subject,
                kind: "dependency".to_owned(),
                required: true,
                blocking: true,
            }),
            "subtask" | "contains_task" | "has_subtask" => append(TaskGraphEdgeRecord {
                id: format!("relation:{}", relation.id),
                source_task_id: subject,
                target_task_id: object,
                kind: "step".to_owned(),
                required: true,
                blocking: true,
            }),
            _ => {}
        }
    }
    Ok(TaskGraphData { tasks, edges })
}

fn task_id_from_uri(uri: &str) -> Option<String> {
    uri.strip_prefix("kb://task/")
        .filter(|id| id.starts_with("t_") && id.len() > 2)
        .map(ToOwned::to_owned)
}

fn bfs_ids<F: Fn(&TaskRecord) -> bool>(
    graph: &TaskGraphData,
    center: &str,
    depth: usize,
    limit: usize,
    allowed: &F,
) -> (Vec<String>, BTreeMap<String, usize>, bool) {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.source_task_id.clone())
            .or_default()
            .push(edge.target_task_id.clone());
        adjacency
            .entry(edge.target_task_id.clone())
            .or_default()
            .push(edge.source_task_id.clone());
    }
    let mut queue = VecDeque::from([(center.to_owned(), 0usize)]);
    let mut distances = BTreeMap::from([(center.to_owned(), 0usize)]);
    while let Some((current, distance)) = queue.pop_front() {
        if distance >= depth {
            continue;
        }
        for next in adjacency.get(&current).into_iter().flatten() {
            if distances.contains_key(next) {
                continue;
            }
            if graph.tasks.get(next).is_none_or(|task| !allowed(task)) {
                continue;
            }
            distances.insert(next.clone(), distance + 1);
            queue.push_back((next.clone(), distance + 1));
        }
    }
    let mut ids = distances.keys().cloned().collect::<Vec<_>>();
    let mut truncated = false;
    if ids.len() > limit {
        ids.sort_by_key(|id| {
            let distance = distances.get(id).copied().unwrap_or(usize::MAX);
            graph
                .tasks
                .get(id)
                .map(|task| (distance, task.position, task.priority, task.seq, id.clone()))
                .unwrap_or((distance, i64::MAX, i64::MAX, i64::MAX, id.clone()))
        });
        ids.truncate(limit);
        if !ids.iter().any(|id| id == center) {
            ids.pop();
            ids.push(center.to_owned());
        }
        truncated = true;
    }
    ids.sort_by_key(|id| {
        graph
            .tasks
            .get(id)
            .map(|task| {
                (
                    distances.get(id).copied().unwrap_or(usize::MAX),
                    task.position,
                    task.seq,
                    id.clone(),
                )
            })
            .unwrap_or((usize::MAX, i64::MAX, i64::MAX, id.clone()))
    });
    (ids, distances, truncated)
}

fn expand_context_ids<F: Fn(&TaskRecord) -> bool>(
    graph: &TaskGraphData,
    seed: &BTreeSet<String>,
    depth: usize,
    allowed: F,
) -> BTreeSet<String> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.source_task_id.clone())
            .or_default()
            .push(edge.target_task_id.clone());
        adjacency
            .entry(edge.target_task_id.clone())
            .or_default()
            .push(edge.source_task_id.clone());
    }
    let mut visited = seed.clone();
    let mut queue = seed
        .iter()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<VecDeque<_>>();
    while let Some((current, distance)) = queue.pop_front() {
        if distance >= depth {
            continue;
        }
        for next in adjacency.get(&current).into_iter().flatten() {
            if visited.contains(next) {
                continue;
            }
            let Some(task) = graph.tasks.get(next) else {
                continue;
            };
            if !allowed(task) {
                continue;
            }
            visited.insert(next.clone());
            queue.push_back((next.clone(), distance + 1));
        }
    }
    visited
}

fn edge_role(edges: &[TaskGraphEdgeRecord], center: &str, task_id: &str) -> &'static str {
    edges
        .iter()
        .find_map(|edge| {
            if edge.source_task_id == task_id && edge.target_task_id == center {
                Some(if edge.kind == "step" {
                    "step_parent"
                } else {
                    "dependency_parent"
                })
            } else if edge.source_task_id == center && edge.target_task_id == task_id {
                Some(if edge.kind == "step" {
                    "step_child"
                } else {
                    "dependency_child"
                })
            } else {
                None
            }
        })
        .unwrap_or("context")
}

fn is_active(task: &TaskRecord) -> bool {
    matches!(
        task.status.as_str(),
        "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review"
    )
}

fn active_statuses() -> Vec<String> {
    [
        "triage",
        "todo",
        "scheduled",
        "ready",
        "running",
        "blocked",
        "review",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}
