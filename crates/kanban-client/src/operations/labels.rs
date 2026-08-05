use kanban_contract::{
    AddTaskLabelRequest, AddTaskLabelResponse, ApiLabel, ApiTask, CreateBoardLabelRequest,
    CreateBoardLabelResponse, ListBoardLabelsResponse, ListTaskLabelsResponse,
    RemoveTaskLabelResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_board_labels(&self, board: &str) -> Result<Vec<ApiLabel>, ClientError> {
        let response: ListBoardLabelsResponse = self.get(&format!(
            "/api/v1/boards/{}/labels",
            encode_path_segment(board.trim())
        ))?;
        Ok(response.data)
    }

    pub fn create_board_label(
        &self,
        board: &str,
        request: &CreateBoardLabelRequest,
    ) -> Result<ApiLabel, ClientError> {
        let response: CreateBoardLabelResponse = self.post(
            &format!(
                "/api/v1/boards/{}/labels",
                encode_path_segment(board.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn list_task_labels(&self, task_id: &str) -> Result<Vec<ApiLabel>, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: ListTaskLabelsResponse = self.get(&format!(
            "/api/v1/tasks/{}/labels",
            encode_path_segment(task_id)
        ))?;
        Ok(response.data)
    }

    pub fn list_task_labels_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<Vec<ApiLabel>, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.list_task_labels(&task_id)
    }

    pub fn add_task_labels(
        &self,
        task_id: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        let task_id = require_task_id(task_id)?;
        request
            .label_names()
            .map_err(|error| ClientError::InvalidInput(error.to_owned()))?;
        self.post(
            &format!("/api/v1/tasks/{}/labels", encode_path_segment(task_id)),
            request,
        )
    }

    pub fn add_task_label(
        &self,
        task_id: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        self.add_task_labels(task_id, request)
    }

    pub fn add_task_labels_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.add_task_labels(&task_id, request)
    }

    pub fn add_task_label_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        self.add_task_labels_by_selector(board, selector, request)
    }

    pub fn remove_task_label(&self, task_id: &str, label_id: &str) -> Result<ApiTask, ClientError> {
        let task_id = require_task_id(task_id)?;
        let label_id = label_id.trim();
        if label_id.is_empty() {
            return Err(ClientError::InvalidInput("label id is required".to_owned()));
        }
        let response: RemoveTaskLabelResponse = self.delete(&format!(
            "/api/v1/tasks/{}/labels/{}",
            encode_path_segment(task_id),
            encode_path_segment(label_id)
        ))?;
        Ok(response.data)
    }

    pub fn remove_task_label_by_selector(
        &self,
        board: &str,
        selector: &str,
        label_id: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.remove_task_label(&task_id, label_id)
    }
}

fn require_task_id(task_id: &str) -> Result<&str, ClientError> {
    let task_id = task_id.trim();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "task selector must resolve to a global t_... id".to_owned(),
        ));
    }
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[test]
    fn label_client_requires_global_task_ids() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client
                .list_task_labels("default#1")
                .expect_err("board-local selector must be resolved first")
                .code(),
            "invalid_input"
        );
    }
}
