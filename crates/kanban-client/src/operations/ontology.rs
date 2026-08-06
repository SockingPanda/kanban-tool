//! 面向 label semantics 与 ontology surface 的 typed localhost 客户端。

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
        let task_id = task_id.unwrap_or("_");
        let mut path = format!(
            "/api/v1/tasks/{}/label-proposals?board={}",
            encode_path_segment(task_id),
            query_value(board)
        );
        if let Some(status) = status {
            path.push_str(&format!("&status={}", query_value(status)));
        }
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
