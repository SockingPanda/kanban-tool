use crate::connect_file;

use super::{MAX_TASK_LIST_LIMIT, TaskRecord, all, board_id, get_task_by_id_global_conn};

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, TaskStatus};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT_NODES: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGraphNodeRole {
    Center,
    DependencyParent,
    DependencyChild,
    SubtaskParent,
    SubtaskChild,
    Active,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGraphEdgeKind {
    Dependency,
    Subtask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphNodeRecord {
    pub task: TaskRecord,
    pub role: TaskGraphNodeRole,
    pub context_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphEdgeRecord {
    pub id: String,
    pub source_task_id: String,
    pub target_task_id: String,
    pub kind: TaskGraphEdgeKind,
    pub required: bool,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphMeta {
    pub depth: usize,
    pub context_depth: usize,
    pub generated_at: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub truncated: bool,
    pub active_statuses: Vec<TaskStatus>,
    pub active_only: bool,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
    pub limit_nodes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNeighborhoodRecord {
    pub center_task_id: String,
    pub nodes: Vec<TaskGraphNodeRecord>,
    pub edges: Vec<TaskGraphEdgeRecord>,
    pub meta: TaskGraphMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardTaskMapRecord {
    pub nodes: Vec<TaskGraphNodeRecord>,
    pub edges: Vec<TaskGraphEdgeRecord>,
    pub meta: TaskGraphMeta,
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
            limit_nodes: DEFAULT_LIMIT_NODES,
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
            limit_nodes: DEFAULT_LIMIT_NODES,
            include_done_context: true,
            include_archived_context: false,
            hide_isolated: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependencyEdge {
    parent_task_id: String,
    child_task_id: String,
}

pub fn task_neighborhood(
    path: impl AsRef<Path>,
    task_id: &str,
    options: TaskNeighborhoodOptions,
) -> Result<TaskNeighborhoodRecord> {
    let conn = connect_file(path.as_ref())?;
    let center = get_task_by_id_global_conn(&conn, task_id)?;
    let options = normalize_neighborhood_options(options)?;
    let all_tasks = tasks_by_id(&conn, &center.board_id)?;
    let dependencies = dependency_edges_for_board(&conn, &center.board_id)?;
    let mut node_ids = BTreeSet::new();
    node_ids.insert(center.id.clone());
    let mut roles = BTreeMap::new();
    roles.insert(center.id.clone(), TaskGraphNodeRole::Center);

    for edge in &dependencies {
        if edge.child_task_id == center.id {
            if context_task_allowed(
                &all_tasks,
                &edge.parent_task_id,
                options.include_archived_context,
                true,
            ) {
                node_ids.insert(edge.parent_task_id.clone());
                roles
                    .entry(edge.parent_task_id.clone())
                    .or_insert(TaskGraphNodeRole::DependencyParent);
            }
        }
        if edge.parent_task_id == center.id {
            if context_task_allowed(
                &all_tasks,
                &edge.child_task_id,
                options.include_archived_context,
                true,
            ) {
                node_ids.insert(edge.child_task_id.clone());
                roles
                    .entry(edge.child_task_id.clone())
                    .or_insert(TaskGraphNodeRole::DependencyChild);
            }
        }
    }

    let (node_ids, truncated) = limit_node_ids_preserving(
        node_ids,
        &all_tasks,
        options.limit_nodes,
        std::slice::from_ref(&center.id),
    );
    let visible = node_ids.iter().cloned().collect::<HashSet<_>>();
    let edges = graph_edges_from_dependencies(&dependencies, &visible);
    let nodes = graph_nodes_from_ids(&node_ids, &all_tasks, &roles, false)?;
    let meta = TaskGraphMeta {
        depth: options.depth,
        context_depth: 0,
        generated_at: SystemClock.now_ms(),
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

pub fn board_task_map(
    path: impl AsRef<Path>,
    board: &str,
    options: BoardTaskMapOptions,
) -> Result<BoardTaskMapRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let options = normalize_board_task_map_options(options)?;
    let all_tasks = tasks_by_id(&conn, &board_id)?;
    let dependencies = dependency_edges_for_board(&conn, &board_id)?;
    let active_statuses = active_statuses();
    let active_status_set = active_statuses.iter().copied().collect::<HashSet<_>>();
    let mut active_ids = BTreeSet::new();
    let mut node_ids = BTreeSet::new();

    for task in all_tasks.values() {
        let is_active = active_status_set.contains(&task.status) && task.archived_at.is_none();
        if is_active || !options.active_only {
            if options.include_archived_context || task.archived_at.is_none() {
                active_ids.insert(task.id.clone());
                node_ids.insert(task.id.clone());
            }
        }
    }

    if options.context_depth > 0 {
        for edge in &dependencies {
            if active_ids.contains(&edge.parent_task_id) {
                maybe_insert_context(
                    &mut node_ids,
                    &all_tasks,
                    &edge.child_task_id,
                    options.include_done_context,
                    options.include_archived_context,
                );
            }
            if active_ids.contains(&edge.child_task_id) {
                maybe_insert_context(
                    &mut node_ids,
                    &all_tasks,
                    &edge.parent_task_id,
                    options.include_done_context,
                    options.include_archived_context,
                );
            }
        }
    }

    let (mut node_ids, mut truncated) = limit_node_ids(node_ids, &all_tasks, options.limit_nodes);
    let mut visible = node_ids.iter().cloned().collect::<HashSet<_>>();
    let mut edges = graph_edges_from_dependencies(&dependencies, &visible);
    if options.hide_isolated {
        let connected = edges
            .iter()
            .flat_map(|edge| [edge.source_task_id.clone(), edge.target_task_id.clone()])
            .collect::<HashSet<_>>();
        node_ids.retain(|task_id| connected.contains(task_id));
        visible = node_ids.iter().cloned().collect();
        edges = graph_edges_from_dependencies(&dependencies, &visible);
    }
    let roles = node_ids
        .iter()
        .map(|task_id| {
            let role = if active_ids.contains(task_id) {
                TaskGraphNodeRole::Active
            } else {
                TaskGraphNodeRole::Context
            };
            (task_id.clone(), role)
        })
        .collect::<BTreeMap<_, _>>();
    let mut nodes = graph_nodes_from_ids(&node_ids, &all_tasks, &roles, true)?;
    for node in &mut nodes {
        node.context_only = !active_ids.contains(&node.task.id);
    }
    truncated |= nodes.len() > options.limit_nodes;
    let meta = TaskGraphMeta {
        depth: 0,
        context_depth: options.context_depth,
        generated_at: SystemClock.now_ms(),
        node_count: nodes.len(),
        edge_count: edges.len(),
        truncated,
        active_statuses,
        active_only: options.active_only,
        include_done_context: options.include_done_context,
        include_archived_context: options.include_archived_context,
        hide_isolated: options.hide_isolated,
        limit_nodes: options.limit_nodes,
    };
    Ok(BoardTaskMapRecord { nodes, edges, meta })
}

fn normalize_neighborhood_options(
    options: TaskNeighborhoodOptions,
) -> Result<TaskNeighborhoodOptions> {
    if options.depth != 1 {
        return Err(KanbanError::InvalidInput(
            "task neighborhood depth must be 1".into(),
        ));
    }
    Ok(TaskNeighborhoodOptions {
        depth: options.depth,
        limit_nodes: normalize_limit_nodes(options.limit_nodes),
        include_archived_context: options.include_archived_context,
    })
}

fn normalize_board_task_map_options(options: BoardTaskMapOptions) -> Result<BoardTaskMapOptions> {
    if options.context_depth > 1 {
        return Err(KanbanError::InvalidInput(
            "board task map context_depth must be 0 or 1".into(),
        ));
    }
    Ok(BoardTaskMapOptions {
        active_only: options.active_only,
        context_depth: options.context_depth,
        limit_nodes: normalize_limit_nodes(options.limit_nodes),
        include_done_context: options.include_done_context,
        include_archived_context: options.include_archived_context,
        hide_isolated: options.hide_isolated,
    })
}

fn normalize_limit_nodes(limit_nodes: usize) -> usize {
    limit_nodes.clamp(1, MAX_TASK_LIST_LIMIT)
}

fn tasks_by_id(conn: &Connection, board_id: &str) -> Result<BTreeMap<String, TaskRecord>> {
    let rows = super::query_tasks(conn, board_id)?;
    Ok(rows
        .into_iter()
        .map(|task| (task.id.clone(), task))
        .collect::<BTreeMap<_, _>>())
}

fn dependency_edges_for_board(conn: &Connection, board_id: &str) -> Result<Vec<DependencyEdge>> {
    all(
        conn,
        "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id=?1 ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
        params![board_id],
        |row| {
            Ok(DependencyEdge {
                parent_task_id: row.get(0)?,
                child_task_id: row.get(1)?,
            })
        },
    )
}

fn context_task_allowed(
    tasks: &BTreeMap<String, TaskRecord>,
    task_id: &str,
    include_archived_context: bool,
    include_done_context: bool,
) -> bool {
    let Some(task) = tasks.get(task_id) else {
        return false;
    };
    if !include_archived_context && task.archived_at.is_some() {
        return false;
    }
    if !include_done_context && task.status == TaskStatus::Done {
        return false;
    }
    true
}

fn maybe_insert_context(
    node_ids: &mut BTreeSet<String>,
    tasks: &BTreeMap<String, TaskRecord>,
    task_id: &str,
    include_done_context: bool,
    include_archived_context: bool,
) {
    if context_task_allowed(
        tasks,
        task_id,
        include_archived_context,
        include_done_context,
    ) {
        node_ids.insert(task_id.to_owned());
    }
}

fn limit_node_ids(
    node_ids: BTreeSet<String>,
    tasks: &BTreeMap<String, TaskRecord>,
    limit_nodes: usize,
) -> (BTreeSet<String>, bool) {
    if node_ids.len() <= limit_nodes {
        return (node_ids, false);
    }
    let mut ids = node_ids.into_iter().collect::<Vec<_>>();
    ids.sort_by_key(|task_id| {
        tasks
            .get(task_id)
            .map(|task| {
                (
                    status_rank(task.status),
                    task.position,
                    task.priority,
                    task.seq,
                )
            })
            .unwrap_or((999, i64::MAX, i64::MAX, i64::MAX))
    });
    (ids.into_iter().take(limit_nodes).collect(), true)
}

fn limit_node_ids_preserving(
    mut node_ids: BTreeSet<String>,
    tasks: &BTreeMap<String, TaskRecord>,
    limit_nodes: usize,
    required_ids: &[String],
) -> (BTreeSet<String>, bool) {
    if node_ids.len() <= limit_nodes {
        return (node_ids, false);
    }

    let mut limited = BTreeSet::new();
    for task_id in required_ids {
        if node_ids.remove(task_id) {
            limited.insert(task_id.clone());
        }
    }

    let remaining_limit = limit_nodes.saturating_sub(limited.len());
    let (remaining, _) = limit_node_ids(node_ids, tasks, remaining_limit);
    limited.extend(remaining);
    (limited, true)
}

fn graph_nodes_from_ids(
    node_ids: &BTreeSet<String>,
    tasks: &BTreeMap<String, TaskRecord>,
    roles: &BTreeMap<String, TaskGraphNodeRole>,
    context_only: bool,
) -> Result<Vec<TaskGraphNodeRecord>> {
    node_ids
        .iter()
        .map(|task_id| {
            let task = tasks.get(task_id).cloned().ok_or_else(|| {
                KanbanError::Storage(format!("graph node task missing: {task_id}"))
            })?;
            Ok(TaskGraphNodeRecord {
                role: roles
                    .get(task_id)
                    .copied()
                    .unwrap_or(TaskGraphNodeRole::Context),
                context_only,
                task,
            })
        })
        .collect()
}

fn graph_edges_from_dependencies(
    dependencies: &[DependencyEdge],
    visible: &HashSet<String>,
) -> Vec<TaskGraphEdgeRecord> {
    dependencies
        .iter()
        .filter(|edge| {
            visible.contains(&edge.parent_task_id) && visible.contains(&edge.child_task_id)
        })
        .map(|edge| TaskGraphEdgeRecord {
            id: format!("dependency:{}->{}", edge.parent_task_id, edge.child_task_id),
            source_task_id: edge.parent_task_id.clone(),
            target_task_id: edge.child_task_id.clone(),
            kind: TaskGraphEdgeKind::Dependency,
            required: true,
            blocking: true,
        })
        .collect()
}

fn active_statuses() -> Vec<TaskStatus> {
    vec![
        TaskStatus::Triage,
        TaskStatus::Todo,
        TaskStatus::Scheduled,
        TaskStatus::Ready,
        TaskStatus::Running,
        TaskStatus::Blocked,
        TaskStatus::Review,
    ]
}

fn status_rank(status: TaskStatus) -> i64 {
    match status {
        TaskStatus::Triage => 10,
        TaskStatus::Todo => 20,
        TaskStatus::Scheduled => 30,
        TaskStatus::Ready => 40,
        TaskStatus::Running => 50,
        TaskStatus::Blocked => 60,
        TaskStatus::Review => 70,
        TaskStatus::Done => 80,
        TaskStatus::Archived => 90,
    }
}
