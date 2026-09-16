//! 读取资源和执行工具共用的调度限制；此处不实现领域状态机或重试。

use std::time::Duration;

use rmcp::{
    ErrorData as McpError, RoleServer,
    handler::server::tool::ToolCallContext,
    model::{CallToolRequestParams, CallToolResponse, ErrorCode, ResultType},
    service::RequestContext,
};

use crate::{
    bounded::json_size,
    errors::{Failure, OperationStatus},
    metadata::response_meta,
    shared::KanbanMcp,
};

impl KanbanMcp {
    pub(crate) async fn dispatch(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if !self.policy.allows(request.name.as_ref()) {
            return Err(McpError::invalid_params("工具未启用或不存在", None));
        }
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(McpError::invalid_params(
                "当前工具不接受 MRTR continuation",
                None,
            ));
        }
        if json_size(&request.arguments, self.config.limits.max_arguments_bytes)?.is_none() {
            return Err(McpError::invalid_params(
                "工具参数超过 MCP 配置限额；大附件请改用 host 所属的附件上传入口",
                None,
            ));
        }
        let _permit = self.in_flight.try_acquire().map_err(|_| {
            Failure::new(
                "busy",
                "MCP 当前并发已满，请稍后重试",
                OperationStatus::NotStarted,
                true,
            )
            .into_internal()
        })?;
        let cancellation = context.ct.clone();
        let call = ToolCallContext::new(self, request, context);
        // 取消后丢弃 handler/gRPC Future。SDK 发送层会丢弃对应响应。
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                Err(Failure::new("cancelled", "调用已取消，写入结果未知", OperationStatus::Unknown, false).into_internal())
            }
            result = tokio::time::timeout(Duration::from_millis(self.config.limits.timeout_ms), self.router.call(call)) => {
                result.unwrap_or_else(|_| Err(Failure::new("timeout", "调用超时，写入结果未知", OperationStatus::Unknown, true).into_internal()))
            }
        }
    }

    pub(crate) fn finish_tool(
        &self,
        name: &str,
        result: Result<CallToolResponse, McpError>,
    ) -> Result<CallToolResponse, McpError> {
        let read_only = self.policy.is_read_only(name);
        let mut result = match result {
            Ok(CallToolResponse::Complete(result)) => result,
            Ok(_) => {
                return Err(McpError::internal_error(
                    "工具返回了当前未启用的 MCP 结果类型",
                    None,
                ));
            }
            Err(error) => {
                if let Some(failure) = Failure::from_internal(&error) {
                    return Ok(failure.into_tool_result(read_only, name).into());
                }
                if error.code == ErrorCode::INTERNAL_ERROR {
                    return Ok(Failure::new(
                        "internal",
                        "工具执行发生内部错误，写入结果未知",
                        OperationStatus::Unknown,
                        false,
                    )
                    .into_tool_result(read_only, name)
                    .into());
                }
                return Err(error);
            }
        };
        result.result_type = Some(ResultType::COMPLETE);
        let meta = result.meta.get_or_insert_default();
        for (key, value) in response_meta().iter() {
            meta.insert(key.clone(), value.clone());
        }
        if json_size(&result, self.config.limits.max_result_bytes)?.is_none() {
            let status = if result.is_error == Some(true) {
                OperationStatus::Unknown
            } else {
                OperationStatus::Succeeded
            };
            return Ok(Failure::new(
                "result_too_large",
                "调用已经返回，结果超过 MCP 输出限额；请核对操作结果，勿重复提交写请求",
                status,
                false,
            )
            .into_tool_result(read_only, name)
            .into());
        }
        Ok(result.into())
    }
}
