//! Thin synchronous client for the canonical localhost kanban host.

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use kanban_contract::{
    ApiBoard, ApiBoardColumn, ApiErrorCode, ErrorEnvelope, HealthReport, HealthResponse,
    ListBoardColumnsResponse, ListBoardsResponse,
};
use serde::de::DeserializeOwned;
use thiserror::Error;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8721";

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid server URL: {0}")]
    InvalidServerUrl(String),
    #[error("server unavailable: {0}")]
    ServerUnavailable(String),
    #[error("{code:?}: {message}")]
    Api {
        status: u16,
        code: ApiErrorCode,
        message: String,
    },
    #[error("invalid server response: {0}")]
    InvalidResponse(String),
}

impl ClientError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidServerUrl(_) => "invalid_input",
            Self::ServerUnavailable(_) => "server_unavailable",
            Self::Api { code, .. } => api_error_code(*code),
            Self::InvalidResponse(_) => "invalid_response",
        }
    }
}

const fn api_error_code(code: ApiErrorCode) -> &'static str {
    match code {
        ApiErrorCode::NotFound => "not_found",
        ApiErrorCode::Conflict => "conflict",
        ApiErrorCode::IdempotencyConflict => "idempotency_conflict",
        ApiErrorCode::DependencyCycle => "dependency_cycle",
        ApiErrorCode::InvalidInput => "invalid_input",
        ApiErrorCode::FeatureNotAvailable => "feature_not_available",
        ApiErrorCode::ServerUnavailable => "server_unavailable",
        ApiErrorCode::ExecutionPlanRequired => "execution_plan_required",
        ApiErrorCode::StepsIncomplete => "steps_incomplete",
        ApiErrorCode::ClaimTokenMismatch => "claim_token_mismatch",
        ApiErrorCode::DependencyBlocked => "dependency_blocked",
        ApiErrorCode::ClaimConflict => "claim_conflict",
        ApiErrorCode::InvalidTransition => "invalid_transition",
        ApiErrorCode::Internal => "internal",
    }
}

#[derive(Clone)]
pub struct KanbanClient {
    base_url: String,
    actor: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for KanbanClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KanbanClient")
            .field("base_url", &self.base_url)
            .field("actor", &self.actor)
            .finish_non_exhaustive()
    }
}

impl KanbanClient {
    pub fn new(base_url: impl Into<String>, actor: impl Into<String>) -> Result<Self, ClientError> {
        let base_url = normalize_localhost_url(base_url.into())?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
        Ok(Self {
            base_url,
            actor: actor.into(),
            agent,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn health(&self) -> Result<HealthReport, ClientError> {
        let response: HealthResponse = self.get("/health")?;
        Ok(response.data)
    }

    pub fn list_boards(&self, include_archived: bool) -> Result<Vec<ApiBoard>, ClientError> {
        let path = format!("/api/v1/boards?include_archived={include_archived}");
        let response: ListBoardsResponse = self.get(&path)?;
        Ok(response.data)
    }

    pub fn list_board_columns(&self, board: &str) -> Result<Vec<ApiBoardColumn>, ClientError> {
        let path = format!("/api/v1/boards/{}/columns", encode_path_segment(board));
        let response: ListBoardColumnsResponse = self.get(&path)?;
        Ok(response.data)
    }

    fn get<T>(&self, path: &str) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let request = self
            .agent
            .get(&format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.call())
    }
}

fn decode_response<T>(response: Result<ureq::Response, ureq::Error>) -> Result<T, ClientError>
where
    T: DeserializeOwned,
{
    match response {
        Ok(response) => response
            .into_json::<T>()
            .map_err(|error| ClientError::InvalidResponse(error.to_string())),
        Err(ureq::Error::Status(status, response)) => {
            let envelope = response.into_json::<ErrorEnvelope>().map_err(|error| {
                ClientError::InvalidResponse(format!(
                    "HTTP {status} did not contain the error envelope: {error}"
                ))
            })?;
            Err(ClientError::Api {
                status,
                code: envelope.error.code,
                message: envelope.error.message,
            })
        }
        Err(ureq::Error::Transport(error)) => {
            Err(ClientError::ServerUnavailable(error.to_string()))
        }
    }
}

fn normalize_localhost_url(value: String) -> Result<String, ClientError> {
    let value = value.trim().trim_end_matches('/').to_owned();
    let Some(authority) = value.strip_prefix("http://") else {
        return Err(ClientError::InvalidServerUrl(value));
    };
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@'])
        || !is_loopback_authority(authority)
    {
        return Err(ClientError::InvalidServerUrl(value));
    }
    Ok(value)
}

fn is_loopback_authority(authority: &str) -> bool {
    if matches!(authority, "localhost" | "[::1]") {
        return true;
    }
    if let Some(port) = authority.strip_prefix("localhost:") {
        return port.parse::<u16>().is_ok();
    }
    authority.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || authority
            .parse::<SocketAddr>()
            .is_ok_and(|addr| addr.ip().is_loopback())
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_loopback_base_urls() {
        for value in [
            "http://127.0.0.1:8721",
            "http://localhost:8721/",
            "http://[::1]:8721",
        ] {
            assert!(KanbanClient::new(value, "test").is_ok(), "{value}");
        }
        for value in [
            "",
            "http://example.com",
            "http://192.168.1.10:8721",
            "https://127.0.0.1:8721",
            "http://127.0.0.1:8721@evil.example",
            "http://localhost:8721/api",
        ] {
            assert!(KanbanClient::new(value, "test").is_err(), "{value}");
        }
    }

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("board/#1"), "board%2F%231");
    }
}
