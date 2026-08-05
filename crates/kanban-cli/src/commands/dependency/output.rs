use kanban_contract::{ApiDependencies, ApiDependencyEdge, ApiDependencyTask, ApiTask};

pub(super) fn cli_dependency_task(task: &ApiTask) -> kanban_contract::CliDependencyTask {
    kanban_contract::CliDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: task.status,
    }
}

pub(super) fn api_dependency_task(task: &ApiTask) -> ApiDependencyTask {
    ApiDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: task.status,
    }
}

pub(super) fn cli_dependency_task_compact(
    task: &ApiDependencyTask,
) -> kanban_contract::CliDependencyTask {
    kanban_contract::CliDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: task.status,
    }
}

pub(super) fn cli_dependency_edge(edge: &ApiDependencyEdge) -> kanban_contract::CliDependencyEdge {
    kanban_contract::CliDependencyEdge {
        parent: cli_dependency_task_compact(&edge.parent),
        child: cli_dependency_task_compact(&edge.child),
    }
}

pub(super) fn cli_dependency_snapshot(
    dependencies: &ApiDependencies,
) -> kanban_contract::CliDependencySnapshot {
    kanban_contract::CliDependencySnapshot {
        task: cli_dependency_task_compact(&dependencies.task),
        parents: dependencies
            .parents
            .iter()
            .map(cli_dependency_task)
            .collect(),
        children: dependencies
            .children
            .iter()
            .map(cli_dependency_task)
            .collect(),
        edges: dependencies.edges.iter().map(cli_dependency_edge).collect(),
    }
}
