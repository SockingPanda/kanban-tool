//! 资源只复用已证明为读取的 typed tool，不创建新的 client/数据库路径。

use rmcp::{
    ErrorData as McpError, RoleServer,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, ReadResourceResult, Resource,
        ResourceContents, ResourceTemplate,
    },
    service::RequestContext,
};
use serde_json::{Value, json};

use crate::{
    bounded::json_size,
    errors::resource_error,
    metadata::{INSTRUCTIONS, LIST_TTL_MS, PROTOCOL, response_meta},
    shared::KanbanMcp,
    uri::KanbanUri,
};

impl KanbanMcp {
    pub(crate) fn resource_catalog(&self) -> Vec<Resource> {
        vec![
            Resource::new(KanbanUri::Instructions.to_uri(), "server_instructions")
                .with_description("读取、操作、错误恢复与资料信任边界")
                .with_mime_type("text/markdown"),
            Resource::new(KanbanUri::Runtime.to_uri(), "server_runtime")
                .with_description("生效的 profile、默认看板与限额；不包含凭据或配置文件路径")
                .with_mime_type("application/json"),
        ]
    }

    pub(crate) fn resource_templates(&self) -> Vec<ResourceTemplate> {
        let mut templates = Vec::new();
        if self.policy.allows_resource_tool("board_show") {
            templates.push(
                ResourceTemplate::new("kanban://boards/{board}", "board")
                    .with_description("看板快照，board 使用 RFC 6570 simple expansion 编码")
                    .with_mime_type("application/json"),
            );
        }
        if self.policy.allows_resource_tool("task_show") {
            templates.push(
                ResourceTemplate::new("kanban://tasks/{task_id}", "task")
                    .with_description("全局 t_... ID 对应的任务详情，按需读取，不自动 claim")
                    .with_mime_type("application/json"),
            );
        }
        templates
    }

    pub(crate) fn runtime_info(&self) -> Value {
        json!({
            "protocol_version": PROTOCOL,
            "mcp_transport": "stdio",
            "backend_transport": "native_grpc",
            "profile": self.config.profile,
            "default_board": self.config.default_board,
            "enabled_tool_count": self.policy.tools.len(),
            "limits": self.config.limits,
            "subscriptions": false,
            "mcp_tasks_extension": false,
            "database_access": false
        })
    }

    pub(crate) async fn read_uri(
        &self,
        uri: &str,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let parsed = KanbanUri::parse(uri)?;
        let (content, ttl) = match parsed {
            KanbanUri::Instructions => (
                ResourceContents::text(INSTRUCTIONS, uri).with_mime_type("text/markdown"),
                LIST_TTL_MS,
            ),
            KanbanUri::Runtime => (
                ResourceContents::text(self.runtime_info().to_string(), uri)
                    .with_mime_type("application/json"),
                0,
            ),
            KanbanUri::Board(board) => (
                self.read_tool_resource("board_show", json!({"board": board}), uri, context)
                    .await?,
                0,
            ),
            KanbanUri::Task(task_id) => (
                self.read_tool_resource(
                    "task_show",
                    json!({"task_ref": task_id, "include_details": true}),
                    uri,
                    context,
                )
                .await?,
                0,
            ),
        };
        let mut result = ReadResourceResult::new(vec![content])
            .with_ttl_ms(ttl)
            .with_cache_scope(CacheScope::Private);
        result.meta = Some(response_meta());
        if json_size(&result, self.config.limits.max_result_bytes)?.is_none() {
            return Err(McpError::internal_error(
                "资源超过输出限额；请使用更窄的读取工具",
                None,
            ));
        }
        Ok(result)
    }

    async fn read_tool_resource(
        &self,
        name: &'static str,
        arguments: Value,
        uri: &str,
        context: RequestContext<RoleServer>,
    ) -> Result<ResourceContents, McpError> {
        if !self.policy.allows_resource_tool(name) {
            return Err(McpError::invalid_params("资源不存在或未启用", None));
        }
        let arguments = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| McpError::internal_error("资源 adapter 参数编码失败", None))?;
        let response = self
            .dispatch(
                CallToolRequestParams::new(name).with_arguments(arguments),
                context,
            )
            .await
            .map_err(resource_error)?;
        let data = match response {
            CallToolResponse::Complete(result) if result.is_error != Some(true) => {
                result.structured_content.ok_or_else(|| {
                    McpError::internal_error("读取工具未返回 structuredContent", None)
                })?
            }
            _ => return Err(McpError::internal_error("读取工具返回了无效资源结果", None)),
        };
        if json_size(&data, self.config.limits.max_result_bytes)?.is_none() {
            return Err(McpError::internal_error("资源内容超过输出限额", None));
        }
        let text = serde_json::to_string(&data)
            .map_err(|_| McpError::internal_error("资源 JSON 编码失败", None))?;
        Ok(ResourceContents::text(text, uri).with_mime_type("application/json"))
    }
}
