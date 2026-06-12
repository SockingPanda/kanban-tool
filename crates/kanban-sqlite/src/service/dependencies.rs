use crate::connect_file;

use super::{
    TaskRecord, board_id, delete_dependency_relation, ensure_board_active, get_task_by_id,
    guarded_set_status, insert_event, recompute_ready_status, resolve_task, storage,
    upsert_dependency_relation, with_immediate_tx,
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
    if child.status == TaskStatus::Running && parent.status != TaskStatus::Done {
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
        if target != fresh_child.status {
            guarded_set_status(
                conn,
                board_id,
                &fresh_child,
                target,
                actor,
                if target == TaskStatus::Ready {
                    "task.promoted"
                } else {
                    "task.recomputed"
                },
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
            if target != fresh_child.status {
                guarded_set_status(
                    &conn,
                    &board_id,
                    &fresh_child,
                    target,
                    actor,
                    if target == TaskStatus::Ready {
                        "task.promoted"
                    } else {
                        "task.recomputed"
                    },
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

pub fn list_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Vec<(String, String)>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let mut stmt = conn.prepare("SELECT parent_task_id, child_task_id FROM task_dependencies WHERE parent_task_id=?1 OR child_task_id=?1 ORDER BY created_at ASC").map_err(storage)?;
    let rows = stmt
        .query_map([task.id], |row| Ok((row.get(0)?, row.get(1)?)))
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
