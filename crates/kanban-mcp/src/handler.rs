//! 仅实现 MCP 2026-07-28。业务 tool 的 DTO 和实现由现有 router 持有。

use std::borrow::Cow;

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CompleteRequestParams, CompleteResult,
        DiscoverResult, ErrorCode, GetPromptRequestParams, GetPromptResponse,
        InitializeRequestParams, InitializeResult, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ReadResourceRequestParams, ReadResourceResponse, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};
use serde::Serialize;

use crate::{
    bounded::json_size,
    metadata::{INSTRUCTIONS, LIST_TTL_MS, SUPPORTED, implementation, response_meta},
    pagination,
    shared::KanbanMcp,
};

fn require_latest(context: &RequestContext<RoleServer>) -> Result<(), McpError> {
    let version = context.meta.protocol_version().ok_or_else(|| {
        McpError::invalid_params(
            "每个请求都需要 _meta.io.modelcontextprotocol/protocolVersion",
            None,
        )
    })?;
    if version != ProtocolVersion::V_2026_07_28 {
        return Err(McpError::unsupported_protocol_version(version, &SUPPORTED));
    }
    if context.meta.client_capabilities().is_none() {
        return Err(McpError::invalid_params(
            "每个请求都需要 _meta.io.modelcontextprotocol/clientCapabilities",
            None,
        ));
    }
    Ok(())
}

impl KanbanMcp {
    fn list_page<T: Clone + Serialize>(
        &self,
        items: &[T],
        params: &Option<PaginatedRequestParams>,
        scope: &str,
    ) -> Result<(Vec<T>, Option<String>), McpError> {
        pagination::page(
            items,
            params.as_ref().and_then(|params| params.cursor.as_deref()),
            scope,
            &self.policy.stamp,
            self.config.limits.page_size,
            self.config.limits.max_result_bytes,
        )
    }
}

impl ServerHandler for KanbanMcp {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&SUPPORTED)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.policy
            .tools
            .iter()
            .find(|tool| tool.name.as_ref() == name)
            .cloned()
    }

    async fn ping(&self, context: RequestContext<RoleServer>) -> Result<(), McpError> {
        require_latest(&context)?;
        Err(McpError::new(
            ErrorCode::METHOD_NOT_FOUND,
            "ping 未启用；使用 server/discover 检查协议端点",
            None,
        ))
    }

    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(Default::default());
        capabilities.resources = Some(Default::default());
        capabilities.prompts = Some(Default::default());
        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(implementation())
            .with_instructions(INSTRUCTIONS)
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Err(McpError::new(
            ErrorCode::METHOD_NOT_FOUND,
            "仅支持 MCP 2026-07-28；使用 server/discover 和逐请求元数据",
            None,
        ))
    }

    async fn discover(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, McpError> {
        require_latest(&context)?;
        Ok(
            DiscoverResult::from_server_info(SUPPORTED.to_vec(), self.get_info())
                .with_ttl_ms(LIST_TTL_MS)
                .with_cache_scope(CacheScope::Private),
        )
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        require_latest(&context)?;
        let (tools, next) = self.list_page(&self.policy.tools, &request, "tools")?;
        let mut result = ListToolsResult::with_all_items(tools)
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Private);
        result.next_cursor = next;
        result.meta = Some(response_meta());
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        require_latest(&context)?;
        let name = request.name.to_string();
        let result = self.dispatch(request, context).await;
        self.finish_tool(&name, result)
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        require_latest(&context)?;
        let (resources, next) = self.list_page(&self.resource_catalog(), &request, "resources")?;
        let mut result = ListResourcesResult::with_all_items(resources)
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Private);
        result.next_cursor = next;
        result.meta = Some(response_meta());
        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        require_latest(&context)?;
        let (templates, next) =
            self.list_page(&self.resource_templates(), &request, "templates")?;
        let mut result = ListResourceTemplatesResult::with_all_items(templates)
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Private);
        result.next_cursor = next;
        result.meta = Some(response_meta());
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        require_latest(&context)?;
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(McpError::invalid_params(
                "当前资源不接受 MRTR continuation",
                None,
            ));
        }
        Ok(self.read_uri(&request.uri, context).await?.into())
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        require_latest(&context)?;
        let (prompts, next) = self.list_page(&self.prompt_catalog(), &request, "prompts")?;
        let mut result = ListPromptsResult::with_all_items(prompts)
            .with_ttl_ms(LIST_TTL_MS)
            .with_cache_scope(CacheScope::Private);
        result.next_cursor = next;
        result.meta = Some(response_meta());
        Ok(result)
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        require_latest(&context)?;
        if request.input_responses.is_some() || request.request_state.is_some() {
            return Err(McpError::invalid_params(
                "当前提示模板不接受 MRTR continuation",
                None,
            ));
        }
        if json_size(&request.arguments, self.config.limits.max_arguments_bytes)?.is_none() {
            return Err(McpError::invalid_params("提示模板参数超过限额", None));
        }
        let result = self.render_prompt(request)?;
        if json_size(&result, self.config.limits.max_result_bytes)?.is_none() {
            return Err(McpError::invalid_params(
                "提示模板结果超过限额，请缩短 objective",
                None,
            ));
        }
        Ok(result.into())
    }

    async fn complete(
        &self,
        _request: CompleteRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        require_latest(&context)?;
        Err(McpError::new(
            ErrorCode::METHOD_NOT_FOUND,
            "completion/complete 未启用",
            None,
        ))
    }
}
