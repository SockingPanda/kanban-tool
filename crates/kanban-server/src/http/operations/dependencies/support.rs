use crate::error::ApiError;
use crate::http::operations::tasks::support::{api_task, api_task_status};
use kanban_protocol::{ApiDependencies, ApiDependencyEdge, ApiDependencyTask};
use kanban_service::TaskRecord;

pub(crate) fn api_dependencies(
    dependencies: kanban_service::DependencySnapshotRecord,
) -> Result<ApiDependencies, ApiError> {
    Ok(ApiDependencies {
        task: api_dependency_task(&dependencies.task),
        parents: dependencies
            .parents
            .into_iter()
            .map(api_task)
            .collect::<Result<Vec<_>, _>>()?,
        children: dependencies
            .children
            .into_iter()
            .map(api_task)
            .collect::<Result<Vec<_>, _>>()?,
        edges: dependencies
            .edges
            .iter()
            .map(|edge| ApiDependencyEdge {
                parent: api_dependency_task(&edge.parent),
                child: api_dependency_task(&edge.child),
            })
            .collect(),
    })
}

pub(super) fn api_dependency_task(task: &TaskRecord) -> ApiDependencyTask {
    ApiDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: api_task_status(task.status),
    }
}
