use kanban_application::{
    AddTaskLabelsRecord as ApplicationAddTaskLabelsRecord, AddTaskLabelsRecordInput,
    BoardLabelCreate, BoardLabelList, CreateLabelRecord, LabelRecord as ApplicationLabel,
    TaskLabelAdd, TaskLabelList, TaskLabelRemove,
};
use kanban_core::Result;
use kanban_store_turso::{
    AddTaskLabelsInput as StoreAddTaskLabelsInput, CreateLabelInput as StoreCreateLabelInput,
    RemoveTaskLabelInput as StoreRemoveTaskLabelInput,
};

use crate::adapter::{
    TursoApplicationStore, application_add_task_labels, application_label, application_task,
    store_error,
};

impl BoardLabelList for TursoApplicationStore {
    async fn list_board_labels(&self, board: &str) -> Result<Vec<ApplicationLabel>> {
        self.store
            .list_board_labels(board)
            .await
            .map_err(store_error)
            .map(|labels| labels.into_iter().map(application_label).collect())
    }
}

impl BoardLabelCreate for TursoApplicationStore {
    async fn create_board_label(
        &self,
        board: &str,
        input: CreateLabelRecord,
    ) -> Result<ApplicationLabel> {
        self.store
            .create_board_label(
                board,
                StoreCreateLabelInput {
                    id: input.id,
                    name: input.name,
                    color: input.color,
                    created_at: input.created_at,
                },
            )
            .await
            .map_err(store_error)
            .map(application_label)
    }
}

impl TaskLabelList for TursoApplicationStore {
    async fn list_task_labels(&self, task_id: &str) -> Result<Vec<ApplicationLabel>> {
        self.store
            .list_task_labels(task_id)
            .await
            .map_err(store_error)
            .map(|labels| labels.into_iter().map(application_label).collect())
    }
}

impl TaskLabelAdd for TursoApplicationStore {
    async fn add_task_labels(
        &self,
        task_id: &str,
        input: AddTaskLabelsRecordInput,
    ) -> Result<ApplicationAddTaskLabelsRecord> {
        self.store
            .add_task_labels(
                task_id,
                StoreAddTaskLabelsInput {
                    names: input.names,
                    label_ids: input.label_ids,
                    event_ids: input.event_ids,
                    create_missing: input.create_missing,
                    actor: input.actor,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_add_task_labels)
    }
}

impl TaskLabelRemove for TursoApplicationStore {
    async fn remove_task_label(
        &self,
        task_id: &str,
        input: kanban_application::RemoveTaskLabelRecord,
    ) -> Result<kanban_application::TaskRecord> {
        self.store
            .remove_task_label(
                task_id,
                StoreRemoveTaskLabelInput {
                    label_ref: input.label_ref,
                    event_id: input.event_id,
                    actor: input.actor,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
