use std::{env, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};
use kanban_derived_io::HelperEnvelope;
use serde::de::DeserializeOwned;

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

    fn label(self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::Graph => "graph",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct HelperErrorPayload {
    code: String,
    message: String,
}

pub(crate) fn resolve_helper(kind: HelperKind) -> PathBuf {
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

pub(crate) fn run_helper_json<T>(kind: HelperKind, args: &[String]) -> Result<T>
where
    T: DeserializeOwned,
{
    let helper = resolve_helper(kind);
    let output = Command::new(&helper).args(args).output().with_context(|| {
        format!(
            "failed to run {} helper {} (set {} to override)",
            kind.label(),
            helper.display(),
            kind.env_var()
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        if let Ok(envelope) = HelperEnvelope::from_json(stdout.trim()) {
            if let Ok(error) = envelope.decode::<HelperErrorPayload>() {
                bail!(
                    "{} helper failed: {} ({})",
                    kind.label(),
                    error.message,
                    error.code
                );
            }
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{} helper {} exited with status {:?}: {}",
            kind.label(),
            helper.display(),
            output.status.code(),
            bounded(stderr.trim())
        );
    }

    let envelope = HelperEnvelope::from_json(stdout.trim()).with_context(|| {
        format!(
            "{} helper {} returned invalid JSON envelope",
            kind.label(),
            helper.display()
        )
    })?;
    envelope.decode::<T>().with_context(|| {
        format!(
            "{} helper {} returned an invalid payload",
            kind.label(),
            helper.display()
        )
    })
}

pub(crate) fn helper_missing_message(kind: HelperKind, error: &anyhow::Error) -> String {
    format!(
        "{} helper unavailable: {}; set {} or install /usr/lib/kanban/{}",
        kind.label(),
        bounded(&error.to_string()),
        kind.env_var(),
        kind.binary_name()
    )
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
