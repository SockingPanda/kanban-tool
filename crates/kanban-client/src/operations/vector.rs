use kanban_protocol::{
    VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse, VectorLabelAtomResult,
    VectorProjectionRequest, VectorProjectionResponse, VectorQuery, VectorQueryChunksResponse,
    VectorQueryLabelAtomsResponse, VectorStatus, VectorStatusResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn vector_status(&self, board: &str) -> Result<VectorStatus, ClientError> {
        let board = board.trim();
        if board.is_empty() {
            return Err(ClientError::InvalidInput("board 不能为空".to_owned()));
        }
        let response: VectorStatusResponse = self.get(&format!(
            "/api/v1/vector/status?board={}",
            encode_path_segment(board)
        ))?;
        Ok(response.data)
    }

    pub fn configure_vector(
        &self,
        config: VectorConfigureRequest,
    ) -> Result<VectorConfigureRequest, ClientError> {
        let response: VectorConfigureResponse = self.post("/api/v1/vector/configure", &config)?;
        Ok(response.data)
    }

    pub fn rebuild_vector(&self, board: &str) -> Result<VectorStatus, ClientError> {
        let response: VectorProjectionResponse = self.post(
            "/api/v1/vector/rebuild",
            &VectorProjectionRequest {
                board: board.to_owned(),
            },
        )?;
        Ok(response.data)
    }

    pub fn sync_vector(&self, board: &str) -> Result<VectorStatus, ClientError> {
        let response: VectorProjectionResponse = self.post(
            "/api/v1/vector/sync",
            &VectorProjectionRequest {
                board: board.to_owned(),
            },
        )?;
        Ok(response.data)
    }

    pub fn query_vector_chunks(
        &self,
        query: VectorQuery,
    ) -> Result<Vec<VectorChunkResult>, ClientError> {
        let response: VectorQueryChunksResponse = self.get(&vector_chunks_path(&query))?;
        Ok(response.data)
    }

    pub fn query_vector_label_atoms(
        &self,
        query: VectorQuery,
    ) -> Result<Vec<VectorLabelAtomResult>, ClientError> {
        let response: VectorQueryLabelAtomsResponse = self.get(&vector_label_atoms_path(&query))?;
        Ok(response.data)
    }
}

fn vector_chunks_path(query: &VectorQuery) -> String {
    let mut path = format!(
        "/api/v1/vector/query-chunks?board={}&q={}&limit={}",
        encode_path_segment(&query.board),
        encode_path_segment(&query.q),
        query.limit
    );
    if let Some(model) = query.embedding_model.as_deref() {
        path.push_str("&embedding_model=");
        path.push_str(&encode_path_segment(model));
    }
    path
}

fn vector_label_atoms_path(query: &VectorQuery) -> String {
    let mut path = format!(
        "/api/v1/vector/query-label-atoms?board={}&q={}&limit={}",
        encode_path_segment(&query.board),
        encode_path_segment(&query.q),
        query.limit
    );
    if let Some(model) = query.embedding_model.as_deref() {
        path.push_str("&embedding_model=");
        path.push_str(&encode_path_segment(model));
    }
    if let Some(polarity) = query.polarity.as_deref() {
        path.push_str("&polarity=");
        path.push_str(&encode_path_segment(polarity));
    }
    if query.include_vector {
        path.push_str("&include_vector=true");
    }
    path
}

#[cfg(test)]
mod tests {
    use serde::{Serialize, de::DeserializeOwned};

    use kanban_protocol::{
        DataEnvelope, VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse,
        VectorLabelAtomResult, VectorProjectionRequest, VectorProjectionResponse, VectorQuery,
        VectorQueryChunksResponse, VectorQueryLabelAtomsResponse, VectorStatus,
    };

    use super::*;
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    fn fixture(path: &str) -> serde_json::Value {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/fixtures/api/");
        let content = std::fs::read_to_string(format!("{root}{path}"))
            .unwrap_or_else(|error| panic!("read fixture {path}: {error}"));
        serde_json::from_str(&content)
            .unwrap_or_else(|error| panic!("parse fixture {path}: {error}"))
    }

    fn assert_fixture<T: Serialize>(value: &T, path: &str) {
        assert_eq!(
            serde_json::to_value(value).expect("serialize fixture value"),
            fixture(path),
            "fixture {path}"
        );
    }

    fn parse_fixture<T: DeserializeOwned>(path: &str) -> T {
        serde_json::from_value(fixture(path)).expect("deserialize fixture value")
    }

    fn configure_request() -> VectorConfigureRequest {
        VectorConfigureRequest {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "nomic-embed-text".to_owned(),
            dimensions: 32,
        }
    }

    fn projection_request() -> VectorProjectionRequest {
        VectorProjectionRequest {
            board: "default".to_owned(),
        }
    }

    fn query() -> VectorQuery {
        VectorQuery {
            board: "default".to_owned(),
            q: "lease retry".to_owned(),
            limit: 5,
            embedding_model: None,
            polarity: None,
            include_vector: false,
        }
    }

    fn label_query() -> VectorQuery {
        VectorQuery {
            q: "label semantics".to_owned(),
            polarity: Some("positive".to_owned()),
            include_vector: true,
            ..query()
        }
    }

    fn vector_status() -> VectorStatus {
        VectorStatus {
            backend: "turso-vector32".to_owned(),
            enabled: false,
            message: "fixture".to_owned(),
            diagnostics: vec!["vector provider 未配置".to_owned()],
            dirty: Some(true),
            board_dirty: Some(true),
            generation: None,
        }
    }

    fn chunk_response() -> VectorQueryChunksResponse {
        DataEnvelope::new(vec![VectorChunkResult {
            id: "vec_fixture".to_owned(),
            entity_uri: Some("kb://task/t_fixture".to_owned()),
            source_kind: "task".to_owned(),
            content: "Lease retry policy".to_owned(),
            content_hash: "sha256:fixture-content".to_owned(),
            embedding_model: "nomic-embed-text".to_owned(),
            distance: 0.1,
            score: 0.9,
        }])
    }

    fn label_response() -> VectorQueryLabelAtomsResponse {
        DataEnvelope::new(vec![VectorLabelAtomResult {
            atom_id: "la_fixture".to_owned(),
            label_id: "l_fixture".to_owned(),
            label_name: "Fixture label".to_owned(),
            board_id: "b_default".to_owned(),
            polarity: "positive".to_owned(),
            kind: "description".to_owned(),
            text: "Label semantics".to_owned(),
            ordinal: 0,
            content_hash: "sha256:fixture-atom".to_owned(),
            embedding_model: "nomic-embed-text".to_owned(),
            distance: 0.2,
            vector: Some(vec![0.1, 0.2]),
        }])
    }

    #[test]
    fn vector_status_rejects_empty_board_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client.vector_status(" ").unwrap_err().code(),
            "invalid_input"
        );
    }

    #[test]
    fn vector_query_paths_encode_filters_and_model() {
        let query = VectorQuery {
            board: "board/#1".to_owned(),
            q: "lease retry".to_owned(),
            limit: 7,
            embedding_model: Some("model/v1".to_owned()),
            polarity: Some("positive".to_owned()),
            include_vector: true,
        };
        assert_eq!(
            vector_chunks_path(&query),
            "/api/v1/vector/query-chunks?board=board%2F%231&q=lease%20retry&limit=7&embedding_model=model%2Fv1"
        );
        assert_eq!(
            vector_label_atoms_path(&query),
            "/api/v1/vector/query-label-atoms?board=board%2F%231&q=lease%20retry&limit=7&embedding_model=model%2Fv1&polarity=positive&include_vector=true"
        );
    }

    #[test]
    fn vector_configure_request_fixture_is_produced() {
        assert_fixture(
            &configure_request(),
            "vector-configure-request.v1.valid.json",
        );
    }

    #[test]
    fn vector_rebuild_request_fixture_is_produced() {
        assert_fixture(
            &projection_request(),
            "vector-rebuild-request.v1.valid.json",
        );
    }

    #[test]
    fn vector_sync_request_fixture_is_produced() {
        assert_fixture(&projection_request(), "vector-sync-request.v1.valid.json");
    }

    #[test]
    fn vector_query_chunks_query_fixture_is_produced() {
        assert_fixture(&query(), "vector-query-chunks-query.v1.valid.json");
    }

    #[test]
    fn vector_query_label_atoms_query_fixture_is_produced() {
        assert_fixture(
            &label_query(),
            "vector-query-label-atoms-query.v1.valid.json",
        );
    }

    #[test]
    fn vector_configure_response_fixture_is_consumed_by_client() {
        let response: VectorConfigureResponse =
            parse_fixture("vector-configure-response.v1.valid.json");
        assert_eq!(response.data, configure_request());
    }

    #[test]
    fn vector_rebuild_response_fixture_is_consumed_by_client() {
        let response: VectorProjectionResponse =
            parse_fixture("vector-rebuild-response.v1.valid.json");
        assert_eq!(response.data, vector_status());
    }

    #[test]
    fn vector_sync_response_fixture_is_consumed_by_client() {
        let response: VectorProjectionResponse =
            parse_fixture("vector-sync-response.v1.valid.json");
        assert_eq!(response.data, vector_status());
    }

    #[test]
    fn vector_query_chunks_response_fixture_is_consumed_by_client() {
        let response: VectorQueryChunksResponse =
            parse_fixture("vector-query-chunks-response.v1.valid.json");
        assert_eq!(response, chunk_response());
    }

    #[test]
    fn vector_query_label_atoms_response_fixture_is_consumed_by_client() {
        let response: VectorQueryLabelAtomsResponse =
            parse_fixture("vector-query-label-atoms-response.v1.valid.json");
        assert_eq!(response, label_response());
    }
}
