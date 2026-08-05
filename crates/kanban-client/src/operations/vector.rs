use kanban_contract::{
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
    use super::*;
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

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
}
