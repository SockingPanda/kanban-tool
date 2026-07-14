use std::{env, fmt, io, path::PathBuf, process::Command};

use kanban_contract::{GraphHelperErrorResponse, VectorHelperErrorResponse};
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
        let message = value.to_string();
        if value.kind() == HelperRunErrorKind::HelperError
            && message.contains("requires a configured embedding provider")
        {
            return ApiError(kanban_core::KanbanError::InvalidInput(message));
        }
        ApiError(kanban_core::KanbanError::Storage(message))
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
        if let Ok(envelope) = HelperEnvelope::from_json(stdout.trim())
            && let Some((code, message)) = decode_helper_error(kind, &envelope)
        {
            return Err(HelperRunError::new(
                HelperRunErrorKind::HelperError,
                format!("{} helper failed: {} ({})", kind.label(), message, code),
            ));
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

fn decode_helper_error(kind: HelperKind, envelope: &HelperEnvelope) -> Option<(String, String)> {
    match kind {
        HelperKind::Graph => envelope
            .decode::<GraphHelperErrorResponse>()
            .ok()
            .map(|error| (error.code, error.message)),
        HelperKind::Vector => envelope
            .decode::<VectorHelperErrorResponse>()
            .ok()
            .map(|error| (error.code, error.message)),
    }
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
    if let Ok(current_exe) = env::current_exe()
        && let Some(dir) = current_exe.parent()
    {
        let sibling = dir.join(kind.binary_name());
        if sibling.exists() {
            return sibling;
        }
    }
    if let Some(helper) = cargo_target_helper(kind) {
        return helper;
    }
    PathBuf::from(kind.binary_name())
}

fn cargo_target_helper(kind: HelperKind) -> Option<PathBuf> {
    ["KANBAN_CARGO_TARGET_ROOT", "CARGO_TARGET_DIR"]
        .into_iter()
        .filter_map(|key| {
            let value = env::var(key).ok()?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }
            let helper = PathBuf::from(trimmed)
                .join("release")
                .join(kind.binary_name());
            helper.is_file().then_some(helper)
        })
        .next()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvRestore {
        fn capture(key: &'static str) -> Self {
            Self {
                key,
                value: env::var(key).ok(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            unsafe {
                match &self.value {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }

    fn clean_helper_env() -> (MutexGuard<'static, ()>, Vec<EnvRestore>) {
        let guard = ENV_LOCK.lock().expect("env lock poisoned");
        let restores = vec![
            EnvRestore::capture("KANBAN_VECTOR_HELPER"),
            EnvRestore::capture("KANBAN_GRAPH_HELPER"),
            EnvRestore::capture("KANBAN_CARGO_TARGET_ROOT"),
            EnvRestore::capture("CARGO_TARGET_DIR"),
        ];
        unsafe {
            env::remove_var("KANBAN_VECTOR_HELPER");
            env::remove_var("KANBAN_GRAPH_HELPER");
            env::remove_var("KANBAN_CARGO_TARGET_ROOT");
            env::remove_var("CARGO_TARGET_DIR");
        }
        (guard, restores)
    }

    fn state() -> AppState {
        AppState::new("/tmp/kanban-test.db", "api-test")
    }

    #[test]
    fn resolve_helper_uses_kanban_cargo_target_root_release_helper() {
        let (_guard, _restore) = clean_helper_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let helper = temp
            .path()
            .join("release")
            .join(HelperKind::Vector.binary_name());
        std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create release");
        std::fs::write(&helper, "").expect("write helper");

        unsafe {
            env::set_var("KANBAN_CARGO_TARGET_ROOT", temp.path());
        }

        assert_eq!(resolve_helper(&state(), HelperKind::Vector), helper);
    }

    #[test]
    fn resolve_helper_uses_cargo_target_dir_release_helper() {
        let (_guard, _restore) = clean_helper_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let helper = temp
            .path()
            .join("release")
            .join(HelperKind::Graph.binary_name());
        std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create release");
        std::fs::write(&helper, "").expect("write helper");

        unsafe {
            env::set_var("CARGO_TARGET_DIR", temp.path());
        }

        assert_eq!(resolve_helper(&state(), HelperKind::Graph), helper);
    }

    #[test]
    fn resolve_helper_env_overrides_target_root_fallback() {
        let (_guard, _restore) = clean_helper_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let target_helper = temp
            .path()
            .join("release")
            .join(HelperKind::Vector.binary_name());
        std::fs::create_dir_all(target_helper.parent().expect("helper parent"))
            .expect("create release");
        std::fs::write(&target_helper, "").expect("write target helper");
        let env_helper = temp.path().join("env-helper");

        unsafe {
            env::set_var("KANBAN_CARGO_TARGET_ROOT", temp.path());
            env::set_var("KANBAN_VECTOR_HELPER", &env_helper);
        }

        assert_eq!(resolve_helper(&state(), HelperKind::Vector), env_helper);
    }
}
