use kanban_protocol::ApiRun;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn get_run(&self, run_id: &str) -> Result<ApiRun, ClientError> {
        let run_id = require_run_id(run_id)?;
        let response: kanban_protocol::GetRunResponse = rpc!(
            self,
            get_run,
            GetRunRequest,
            kanban_protocol::GetRunPath {
                run_id: run_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }
}

fn require_run_id(run_id: &str) -> Result<&str, ClientError> {
    let run_id = run_id.trim();
    if !run_id.starts_with("r_") || run_id.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "运行记录 ID 必须是全局 r_... ID".to_owned(),
        ));
    }
    Ok(run_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> KanbanClient {
        KanbanClient::new("http://127.0.0.1:8721", "test").unwrap()
    }

    #[tokio::test]
    async fn rejects_invalid_run_ids_locally() {
        for run_id in ["", "r_", "default#1", "ordinary"] {
            assert!(matches!(
                client().get_run(run_id).await,
                Err(ClientError::InvalidInput(_))
            ));
        }
    }
}
