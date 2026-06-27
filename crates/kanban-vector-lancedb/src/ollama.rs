use serde::{Deserialize, Serialize};
use std::time::Duration;

use kanban_vector::{EmbeddingProvider, VectorError, ensure_dimensions};

const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_READ_TIMEOUT: Duration = Duration::from_secs(30);

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
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(OLLAMA_CONNECT_TIMEOUT)
            .timeout_read(OLLAMA_READ_TIMEOUT)
            .build();
        let response = agent
            .post(&url)
            .send_json(EmbedRequest {
                model: &self.model,
                input: text,
                dimensions: self.dimensions,
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
    dimensions: usize,
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use kanban_vector::{EmbeddingProvider, VectorError};

    use super::OllamaEmbeddingProvider;

    fn mock_ollama(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            assert!(request.starts_with("POST /api/embed "));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });
        endpoint
    }

    fn mock_ollama_with_request(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            assert!(request.starts_with("POST /api/embed "));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
            request
        });
        (endpoint, handle)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut chunk = [0; 1024];
        let header_end = loop {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0);
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0);
            buffer.extend_from_slice(&chunk[..read]);
        }
        String::from_utf8_lossy(&buffer).to_string()
    }

    #[test]
    fn ollama_provider_reads_first_embedding() {
        let endpoint = mock_ollama(r#"{"embeddings":[[0.1,0.2,0.3]]}"#);
        let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

        assert_eq!(provider.embedding_model(), "test-model");
        assert_eq!(provider.embed("short text").unwrap(), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn ollama_provider_sends_dimensions_in_embed_request() {
        let (endpoint, request) = mock_ollama_with_request(r#"{"embeddings":[[0.1,0.2,0.3]]}"#);
        let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

        assert_eq!(provider.embed("short text").unwrap(), vec![0.1, 0.2, 0.3]);
        let request = request.join().unwrap();
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["input"], "short text");
        assert_eq!(body["dimensions"], 3);
    }

    #[test]
    fn ollama_provider_rejects_dimension_mismatch() {
        let endpoint = mock_ollama(r#"{"embeddings":[[0.1,0.2]]}"#);
        let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

        assert!(matches!(
            provider.embed("short text"),
            Err(VectorError::DimensionMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }

    #[test]
    fn ollama_provider_maps_error_responses_to_store_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).unwrap();
            let response = r#"{"error":"model not found"}"#;
            write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });
        let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

        assert!(matches!(
            provider.embed("short text"),
            Err(VectorError::Store(_))
        ));
    }
}
