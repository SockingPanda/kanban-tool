//! 面向 label semantics 与 ontology surface 的 typed localhost 客户端。

use kanban_protocol::{ListBoardLabelProposalsResponse, ListTaskLabelProposalsResponse};
use serde_json::{Value, json};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

fn data<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ClientError> {
    value
        .get("data")
        .cloned()
        .ok_or_else(|| ClientError::InvalidResponse("响应缺少 data".to_owned()))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        })
}

fn query_value(value: &str) -> String {
    encode_path_segment(value)
}

impl KanbanClient {
    pub fn list_label_semantics(&self, board: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/labels/semantics",
            encode_path_segment(board)
        ))
    }

    pub fn get_label_semantics(&self, board: &str, label_ref: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/labels/{}/semantics",
            encode_path_segment(board),
            encode_path_segment(label_ref)
        ))
    }

    pub fn upsert_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        mut body: Value,
    ) -> Result<Value, ClientError> {
        body["label_ref"] = Value::String(label_ref.to_owned());
        self.put_json(
            &format!(
                "/api/v1/boards/{}/labels/{}/semantics",
                encode_path_segment(board),
                encode_path_segment(label_ref)
            ),
            &body,
        )
    }

    pub fn delete_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        expected_hash: &str,
        reason: &str,
    ) -> Result<Value, ClientError> {
        self.delete(&format!(
            "/api/v1/boards/{}/labels/{}/semantics?expected_semantics_hash={}&reason={}",
            encode_path_segment(board),
            encode_path_segment(label_ref),
            query_value(expected_hash),
            query_value(reason)
        ))
    }

    pub fn list_label_atoms(&self, board: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/labels/atoms",
            encode_path_segment(board)
        ))
    }

    pub fn explain_label_atom(&self, board: &str, atom_ref: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/labels/atoms/{}/explain",
            encode_path_segment(board),
            encode_path_segment(atom_ref)
        ))
    }

    pub fn label_atom_index_status(&self, board: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/labels/atom-index/status",
            encode_path_segment(board)
        ))
    }

    pub fn rebuild_label_atom_index(&self, board: &str) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/boards/{}/labels/atom-index/rebuild",
                encode_path_segment(board)
            ),
            &json!({}),
        )
    }

    pub fn query_label_atom_index(
        &self,
        board: &str,
        query: Option<&str>,
        polarity: Option<&str>,
        limit: usize,
    ) -> Result<Value, ClientError> {
        let mut path = format!(
            "/api/v1/boards/{}/labels/atom-index/query?limit={limit}",
            encode_path_segment(board)
        );
        if let Some(query) = query {
            path.push_str(&format!("&q={}", query_value(query)));
        }
        if let Some(polarity) = polarity {
            path.push_str(&format!("&polarity={}", query_value(polarity)));
        }
        self.get(&path)
    }

    pub fn suggest_task_labels(
        &self,
        task_id: &str,
        board: Option<&str>,
        options: Value,
    ) -> Result<Value, ClientError> {
        let board = board.unwrap_or("default");
        let mut path = format!(
            "/api/v1/tasks/{}/labels/suggestions?board={}",
            encode_path_segment(task_id),
            query_value(board)
        );
        if let Some(object) = options.as_object() {
            for (key, value) in object {
                if let Some(value) = value.as_str() {
                    path.push_str(&format!("&{key}={}", query_value(value)));
                } else if let Some(value) = value.as_f64() {
                    path.push_str(&format!("&{key}={value}"));
                } else if let Some(value) = value.as_u64() {
                    path.push_str(&format!("&{key}={value}"));
                }
            }
        }
        self.get(&path)
    }

    pub fn list_label_proposals(
        &self,
        board: &str,
        task_id: Option<&str>,
        status: Option<&str>,
    ) -> Result<Value, ClientError> {
        if let Some(task_id) = task_id {
            let response = self.list_task_label_proposals(board, task_id, status)?;
            serde_json::to_value(response)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        } else {
            let response = self.list_board_label_proposals(board, status)?;
            serde_json::to_value(response)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        }
    }

    pub fn list_task_label_proposals(
        &self,
        board: &str,
        task_id: &str,
        status: Option<&str>,
    ) -> Result<ListTaskLabelProposalsResponse, ClientError> {
        let path = proposals_path(
            &format!(
                "/api/v1/tasks/{}/label-proposals?board={}",
                encode_path_segment(task_id),
                query_value(board)
            ),
            status,
        );
        self.get(&path)
    }

    pub fn list_board_label_proposals(
        &self,
        board: &str,
        status: Option<&str>,
    ) -> Result<ListBoardLabelProposalsResponse, ClientError> {
        let path = proposals_path(
            &format!(
                "/api/v1/boards/{}/label-proposals",
                encode_path_segment(board)
            ),
            status,
        );
        self.get(&path)
    }

    pub fn propose_task_label(
        &self,
        board: &str,
        task_id: &str,
        mut body: Value,
    ) -> Result<Value, ClientError> {
        body["task_ref"] = Value::String(task_id.to_owned());
        self.post_json(
            &format!(
                "/api/v1/tasks/{}/label-proposals?board={}",
                encode_path_segment(task_id),
                query_value(board)
            ),
            &body,
        )
    }

    pub fn get_label_proposal(&self, proposal_id: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/label-proposals/{}",
            encode_path_segment(proposal_id)
        ))
    }

    pub fn decide_label_proposal(
        &self,
        proposal_id: &str,
        accept: bool,
        body: Value,
    ) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/label-proposals/{}/{decision}",
                encode_path_segment(proposal_id),
                decision = if accept { "accept" } else { "reject" }
            ),
            &body,
        )
    }

    pub fn record_label_ontology_observation(
        &self,
        board: &str,
        task_id: &str,
        mut body: Value,
    ) -> Result<Value, ClientError> {
        body["task_ref"] = Value::String(task_id.to_owned());
        self.post_json(
            &format!(
                "/api/v1/tasks/{}/label-ontology/observations?board={}",
                encode_path_segment(task_id),
                query_value(board)
            ),
            &body,
        )
    }

    pub fn list_label_ontology_signals(
        &self,
        board: &str,
        query: Value,
    ) -> Result<Value, ClientError> {
        let mut path = format!(
            "/api/v1/boards/{}/label-ontology/signals",
            encode_path_segment(board)
        );
        if let Some(object) = query.as_object() {
            let mut first = true;
            for (key, value) in object {
                let encoded = match value {
                    Value::String(value) => query_value(value),
                    Value::Bool(value) => value.to_string(),
                    Value::Number(value) => value.to_string(),
                    Value::Array(value) => value
                        .iter()
                        .filter_map(Value::as_str)
                        .map(query_value)
                        .collect::<Vec<_>>()
                        .join(","),
                    _ => continue,
                };
                path.push(if first { '?' } else { '&' });
                first = false;
                path.push_str(key);
                path.push('=');
                path.push_str(&encoded);
            }
        }
        self.get(&path)
    }

    pub fn get_label_ontology_signal(&self, signal_id: &str) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/label-ontology/signals/{}",
            encode_path_segment(signal_id)
        ))
    }

    pub fn review_label_ontology(&self, board: &str, query: Value) -> Result<Value, ClientError> {
        let group_by = query
            .get("group_by")
            .and_then(Value::as_str)
            .unwrap_or("label");
        self.get(&format!(
            "/api/v1/boards/{}/label-ontology/review?group_by={}",
            encode_path_segment(board),
            query_value(group_by)
        ))
    }

    pub fn create_label_ontology_action(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/boards/{}/label-ontology/actions",
                encode_path_segment(board)
            ),
            &body,
        )
    }

    pub fn apply_label_ontology_atom(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/boards/{}/label-ontology/apply/atom",
                encode_path_segment(board)
            ),
            &body,
        )
    }

    pub fn revert_label_ontology(&self, board: &str, body: Value) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/boards/{}/label-ontology/revert",
                encode_path_segment(board)
            ),
            &body,
        )
    }

    pub fn validate_label_ontology(&self, board: &str, body: Value) -> Result<Value, ClientError> {
        self.post_json(
            &format!(
                "/api/v1/boards/{}/label-ontology/validate",
                encode_path_segment(board)
            ),
            &body,
        )
    }

    pub fn label_ontology_quality(
        &self,
        board: &str,
        sample_limit: usize,
    ) -> Result<Value, ClientError> {
        self.get(&format!(
            "/api/v1/boards/{}/label-ontology/review?quality=true&sample_limit={sample_limit}",
            encode_path_segment(board)
        ))
    }

    pub fn ontology_data<T: serde::de::DeserializeOwned>(
        &self,
        response: Value,
    ) -> Result<T, ClientError> {
        data(response)
    }

    fn post_json(&self, path: &str, body: &Value) -> Result<Value, ClientError> {
        self.post(path, body)
    }

    fn put_json(&self, path: &str, body: &Value) -> Result<Value, ClientError> {
        self.put(path, body)
    }
}

fn proposals_path(base: &str, status: Option<&str>) -> String {
    status.map_or_else(
        || base.to_owned(),
        |status| {
            let separator = if base.contains('?') { '&' } else { '?' };
            format!("{base}{separator}status={}", query_value(status))
        },
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread::{self, JoinHandle},
    };

    use super::{KanbanClient, proposals_path};

    fn response_server(expected_path: &str, status: &str, body: &str) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected_request_line = format!("GET {expected_path} HTTP/1.1");
        let status = status.to_owned();
        let body = body.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert_eq!(request.lines().next(), Some(expected_request_line.as_str()));
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}"), handle)
    }

    #[test]
    fn proposal_list_paths_keep_task_and_board_scopes_distinct() {
        assert_eq!(
            proposals_path(
                "/api/v1/tasks/t_task/label-proposals?board=team%2Fone",
                Some("proposed"),
            ),
            "/api/v1/tasks/t_task/label-proposals?board=team%2Fone&status=proposed"
        );
        assert_eq!(
            proposals_path(
                "/api/v1/boards/team%2Fone/label-proposals",
                Some("accepted"),
            ),
            "/api/v1/boards/team%2Fone/label-proposals?status=accepted"
        );
        assert_eq!(
            proposals_path("/api/v1/boards/team%2Fone/label-proposals", None),
            "/api/v1/boards/team%2Fone/label-proposals"
        );
    }

    #[test]
    fn generic_proposal_list_dispatches_task_and_board_routes_without_fake_task() {
        let (base_url, handle) = response_server(
            "/api/v1/boards/team%2Fone/label-proposals?status=accepted",
            "200 OK",
            r#"{"data":[]}"#,
        );
        let client = KanbanClient::new(base_url, "test").unwrap();
        let board = client
            .list_label_proposals("team/one", None, Some("accepted"))
            .unwrap();
        assert_eq!(board, serde_json::json!({"data": []}));
        handle.join().unwrap();

        let (base_url, handle) = response_server(
            "/api/v1/tasks/t_scope/label-proposals?board=team%2Fone&status=proposed",
            "200 OK",
            r#"{"data":[]}"#,
        );
        let client = KanbanClient::new(base_url, "test").unwrap();
        let task = client
            .list_label_proposals("team/one", Some("t_scope"), Some("proposed"))
            .unwrap();
        assert_eq!(task, serde_json::json!({"data": []}));
        handle.join().unwrap();
    }

    #[test]
    fn proposal_list_client_preserves_standard_error_envelope() {
        let (base_url, handle) = response_server(
            "/api/v1/boards/missing/label-proposals",
            "404 Not Found",
            r#"{"error":{"code":"not_found","message":"board missing"}}"#,
        );
        let client = KanbanClient::new(base_url, "test").unwrap();
        let error = client
            .list_board_label_proposals("missing", None)
            .expect_err("HTTP error should decode as ErrorEnvelope");
        assert_eq!(error.code(), "not_found");
        match error {
            crate::ClientError::Api {
                status,
                code: kanban_protocol::ApiErrorCode::NotFound,
                message,
            } => {
                assert_eq!(status, 404);
                assert_eq!(message, "board missing");
            }
            other => panic!("unexpected client error: {other:?}"),
        }
        handle.join().unwrap();
    }
}
