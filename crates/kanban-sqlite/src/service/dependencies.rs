use crate::db::connect_file;

use super::{
    DependencyEdgeRecord, DependencySnapshot, DependencyTaskRecord, TaskRecord, board_id,
    delete_dependency_relation, dependency_parent_is_satisfied, ensure_board_active,
    get_task_by_id, guarded_set_status, insert_event, recompute_ready_status, resolve_task,
    storage, upsert_dependency_relation, with_immediate_tx,
};

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, is_active_recomputable_status,
};

use rusqlite::{Connection, params};

use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddDependencyOutcome {
    Added,
    AlreadyExists,
}

pub fn add_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    add_dependency_with_outcome(path, board, actor, parent_ref, child_ref).map(|_| ())
}

pub fn add_dependency_with_outcome(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<AddDependencyOutcome> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let child = resolve_task(&conn, &board_id, child_ref)?;
        add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)
    })
}

pub(crate) fn add_dependency_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    parent: &TaskRecord,
    child: &TaskRecord,
    now: i64,
) -> Result<AddDependencyOutcome> {
    if parent.id == child.id {
        return Err(KanbanError::InvalidInput(
            "dependency cannot point to itself".into(),
        ));
    }
    if parent.board_id != board_id || child.board_id != board_id {
        return Err(KanbanError::InvalidInput(
            "cross-board dependency is not allowed".into(),
        ));
    }
    if has_path(conn, &child.id, &parent.id)? {
        return Err(KanbanError::InvalidInput(
            "dependency cycle detected".into(),
        ));
    }
    if child.status == TaskStatus::Running && !dependency_parent_is_satisfied(parent.status) {
        return Err(KanbanError::InvalidTransition(
            "cannot add incomplete dependency to running task".into(),
        ));
    }
    let inserted = conn
        .execute(
        "INSERT OR IGNORE INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![board_id, parent.id, child.id, now],
    )
        .map_err(storage)?;
    if inserted == 0 {
        return Ok(AddDependencyOutcome::AlreadyExists);
    }
    upsert_dependency_relation(conn, &parent.id, &child.id, now)?;
    let fresh_child = get_task_by_id(conn, board_id, &child.id)?;
    if is_active_recomputable_status(fresh_child.status) {
        let target = recompute_ready_status(conn, &fresh_child, now)?;
        if target != fresh_child.status && target != TaskStatus::Ready {
            guarded_set_status(
                conn,
                board_id,
                &fresh_child,
                target,
                actor,
                "task.recomputed",
                now,
            )?;
        }
    }
    let payload = json!({ "parent_task_id": parent.id }).to_string();
    insert_event(
        conn,
        board_id,
        Some(&child.id),
        None,
        "dependency.added",
        actor,
        &payload,
        now,
    )?;
    Ok(AddDependencyOutcome::Added)
}

pub fn remove_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    let child = resolve_task(&conn, &board_id, child_ref)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        conn.execute(
            "DELETE FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
            params![parent.id, child.id],
        )
        .map_err(storage)?;
        delete_dependency_relation(&conn, &parent.id, &child.id)?;
        let fresh_child = get_task_by_id(&conn, &board_id, &child.id)?;
        if matches!(
            fresh_child.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            let target = recompute_ready_status(&conn, &fresh_child, now)?;
            if target != fresh_child.status && target != TaskStatus::Ready {
                guarded_set_status(
                    &conn,
                    &board_id,
                    &fresh_child,
                    target,
                    actor,
                    "task.recomputed",
                    now,
                )?;
            }
        }
        let payload = json!({ "parent_task_id": parent.id }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&child.id),
            None,
            "dependency.removed",
            actor,
            &payload,
            now,
        )
    })
}

pub fn dependency_edge(
    path: impl AsRef<Path>,
    board: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<DependencyEdgeRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    dependency_edge_conn(&conn, &board_id, parent_ref, child_ref)
}

pub fn dependency_snapshot(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<DependencySnapshot> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    dependency_snapshot_conn(&conn, &board_id, task_ref)
}

pub fn list_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Vec<(String, String)>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    list_dependency_ids_conn(&conn, &task.id)
}

fn dependency_edge_conn(
    conn: &Connection,
    board_id: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<DependencyEdgeRecord> {
    let parent = resolve_task(conn, board_id, parent_ref)?;
    let child = resolve_task(conn, board_id, child_ref)?;
    Ok(edge_record(parent, child))
}

fn dependency_snapshot_conn(
    conn: &Connection,
    board_id: &str,
    task_ref: &str,
) -> Result<DependencySnapshot> {
    let task = resolve_task(conn, board_id, task_ref)?;
    let task_id = task.id.clone();
    let edges = dependency_edges_for_task_conn(conn, board_id, &task_id)?;
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for edge in &edges {
        if edge.child.id == task_id {
            parents.push(edge.parent.clone());
        }
        if edge.parent.id == task_id {
            children.push(edge.child.clone());
        }
    }
    Ok(DependencySnapshot {
        task: DependencyTaskRecord::from(task),
        parents,
        children,
        edges,
    })
}

fn dependency_edges_for_task_conn(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<Vec<DependencyEdgeRecord>> {
    list_dependency_ids_conn(conn, task_id)?
        .into_iter()
        .map(|(parent_id, child_id)| {
            let parent = get_task_by_id(conn, board_id, &parent_id)?;
            let child = get_task_by_id(conn, board_id, &child_id)?;
            Ok(edge_record(parent, child))
        })
        .collect()
}

fn edge_record(parent: TaskRecord, child: TaskRecord) -> DependencyEdgeRecord {
    DependencyEdgeRecord {
        parent: DependencyTaskRecord::from(parent),
        child: DependencyTaskRecord::from(child),
    }
}

fn list_dependency_ids_conn(conn: &Connection, task_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT parent_task_id, child_task_id FROM task_dependencies \
             WHERE parent_task_id=?1 OR child_task_id=?1 \
             ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([task_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub(crate) fn count_dependency_cycles(conn: &Connection) -> Result<i64> {
    let mut stmt = conn
        .prepare("SELECT parent_task_id, child_task_id FROM task_dependencies")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    for row in rows {
        let (parent, child) = row.map_err(storage)?;
        nodes.insert(parent.clone());
        nodes.insert(child.clone());
        graph.entry(parent).or_default().push(child);
    }
    Ok(count_cyclic_components(&nodes, &graph))
}

pub(crate) fn count_cyclic_components(
    nodes: &HashSet<String>,
    graph: &HashMap<String, Vec<String>>,
) -> i64 {
    struct Tarjan<'a> {
        graph: &'a HashMap<String, Vec<String>>,
        index: usize,
        stack: Vec<String>,
        indices: HashMap<String, usize>,
        lowlinks: HashMap<String, usize>,
        on_stack: HashSet<String>,
        cycles: i64,
    }

    impl Tarjan<'_> {
        fn visit(&mut self, node: &str) {
            self.indices.insert(node.to_owned(), self.index);
            self.lowlinks.insert(node.to_owned(), self.index);
            self.index += 1;
            self.stack.push(node.to_owned());
            self.on_stack.insert(node.to_owned());

            for next in self.graph.get(node).into_iter().flatten() {
                if !self.indices.contains_key(next) {
                    self.visit(next);
                    let node_low = self.lowlinks[node].min(self.lowlinks[next]);
                    self.lowlinks.insert(node.to_owned(), node_low);
                } else if self.on_stack.contains(next) {
                    let node_low = self.lowlinks[node].min(self.indices[next]);
                    self.lowlinks.insert(node.to_owned(), node_low);
                }
            }

            if self.lowlinks[node] == self.indices[node] {
                let mut component_len = 0;
                while let Some(member) = self.stack.pop() {
                    self.on_stack.remove(&member);
                    component_len += 1;
                    if member == node {
                        break;
                    }
                }
                if component_len > 1
                    || self
                        .graph
                        .get(node)
                        .is_some_and(|edges| edges.iter().any(|next| next == node))
                {
                    self.cycles += 1;
                }
            }
        }
    }

    let mut tarjan = Tarjan {
        graph,
        index: 0,
        stack: Vec::new(),
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        on_stack: HashSet::new(),
        cycles: 0,
    };
    for node in nodes {
        if !tarjan.indices.contains_key(node) {
            tarjan.visit(node);
        }
    }
    tarjan.cycles
}

pub(crate) fn has_path(conn: &Connection, start: &str, goal: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "WITH RECURSIVE walk(id) AS (SELECT child_task_id FROM task_dependencies WHERE parent_task_id=?1 UNION SELECT d.child_task_id FROM task_dependencies d JOIN walk w ON d.parent_task_id=w.id) SELECT COUNT(*) FROM walk WHERE id=?2",
        params![start, goal], |r| r.get(0)).map_err(storage)?;
    Ok(count > 0)
}
