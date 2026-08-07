use kanban_protocol::{ApiRunLog, GetRunLogResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn get_run_log(&self, run_id: &str) -> Result<ApiRunLog, ClientError> {
        let response: GetRunLogResponse = self.get(&run_log_path(run_id)?)?;
        Ok(response.data)
    }
}

fn run_log_path(run_id: &str) -> Result<String, ClientError> {
    let run_id = run_id.trim();
    if !run_id.starts_with("r_") || run_id.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "运行记录 ID 必须是全局 r_... ID".to_owned(),
        ));
    }
    Ok(format!("/api/v1/runs/{}/log", encode_path_segment(run_id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> KanbanClient {
        KanbanClient::new("http://127.0.0.1:8721", "test").unwrap()
    }

    #[test]
    fn rejects_invalid_run_ids_locally() {
        for run_id in ["", "r_", "default#1", "ordinary"] {
            assert!(matches!(
                client().get_run_log(run_id),
                Err(ClientError::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn run_log_path_trims_and_encodes_global_id() {
        assert_eq!(
            run_log_path(" r_run/id ").unwrap(),
            "/api/v1/runs/r_run%2Fid/log"
        );
    }
}
