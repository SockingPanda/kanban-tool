use kanban_application::{
    TaskList, TaskListOptions as ApplicationTaskListOptions,
    TaskListPage as ApplicationTaskListPage, TaskListSort as ApplicationTaskListSort,
    TaskPlanFilter as ApplicationTaskPlanFilter,
};
use kanban_core::Result;
use kanban_store_turso::{
    TaskListOptions as StoreTaskListOptions, TaskListSort as StoreTaskListSort,
    TaskPlanFilter as StoreTaskPlanFilter,
};

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskList for TursoApplicationStore {
    async fn list_tasks(
        &self,
        board: &str,
        options: ApplicationTaskListOptions,
    ) -> Result<ApplicationTaskListPage> {
        let page = self
            .store
            .list_tasks(
                board,
                StoreTaskListOptions {
                    statuses: options
                        .statuses
                        .into_iter()
                        .map(|status| status.as_str().to_owned())
                        .collect(),
                    priorities: options.priorities,
                    labels: options.labels,
                    include_archived: options.include_archived,
                    assignee: options.assignee,
                    q: options.query,
                    plan_filters: options
                        .plan_filters
                        .into_iter()
                        .map(store_plan_filter)
                        .collect(),
                    sort: store_task_sort(options.sort),
                    limit: options.limit,
                    offset: options.offset,
                },
            )
            .await
            .map_err(store_error)?;
        Ok(ApplicationTaskListPage {
            tasks: page
                .tasks
                .into_iter()
                .map(application_task)
                .collect::<Result<Vec<_>>>()?,
            total: page.total,
        })
    }
}

fn store_plan_filter(filter: ApplicationTaskPlanFilter) -> StoreTaskPlanFilter {
    match filter {
        ApplicationTaskPlanFilter::PlanNeeded => StoreTaskPlanFilter::PlanNeeded,
        ApplicationTaskPlanFilter::HasSteps => StoreTaskPlanFilter::HasSteps,
        ApplicationTaskPlanFilter::IncompleteRequiredSteps => {
            StoreTaskPlanFilter::IncompleteRequiredSteps
        }
    }
}

fn store_task_sort(sort: ApplicationTaskListSort) -> StoreTaskListSort {
    match sort {
        ApplicationTaskListSort::Seq => StoreTaskListSort::Seq,
        ApplicationTaskListSort::SeqDesc => StoreTaskListSort::SeqDesc,
        ApplicationTaskListSort::Title => StoreTaskListSort::Title,
        ApplicationTaskListSort::TitleDesc => StoreTaskListSort::TitleDesc,
        ApplicationTaskListSort::Status => StoreTaskListSort::Status,
        ApplicationTaskListSort::StatusDesc => StoreTaskListSort::StatusDesc,
        ApplicationTaskListSort::Position => StoreTaskListSort::Position,
        ApplicationTaskListSort::PositionDesc => StoreTaskListSort::PositionDesc,
        ApplicationTaskListSort::Priority => StoreTaskListSort::Priority,
        ApplicationTaskListSort::PriorityDesc => StoreTaskListSort::PriorityDesc,
        ApplicationTaskListSort::Assignee => StoreTaskListSort::Assignee,
        ApplicationTaskListSort::AssigneeDesc => StoreTaskListSort::AssigneeDesc,
        ApplicationTaskListSort::ScheduledAt => StoreTaskListSort::ScheduledAt,
        ApplicationTaskListSort::ScheduledAtDesc => StoreTaskListSort::ScheduledAtDesc,
        ApplicationTaskListSort::DueAt => StoreTaskListSort::DueAt,
        ApplicationTaskListSort::DueAtDesc => StoreTaskListSort::DueAtDesc,
        ApplicationTaskListSort::CreatedAt => StoreTaskListSort::CreatedAt,
        ApplicationTaskListSort::CreatedAtDesc => StoreTaskListSort::CreatedAtDesc,
        ApplicationTaskListSort::UpdatedAt => StoreTaskListSort::UpdatedAt,
        ApplicationTaskListSort::UpdatedAtDesc => StoreTaskListSort::UpdatedAtDesc,
    }
}
