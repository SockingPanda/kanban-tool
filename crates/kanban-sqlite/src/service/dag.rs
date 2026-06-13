use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, TaskStatus};
use rusqlite::params;

use crate::connect_file;

use super::{
    DagAdjacency, DagAncestors, DagBoardSnapshot, DagDerivedGraph, DagEdge, DagNode, DagRawGraph,
    DagSnapshot, DagSnapshotMeta, DagTaskReason, board_id, get_board_conn, query_tasks,
    resolve_task, storage,
};

pub fn dag_snapshot(path: impl AsRef<Path>, board: &str) -> Result<DagSnapshot> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let board = get_board_conn(&conn, &board_id)?;
    let mut nodes = query_tasks(&conn, &board_id)?
        .into_iter()
        .filter(|task| task.status != TaskStatus::Archived && task.archived_at.is_none())
        .map(|task| DagNode {
            why: format!("{} 当前状态为 {}", task.task_ref, task.status.as_str()),
            id: task.id,
            task_ref: task.task_ref,
            seq: task.seq,
            title: task.title,
            status: task.status,
            priority: task.priority,
            due_at: task.due_at,
            scheduled_at: task.scheduled_at,
            created_at: task.created_at,
            archived_at: task.archived_at,
        })
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mut fan_out = node_ids
        .iter()
        .map(|task_id| (task_id.clone(), 0_usize))
        .collect::<HashMap<_, _>>();
    let mut blocked_by_map = node_ids
        .iter()
        .map(|task_id| (task_id.clone(), Vec::<String>::new()))
        .collect::<HashMap<_, _>>();
    let mut unblocks_map = node_ids
        .iter()
        .map(|task_id| (task_id.clone(), Vec::<String>::new()))
        .collect::<HashMap<_, _>>();

    let mut stmt = conn
        .prepare(
            "SELECT parent_task_id, child_task_id FROM task_dependencies \
             WHERE board_id=?1 ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
        )
        .map_err(storage)?;
    let edge_rows = stmt
        .query_map(params![board_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut edges = Vec::new();
    for row in edge_rows {
        let (parent, child) = row.map_err(storage)?;
        if !node_ids.contains(&parent) || !node_ids.contains(&child) {
            continue;
        }
        edges.push(DagEdge {
            why: format!("{parent} 必须先完成，{child} 才能执行"),
            parent: parent.clone(),
            child: child.clone(),
        });
        *fan_out.entry(parent.clone()).or_default() += 1;
        blocked_by_map
            .entry(child.clone())
            .or_default()
            .push(parent.clone());
        unblocks_map.entry(parent).or_default().push(child);
    }

    let status_by_id = nodes
        .iter()
        .map(|node| (node.id.clone(), node.status))
        .collect::<HashMap<_, _>>();
    let ref_by_id = nodes
        .iter()
        .map(|node| (node.id.clone(), node.task_ref.clone()))
        .collect::<HashMap<_, _>>();

    for parents in blocked_by_map.values_mut() {
        parents.sort_by(|left, right| compare_ids_by_ref(left, right, &ref_by_id));
    }
    for children in unblocks_map.values_mut() {
        children.sort_by(|left, right| compare_ids_by_ref(left, right, &ref_by_id));
    }

    sort_nodes(&mut nodes, &fan_out);
    edges.sort_by(|left, right| {
        compare_ids_by_ref(&left.parent, &right.parent, &ref_by_id)
            .then_with(|| compare_ids_by_ref(&left.child, &right.child, &ref_by_id))
    });

    let actionable = nodes
        .iter()
        .filter(|node| matches!(node.status, TaskStatus::Todo | TaskStatus::Ready))
        .map(|node| {
            let parents = blocked_by_map.get(&node.id).cloned().unwrap_or_default();
            let unfinished = unfinished_parents(&parents, &status_by_id);
            DagTaskReason {
                task_id: node.id.clone(),
                task_ref: node.task_ref.clone(),
                why: if unfinished.is_empty() {
                    format!(
                        "{} 状态为 {}，没有未完成的前置依赖",
                        node.task_ref,
                        node.status.as_str()
                    )
                } else {
                    format!(
                        "{} 状态为 {}，等待未完成的前置依赖：{}",
                        node.task_ref,
                        node.status.as_str(),
                        refs_for(&unfinished, &ref_by_id).join(", ")
                    )
                },
            }
        })
        .collect::<Vec<_>>();

    let frontier = nodes
        .iter()
        .filter(|node| matches!(node.status, TaskStatus::Todo | TaskStatus::Ready))
        .filter(|node| {
            blocked_by_map
                .get(&node.id)
                .map(|parents| unfinished_parents(parents, &status_by_id).is_empty())
                .unwrap_or(true)
        })
        .map(|node| DagTaskReason {
            task_id: node.id.clone(),
            task_ref: node.task_ref.clone(),
            why: format!(
                "{} 是 frontier：状态为 {}，且前置依赖已完成或不存在",
                node.task_ref,
                node.status.as_str()
            ),
        })
        .collect::<Vec<_>>();

    let blocked_by = adjacency_from_map(&nodes, &blocked_by_map, &ref_by_id, true);
    let unblocks = adjacency_from_map(&nodes, &unblocks_map, &ref_by_id, false);

    Ok(DagSnapshot {
        board: DagBoardSnapshot {
            id: board.id,
            slug: board.slug,
            name: board.name,
        },
        snapshot: DagSnapshotMeta {
            generated_at: SystemClock.now_ms(),
            node_count: nodes.len(),
            edge_count: edges.len(),
            sort: vec![
                "priority asc".to_owned(),
                "due_at asc nulls last".to_owned(),
                "scheduled_at asc nulls last".to_owned(),
                "dependency fan-out desc".to_owned(),
                "created_at asc".to_owned(),
                "ref asc".to_owned(),
                "id asc".to_owned(),
            ],
        },
        raw: DagRawGraph { nodes, edges },
        derived: DagDerivedGraph {
            blocked_by,
            unblocks,
            actionable,
            frontier,
        },
    })
}

pub fn dag_ancestors(path: impl AsRef<Path>, board: &str, task_ref: &str) -> Result<DagAncestors> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let snapshot = dag_snapshot(path, board)?;
    let node_by_id = snapshot
        .raw
        .nodes
        .iter()
        .cloned()
        .map(|node| (node.id.clone(), node))
        .collect::<HashMap<_, _>>();
    let target = node_by_id
        .get(&task.id)
        .cloned()
        .ok_or_else(|| KanbanError::NotFound(format!("task {task_ref} in active DAG snapshot")))?;

    let mut parents_by_child: HashMap<String, Vec<String>> = HashMap::new();
    let mut children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &snapshot.raw.edges {
        parents_by_child
            .entry(edge.child.clone())
            .or_default()
            .push(edge.parent.clone());
        children_by_parent
            .entry(edge.parent.clone())
            .or_default()
            .push(edge.child.clone());
    }

    let node_rank = snapshot
        .raw
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect::<HashMap<_, _>>();
    for parents in parents_by_child.values_mut() {
        parents.sort_by_key(|id| node_rank.get(id).copied().unwrap_or(usize::MAX));
    }
    for children in children_by_parent.values_mut() {
        children.sort_by_key(|id| node_rank.get(id).copied().unwrap_or(usize::MAX));
    }

    let mut subset = HashSet::new();
    let mut stack = vec![target.id.clone()];
    while let Some(task_id) = stack.pop() {
        if !subset.insert(task_id.clone()) {
            continue;
        }
        if let Some(parents) = parents_by_child.get(&task_id) {
            for parent in parents.iter().rev() {
                stack.push(parent.clone());
            }
        }
    }

    let ordered_ids =
        topological_subset(&subset, &children_by_parent, &parents_by_child, &node_rank);
    let nodes = ordered_ids
        .iter()
        .filter_map(|task_id| node_by_id.get(task_id).cloned())
        .collect::<Vec<_>>();
    let ordered_refs = nodes
        .iter()
        .map(|node| node.task_ref.clone())
        .collect::<Vec<_>>();
    let edges = snapshot
        .raw
        .edges
        .into_iter()
        .filter(|edge| subset.contains(&edge.parent) && subset.contains(&edge.child))
        .collect::<Vec<_>>();

    Ok(DagAncestors {
        target,
        nodes,
        edges,
        ordered_refs,
        generated_at: SystemClock.now_ms(),
    })
}

fn topological_subset(
    subset: &HashSet<String>,
    children_by_parent: &HashMap<String, Vec<String>>,
    parents_by_child: &HashMap<String, Vec<String>>,
    node_rank: &HashMap<String, usize>,
) -> Vec<String> {
    let mut in_degree = subset
        .iter()
        .map(|task_id| {
            let count = parents_by_child
                .get(task_id)
                .into_iter()
                .flatten()
                .filter(|parent| subset.contains(*parent))
                .count();
            (task_id.clone(), count)
        })
        .collect::<HashMap<_, _>>();
    let mut ready = in_degree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(task_id, _)| task_id.clone())
        .collect::<Vec<_>>();
    sort_ids_by_rank(&mut ready, node_rank);

    let mut ordered = Vec::new();
    while let Some(task_id) = ready.first().cloned() {
        ready.remove(0);
        ordered.push(task_id.clone());
        if let Some(children) = children_by_parent.get(&task_id) {
            for child in children.iter().filter(|child| subset.contains(*child)) {
                if let Some(count) = in_degree.get_mut(child) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(child.clone());
                    }
                }
            }
            sort_ids_by_rank(&mut ready, node_rank);
        }
    }

    if ordered.len() != subset.len() {
        let mut remaining = subset
            .iter()
            .filter(|task_id| !ordered.contains(*task_id))
            .cloned()
            .collect::<Vec<_>>();
        sort_ids_by_rank(&mut remaining, node_rank);
        ordered.extend(remaining);
    }

    ordered
}

fn sort_ids_by_rank(ids: &mut [String], node_rank: &HashMap<String, usize>) {
    ids.sort_by_key(|id| node_rank.get(id).copied().unwrap_or(usize::MAX));
}

fn adjacency_from_map(
    nodes: &[DagNode],
    map: &HashMap<String, Vec<String>>,
    ref_by_id: &HashMap<String, String>,
    incoming: bool,
) -> Vec<DagAdjacency> {
    nodes
        .iter()
        .filter_map(|node| {
            let tasks = map.get(&node.id).cloned().unwrap_or_default();
            if tasks.is_empty() {
                return None;
            }
            let refs = refs_for(&tasks, ref_by_id);
            Some(DagAdjacency {
                task_id: node.id.clone(),
                why: if incoming {
                    format!("{} 被以下前置任务阻塞：{}", node.task_ref, refs.join(", "))
                } else {
                    format!("{} 解除后会放行：{}", node.task_ref, refs.join(", "))
                },
                tasks,
            })
        })
        .collect()
}

fn unfinished_parents(
    parents: &[String],
    status_by_id: &HashMap<String, TaskStatus>,
) -> Vec<String> {
    parents
        .iter()
        .filter(|parent| status_by_id.get(*parent) != Some(&TaskStatus::Done))
        .cloned()
        .collect()
}

fn refs_for(task_ids: &[String], ref_by_id: &HashMap<String, String>) -> Vec<String> {
    task_ids
        .iter()
        .map(|task_id| {
            ref_by_id
                .get(task_id)
                .cloned()
                .unwrap_or_else(|| task_id.clone())
        })
        .collect()
}

fn sort_nodes(nodes: &mut [DagNode], fan_out: &HashMap<String, usize>) {
    nodes.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| nulls_last(left.due_at, right.due_at))
            .then_with(|| nulls_last(left.scheduled_at, right.scheduled_at))
            .then_with(|| {
                fan_out
                    .get(&right.id)
                    .unwrap_or(&0)
                    .cmp(fan_out.get(&left.id).unwrap_or(&0))
            })
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.task_ref.cmp(&right.task_ref))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn nulls_last(left: Option<i64>, right: Option<i64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn compare_ids_by_ref(
    left: &str,
    right: &str,
    ref_by_id: &HashMap<String, String>,
) -> std::cmp::Ordering {
    refs_for(&[left.to_owned()], ref_by_id)[0]
        .cmp(&refs_for(&[right.to_owned()], ref_by_id)[0])
        .then_with(|| left.cmp(right))
}
