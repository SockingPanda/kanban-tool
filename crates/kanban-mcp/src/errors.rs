//! 将 client 错误带过既有 typed tool 返回类型，再在 MCP 边界输出 tool error。
//! 标记只在进程内部使用，不分配新的 JSON-RPC 保留错误码。

use kanban_client::ClientError;
use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, ContentBlock, ErrorCode},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::metadata::{ERROR_KEY, response_meta};

const INTERNAL_MARKER: &str = "kanban_mcp_client_failure";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OperationStatus {
    NotStarted,
    Rejected,
    Unknown,
    Succeeded,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Failure {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) operation_status: OperationStatus,
    pub(crate) transient: bool,
}

impl Failure {
    pub(crate) fn new(code: &str, message: &str, status: OperationStatus, transient: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            operation_status: status,
            transient,
        }
    }

    pub(crate) fn from_client(error: ClientError) -> Self {
        let code = error.code().to_owned();
        let transient =
            code == "server_unavailable" || code == "stream_error" || code == "stream_closed";
        let (message, status) = match error {
            ClientError::Api { message, .. }
                if code != "internal" && code != "server_unavailable" =>
            {
                (
                    message.chars().take(1024).collect(),
                    OperationStatus::Rejected,
                )
            }
            ClientError::InvalidInput(_) | ClientError::InvalidServerUrl(_) => (
                "请求未通过本地 client 校验，请核对标识和参数".into(),
                OperationStatus::NotStarted,
            ),
            ClientError::ServerUnavailable(_)
            | ClientError::StreamClosed
            | ClientError::StreamRead(_) => (
                "本地 host 不可用或调用中断；写请求的持久化结果尚未确认".into(),
                OperationStatus::Unknown,
            ),
            _ => (
                "host 返回无效响应或内部错误；写请求的持久化结果尚未确认".into(),
                OperationStatus::Unknown,
            ),
        };
        Self {
            code,
            message,
            operation_status: status,
            transient,
        }
    }

    pub(crate) fn into_internal(self) -> McpError {
        McpError::internal_error(
            "kanban client operation failed",
            Some(json!({ (INTERNAL_MARKER): self })),
        )
    }

    pub(crate) fn from_internal(error: &McpError) -> Option<Self> {
        serde_json::from_value(error.data.as_ref()?.get(INTERNAL_MARKER)?.clone()).ok()
    }

    pub(crate) fn into_tool_result(mut self, read_only: bool, tool: &str) -> CallToolResult {
        if read_only {
            self.operation_status = OperationStatus::NotApplicable;
        }
        let next_step = match self.code.as_str() {
            "not_found" => "使用读取工具核对对象标识和看板。",
            "conflict" | "invalid_transition" | "claim_conflict" | "idempotency_conflict" => {
                "重新读取当前状态和已有执行记录，再决定下一步。"
            }
            "result_too_large" => "先核对操作结果，再使用更窄的读取；不要重复提交写操作。",
            "server_unavailable" | "timeout" => {
                "检查 kanban serve。写请求先核对持久化结果，不要盲目重试。"
            }
            "busy" => "当前请求尚未执行，可稍后重新发起。",
            _ => "核对工具参数、当前对象状态及执行记录。",
        };
        let payload = json!({
            "code": self.code,
            "message": self.message,
            "tool": tool,
            "operation_status": self.operation_status,
            "retry_safe": (read_only && self.transient) || self.operation_status == OperationStatus::NotStarted,
            "next_step": next_step
        });
        let mut meta = response_meta();
        meta.insert(ERROR_KEY.to_owned(), payload.clone());
        // 不把错误对象塞进要求领域 DTO 的 structuredContent/outputSchema。
        CallToolResult::error(vec![ContentBlock::text(payload.to_string())]).with_meta(Some(meta))
    }
}

pub(crate) fn resource_error(error: McpError) -> McpError {
    let Some(failure) = Failure::from_internal(&error) else {
        return error;
    };
    let code = if matches!(failure.code.as_str(), "not_found" | "invalid_input") {
        ErrorCode::INVALID_PARAMS
    } else {
        ErrorCode::INTERNAL_ERROR
    };
    McpError::new(
        code,
        failure.message.clone(),
        Some(json!({
            "code": failure.code,
            "retry_safe": failure.transient
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_write_is_not_safe_to_retry_and_hides_transport_details() {
        let result = Failure::from_client(ClientError::ServerUnavailable(
            "http://secret:token@host".into(),
        ))
        .into_tool_result(false, "task_create");
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["isError"], true);
        assert_eq!(value["_meta"][ERROR_KEY]["operation_status"], "unknown");
        assert_eq!(value["_meta"][ERROR_KEY]["retry_safe"], false);
        assert!(value.get("structuredContent").is_none());
        assert!(!value.to_string().contains("token@host"));
    }

    #[test]
    fn missing_resource_uses_current_invalid_params_code() {
        let error = Failure::new("not_found", "资源不存在", OperationStatus::Rejected, false)
            .into_internal();
        assert_eq!(resource_error(error).code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn protocol_errors_are_not_misclassified_as_business_errors() {
        let error = McpError::invalid_params("unknown tool", None);
        assert!(Failure::from_internal(&error).is_none());
    }

    #[test]
    fn read_failure_and_not_started_failure_can_be_retried() {
        for (read_only, failure) in [
            (
                true,
                Failure::new("timeout", "超时", OperationStatus::Unknown, true),
            ),
            (
                false,
                Failure::new("busy", "繁忙", OperationStatus::NotStarted, true),
            ),
        ] {
            let result = failure.into_tool_result(read_only, "example");
            assert_eq!(result.meta.unwrap()[ERROR_KEY]["retry_safe"], true);
        }
    }
}
