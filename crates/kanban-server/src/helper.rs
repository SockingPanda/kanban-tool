use std::{env, fmt, io, path::PathBuf, process::Command};

use kanban_helper_protocol::HelperEnvelope;
use serde::de::DeserializeOwned;

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Clone, Copy)]
pub(crate) enum HelperKind {
    Vector,
    Graph,
}

impl HelperKind {
    pub(crate) fn binary_name(self) -> &'static str {
        match self {
            Self::Vector => "kanban-vector-lancedb",
            Self::Graph => "kanban-graph-oxigraph",
        }
    }

    pub(crate) fn env_var(self) -> &'static str {
        match self {
            Self::Vector => "KANBAN_VECTOR_HELPER",
            Self::Graph => "KANBAN_GRAPH_HELPER",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::Graph => "graph",
        }
    }

    fn configured_path(self, state: &AppState) -> Option<PathBuf> {
        match self {
            Self::Vector => state.vector_helper_path().map(PathBuf::from),
            Self::Graph => state.graph_helper_path().map(PathBuf::from),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct HelperErrorPayload {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelperRunErrorKind {
    SpawnNotFound,
    SpawnPermission,
    SpawnFailed,
    InvalidEnvelope,
    InvalidPayload,
    HelperError,
    ExitFailed,
}

#[derive(Debug)]
pub(crate) struct HelperRunError {
    kind: HelperRunErrorKind,
    message: String,
}

impl HelperRunError {
    pub(crate) fn kind(&self) -> HelperRunErrorKind {
        self.kind
    }

    pub(crate) fn is_status_degraded(&self) -> bool {
        self.is_helper_missing() || matches!(self.kind, HelperRunErrorKind::InvalidEnvelope)
    }

    pub(crate) fn is_helper_missing(&self) -> bool {
        matches!(
            self.kind,
            HelperRunErrorKind::SpawnNotFound | HelperRunErrorKind::SpawnPermission
        )
    }

    pub(crate) fn degraded_backend(&self) -> &'static str {
        match self.kind {
            HelperRunErrorKind::InvalidEnvelope => "helper-invalid",
            _ => "helper-missing",
        }
    }

    pub(crate) fn degraded_diagnostic(&self) -> &'static str {
        match self.kind {
            HelperRunErrorKind::InvalidEnvelope => "helper_invalid_envelope",
            _ => "helper_missing",
        }
    }

    fn new(kind: HelperRunErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for HelperRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for HelperRunError {}

impl From<HelperRunError> for ApiError {
    fn from(value: HelperRunError) -> Self {
        ApiError(kanban_core::KanbanError::Storage(value.to_string()))
    }
}

pub(crate) async fn run_helper_json<T>(
    state: AppState,
    kind: HelperKind,
    args: Vec<String>,
) -> Result<T, HelperRunError>
where
    T: DeserializeOwned + Send + 'static,
{
    tokio::task::spawn_blocking(move || run_helper_json_blocking(&state, kind, &args))
        .await
        .map_err(|error| {
            HelperRunError::new(
                HelperRunErrorKind::SpawnFailed,
                format!("{} helper worker failed: {error}", kind.label()),
            )
        })?
}

pub(crate) fn helper_degraded_message(kind: HelperKind, error: &HelperRunError) -> String {
    match error.kind() {
        HelperRunErrorKind::InvalidEnvelope => format!(
            "{} helper returned invalid output: {}; set {} or install /usr/lib/kanban/{}",
            kind.label(),
            bounded(&error.to_string()),
            kind.env_var(),
            kind.binary_name()
        ),
        _ => format!(
            "{} helper unavailable: {}; set {} or install /usr/lib/kanban/{}",
            kind.label(),
            bounded(&error.to_string()),
            kind.env_var(),
            kind.binary_name()
        ),
    }
}

fn run_helper_json_blocking<T>(
    state: &AppState,
    kind: HelperKind,
    args: &[String],
) -> Result<T, HelperRunError>
where
    T: DeserializeOwned,
{
    let helper = resolve_helper(state, kind);
    let output = Command::new(&helper).args(args).output().map_err(|error| {
        let kind_code = match error.kind() {
            io::ErrorKind::NotFound => HelperRunErrorKind::SpawnNotFound,
            io::ErrorKind::PermissionDenied => HelperRunErrorKind::SpawnPermission,
            _ => HelperRunErrorKind::SpawnFailed,
        };
        HelperRunError::new(
            kind_code,
            format!(
                "failed to run {} helper {} (set {} or AppState helper path to override): {}",
                kind.label(),
                helper.display(),
                kind.env_var(),
                error
            ),
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        if let Ok(envelope) = HelperEnvelope::from_json(stdout.trim()) {
            if let Ok(error) = envelope.decode::<HelperErrorPayload>() {
                return Err(HelperRunError::new(
                    HelperRunErrorKind::HelperError,
                    format!(
                        "{} helper failed: {} ({})",
                        kind.label(),
                        error.message,
                        error.code
                    ),
                ));
            }
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HelperRunError::new(
            HelperRunErrorKind::ExitFailed,
            format!(
                "{} helper {} exited with status {:?}: {}",
                kind.label(),
                helper.display(),
                output.status.code(),
                bounded(stderr.trim())
            ),
        ));
    }

    let envelope = HelperEnvelope::from_json(stdout.trim()).map_err(|error| {
        HelperRunError::new(
            HelperRunErrorKind::InvalidEnvelope,
            format!(
                "{} helper {} returned invalid JSON envelope: {}",
                kind.label(),
                helper.display(),
                error
            ),
        )
    })?;
    envelope.decode::<T>().map_err(|error| {
        HelperRunError::new(
            HelperRunErrorKind::InvalidPayload,
            format!(
                "{} helper {} returned an invalid payload: {}",
                kind.label(),
                helper.display(),
                error
            ),
        )
    })
}

pub(crate) fn resolve_helper(state: &AppState, kind: HelperKind) -> PathBuf {
    if let Some(path) = kind.configured_path(state) {
        return path;
    }
    if let Ok(value) = env::var(kind.env_var()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let installed = PathBuf::from("/usr/lib/kanban").join(kind.binary_name());
    if installed.exists() {
        return installed;
    }
    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let sibling = dir.join(kind.binary_name());
            if sibling.exists() {
                return sibling;
            }
        }
    }
    PathBuf::from(kind.binary_name())
}

fn bounded(value: &str) -> String {
    const MAX: usize = 240;
    let mut value = value.replace(['\r', '\n'], " ");
    if value.len() > MAX {
        value.truncate(MAX);
        value.push_str("...");
    }
    value
}
