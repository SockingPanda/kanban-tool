use serde::{Deserialize, Serialize};

use crate::{EmbeddingProvider, VectorError, ensure_dimensions};

#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    endpoint: String,
    model: String,
    dimensions: usize,
}

impl OllamaEmbeddingProvider {
    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
    ) -> Result<Self, VectorError> {
        let endpoint = endpoint.into().trim_end_matches('/').to_owned();
        let model = model.into();
        if endpoint.trim().is_empty() {
            return Err(VectorError::Store(
                "Ollama endpoint must not be empty".to_owned(),
            ));
        }
        if model.trim().is_empty() {
            return Err(VectorError::Store(
                "Ollama model must not be empty".to_owned(),
            ));
        }
        if dimensions == 0 {
            return Err(VectorError::Store(
                "Ollama embedding dimensions must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            endpoint,
            model,
            dimensions,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn check(&self) -> Result<(), VectorError> {
        self.embed("kanban vector provider check").map(|_| ())
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn embedding_model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let url = format!("{}/api/embed", self.endpoint);
        let response = ureq::post(&url)
            .send_json(EmbedRequest {
                model: &self.model,
                input: text,
            })
            .map_err(ollama_error)?;
        let body: EmbedResponse = response
            .into_json()
            .map_err(|err| VectorError::Store(format!("Ollama embed response error: {err}")))?;
        let embedding =
            body.embeddings.into_iter().next().ok_or_else(|| {
                VectorError::Store("Ollama embed response had no embeddings".into())
            })?;
        ensure_dimensions(&embedding, self.dimensions)?;
        Ok(embedding)
    }
}

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorResponse {
    error: Option<String>,
}

fn ollama_error(error: ureq::Error) -> VectorError {
    match error {
        ureq::Error::Status(status, response) => {
            let message = response
                .into_json::<OllamaErrorResponse>()
                .ok()
                .and_then(|body| body.error)
                .unwrap_or_else(|| format!("HTTP {status}"));
            VectorError::Store(format!("Ollama embed request failed: {message}"))
        }
        ureq::Error::Transport(error) => {
            VectorError::Store(format!("Ollama embed request failed: {error}"))
        }
    }
}
