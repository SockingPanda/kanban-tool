//! Thin synchronous client for the canonical localhost kanban host.

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use kanban_contract::{
    ApiBoard, ApiBoardColumn, ApiClaim, ApiErrorCode, ApiExecutionPlan, ApiTask, ClaimTaskRequest,
    ClaimTaskResponse, CreateTaskRequest, CreateTaskResponse, ErrorEnvelope, GetTaskResponse,
    HealthReport, HealthResponse, ListBoardColumnsResponse, ListBoardsResponse, ListTasksQuery,
    ListTasksResponse, MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse,
    PromoteTaskRequest, PromoteTaskResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8721";

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
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
            Self::InvalidInput(_) | Self::InvalidServerUrl(_) => "invalid_input",
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

    pub fn create_task(
        &self,
        board: &str,
        request: CreateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let request = prepare_create_request(request);
        let path = format!("/api/v1/boards/{}/tasks", encode_path_segment(board));
        let response: CreateTaskResponse = self.post(&path, &request)?;
        Ok(response.data)
    }

    pub fn list_tasks(
        &self,
        board: &str,
        query: &ListTasksQuery,
    ) -> Result<ListTasksResponse, ClientError> {
        self.get(&list_tasks_path(board, query))
    }

    pub fn get_task(&self, task_id: &str) -> Result<ApiTask, ClientError> {
        let response: GetTaskResponse = self.get(&format!(
            "/api/v1/tasks/{}",
            encode_path_segment(task_id.trim())
        ))?;
        Ok(response.data)
    }

    pub fn resolve_task_id(&self, board: &str, selector: &str) -> Result<String, ClientError> {
        let selector = selector.trim();
        if selector.starts_with("t_") && selector.len() > 2 {
            return Ok(selector.to_owned());
        }
        if !is_board_local_task_selector(selector) {
            return Err(ClientError::InvalidInput(
                "task selector must be a global t_... id, board#seq, #seq, or numeric seq"
                    .to_owned(),
            ));
        }
        let response = self.list_tasks(
            board,
            &ListTasksQuery {
                q: Some(selector.to_owned()),
                include_archived: true,
                limit: 2,
                ..ListTasksQuery::default()
            },
        )?;
        match response.data.as_slice() {
            [task] => Ok(task.id.clone()),
            [] => Err(ClientError::Api {
                status: 404,
                code: ApiErrorCode::NotFound,
                message: format!("task not found: {selector}"),
            }),
            _ => Err(ClientError::InvalidResponse(format!(
                "task selector is ambiguous: {selector}"
            ))),
        }
    }

    pub fn get_task_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.get_task(&task_id)
    }

    pub fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let response: MarkExecutionPlanNotRequiredResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/execution-plan/not-required",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn mark_execution_plan_not_required_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.mark_execution_plan_not_required(&task_id, request)
    }

    pub fn promote_task(
        &self,
        task_id: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: PromoteTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/promote",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn promote_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.promote_task(&task_id, request)
    }

    pub fn claim_task(
        &self,
        task_id: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let response: ClaimTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/claim",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn claim_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.claim_task(&task_id, request)
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

    fn post<B, T>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let body = serde_json::to_value(body)
            .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
        let request = self
            .agent
            .post(&format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.send_json(body))
    }
}

fn prepare_create_request(mut request: CreateTaskRequest) -> CreateTaskRequest {
    let task_id = request.task_id.get_or_insert_with(kanban_core::new_task_id);
    request
        .idempotency_key
        .get_or_insert_with(|| format!("task.create:{task_id}"));
    request
}

fn is_board_local_task_selector(selector: &str) -> bool {
    let numeric = |value: &str| {
        !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
    };
    if let Some(seq) = selector.strip_prefix('#') {
        return numeric(seq);
    }
    if numeric(selector) {
        return true;
    }
    selector
        .split_once('#')
        .is_some_and(|(board, seq)| !board.is_empty() && numeric(seq))
}

fn list_tasks_path(board: &str, query: &ListTasksQuery) -> String {
    let mut pairs = Vec::new();
    for status in &query.status {
        pairs.push(("status", status.as_str().to_owned()));
    }
    for priority in &query.priority {
        pairs.push(("priority", priority.get().to_string()));
    }
    for label in &query.label {
        pairs.push(("label", label.as_str().to_owned()));
    }
    for filter in &query.plan_filter {
        pairs.push(("plan_filter", filter.as_str().to_owned()));
    }
    if let Some(assignee) = query.assignee.as_deref() {
        pairs.push(("assignee", assignee.to_owned()));
    }
    if let Some(search) = query.q.as_deref() {
        pairs.push(("q", search.to_owned()));
    }
    pairs.push(("include_archived", query.include_archived.to_string()));
    pairs.push(("limit", query.limit.to_string()));
    pairs.push(("offset", query.offset.to_string()));
    pairs.push(("sort", query.sort.as_str().to_owned()));
    let query = pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={}", encode_path_segment(&value)))
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "/api/v1/boards/{}/tasks?{query}",
        encode_path_segment(board)
    )
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

    #[test]
    fn create_request_gets_stable_entity_local_identifiers() {
        let request = prepare_create_request(CreateTaskRequest {
            task_id: None,
            idempotency_key: None,
            title: "Create".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 3,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata: None,
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: None,
        });
        let task_id = request.task_id.as_deref().unwrap();
        assert!(task_id.starts_with("t_"));
        assert_eq!(
            request.idempotency_key.as_deref(),
            Some(format!("task.create:{task_id}").as_str())
        );
    }

    #[test]
    fn list_task_query_preserves_repeated_filters_and_escaping() {
        let query = ListTasksQuery {
            status: vec![
                kanban_contract::ApiTaskStatus::Ready,
                kanban_contract::ApiTaskStatus::Blocked,
            ],
            priority: vec![
                kanban_contract::ApiTaskPriority::new(0).unwrap(),
                kanban_contract::ApiTaskPriority::new(2).unwrap(),
            ],
            q: Some("a & b".into()),
            limit: 25,
            offset: 50,
            sort: kanban_contract::TaskReadSort::UpdatedAtDesc,
            ..ListTasksQuery::default()
        };
        assert_eq!(
            list_tasks_path("team/one", &query),
            "/api/v1/boards/team%2Fone/tasks?status=ready&status=blocked&priority=0&priority=2&q=a%20%26%20b&include_archived=false&limit=25&offset=50&sort=-updated_at"
        );
    }

    #[test]
    fn task_selector_classification_is_narrow_and_deterministic() {
        for selector in ["#1", "1", "default#1", "b_default#42"] {
            assert!(is_board_local_task_selector(selector), "{selector}");
        }
        for selector in ["", "#", "default", "default#x", "default#1#2"] {
            assert!(!is_board_local_task_selector(selector), "{selector}");
        }
    }
}
