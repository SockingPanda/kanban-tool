use kanban_client::ClientError;

use crate::config::ConfigError;

#[derive(Debug, serde::Serialize)]
pub(crate) struct CliErrorBody<'a> {
    pub(crate) code: &'a str,
    pub(crate) message: String,
    pub(crate) exit_code: u8,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct CliErrorEnvelope<'a> {
    pub(crate) error: CliErrorBody<'a>,
}

#[derive(Debug)]
pub(crate) struct CliFailure {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) exit_code: u8,
}

impl From<ClientError> for CliFailure {
    fn from(error: ClientError) -> Self {
        let code = error.code();
        let exit_code = match code {
            "not_found" => 3,
            "invalid_transition"
            | "execution_plan_required"
            | "steps_incomplete"
            | "dependency_cycle" => 4,
            "claim_conflict" | "claim_token_mismatch" | "idempotency_conflict" => 5,
            "dependency_blocked" => 6,
            "server_unavailable" => 9,
            "feature_not_available" => 10,
            "invalid_input" | "invalid_response" => 2,
            _ => 1,
        };
        Self {
            code,
            message: error.to_string(),
            exit_code,
        }
    }
}

impl From<ConfigError> for CliFailure {
    fn from(error: ConfigError) -> Self {
        let message = error.to_string();
        let (code, exit_code) = match error {
            ConfigError::Parse { .. } => ("invalid_input", 2),
            ConfigError::Io { .. } | ConfigError::Serialize { .. } => ("generic_error", 1),
        };
        Self {
            code,
            message,
            exit_code,
        }
    }
}

pub(crate) fn feature_not_available(message: impl Into<String>) -> CliFailure {
    CliFailure {
        code: "feature_not_available",
        message: message.into(),
        exit_code: 10,
    }
}
