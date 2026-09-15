use kanban_protocol::{
    AddTaskLabelRequest, AddTaskLabelResponse, ApiLabel, ApiTask, BootstrapTaskLabelRequest,
    BootstrapTaskLabelResponse, CreateBoardLabelRequest, DeleteBoardLabelResult,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_board_labels(&self, board: &str) -> Result<Vec<ApiLabel>, ClientError> {
        let response: kanban_protocol::ListBoardLabelsResponse = rpc!(
            self,
            list_board_labels,
            ListBoardLabelsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.trim().to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn create_board_label(
        &self,
        board: &str,
        request: &CreateBoardLabelRequest,
    ) -> Result<ApiLabel, ClientError> {
        let response: kanban_protocol::CreateBoardLabelResponse = rpc!(
            self,
            create_board_label,
            CreateBoardLabelRequest,
            kanban_protocol::BoardLabelPath {
                board: board.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn delete_board_label(
        &self,
        board: &str,
        label_ref: &str,
        force: bool,
    ) -> Result<DeleteBoardLabelResult, ClientError> {
        let board = require_board(board)?;
        let label_ref = require_label_ref(label_ref)?;
        let response: kanban_protocol::DeleteBoardLabelResponse = rpc!(
            self,
            delete_board_label,
            DeleteBoardLabelRequest,
            kanban_protocol::DeleteBoardLabelPath {
                board: board.to_owned(),
                label_id: label_ref.to_owned()
            },
            kanban_protocol::DeleteBoardLabelQuery { force },
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_task_labels(&self, task_id: &str) -> Result<Vec<ApiLabel>, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: kanban_protocol::ListTaskLabelsResponse = rpc!(
            self,
            list_task_labels,
            ListTaskLabelsRequest,
            kanban_protocol::ListTaskLabelsPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_task_labels_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<Vec<ApiLabel>, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.list_task_labels(&task_id).await
    }

    pub async fn add_task_labels(
        &self,
        task_id: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        let task_id = require_task_id(task_id)?;
        request
            .label_names()
            .map_err(|error| ClientError::InvalidInput(error.to_owned()))?;
        let response: kanban_protocol::AddTaskLabelResponse = rpc!(
            self,
            add_task_label,
            AddTaskLabelRequest,
            kanban_protocol::AddTaskLabelPath {
                task_id: task_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn add_task_label(
        &self,
        task_id: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        self.add_task_labels(task_id, request).await
    }

    pub async fn add_task_labels_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.add_task_labels(&task_id, request).await
    }

    pub async fn add_task_label_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &AddTaskLabelRequest,
    ) -> Result<AddTaskLabelResponse, ClientError> {
        self.add_task_labels_by_selector(board, selector, request)
            .await
    }

    pub async fn remove_task_label(
        &self,
        task_id: &str,
        label_id: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = require_task_id(task_id)?;
        let label_id = label_id.trim();
        if label_id.is_empty() {
            return Err(ClientError::InvalidInput("必须提供 label ID".to_owned()));
        }
        let response: kanban_protocol::RemoveTaskLabelResponse = rpc!(
            self,
            remove_task_label,
            RemoveTaskLabelRequest,
            kanban_protocol::RemoveTaskLabelPath {
                task_id: task_id.to_owned(),
                label_id: label_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn remove_task_label_by_selector(
        &self,
        board: &str,
        selector: &str,
        label_id: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.remove_task_label(&task_id, label_id).await
    }

    pub async fn bootstrap_task_label(
        &self,
        task_id: &str,
        request: &BootstrapTaskLabelRequest,
    ) -> Result<BootstrapTaskLabelResponse, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: kanban_protocol::BootstrapTaskLabelResponse = rpc!(
            self,
            bootstrap_task_label,
            BootstrapTaskLabelRequest,
            kanban_protocol::TaskLabelSurfacePath {
                task_id: task_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn bootstrap_task_label_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &BootstrapTaskLabelRequest,
    ) -> Result<BootstrapTaskLabelResponse, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.bootstrap_task_label(&task_id, request).await
    }
}

fn require_task_id(task_id: &str) -> Result<&str, ClientError> {
    let task_id = task_id.trim();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "任务选择器必须解析为全局 t_... ID".to_owned(),
        ));
    }
    Ok(task_id)
}

fn require_board(board: &str) -> Result<&str, ClientError> {
    let board = board.trim();
    if board.is_empty() {
        return Err(ClientError::InvalidInput("必须提供 board".to_owned()));
    }
    Ok(board)
}

fn require_label_ref(label_ref: &str) -> Result<&str, ClientError> {
    let label_ref = label_ref.trim();
    if label_ref.is_empty() {
        return Err(ClientError::InvalidInput(
            "必须提供 label ID 或名称".to_owned(),
        ));
    }
    Ok(label_ref)
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn label_client_requires_global_task_ids() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client
                .list_task_labels("default#1")
                .await
                .expect_err("board-local selector must be resolved first")
                .code(),
            "invalid_input"
        );
    }

    #[tokio::test]
    async fn label_delete_requires_board_and_label_reference() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client
                .delete_board_label(" ", "l_label", false)
                .await
                .expect_err("empty board must be rejected")
                .code(),
            "invalid_input"
        );
        assert_eq!(
            client
                .delete_board_label("default", " ", false)
                .await
                .expect_err("empty label reference must be rejected")
                .code(),
            "invalid_input"
        );
    }
}
