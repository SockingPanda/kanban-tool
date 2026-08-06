use kanban_protocol::GetRunResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunShowArgs {
    run_id: String,
}

#[tool_router(router = run_show_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "run_show",
        description = "通过 canonical kanban application service 查看一条执行记录"
    )]
    async fn run_show(
        &self,
        Parameters(args): Parameters<RunShowArgs>,
    ) -> Result<Json<GetRunResponse>, McpError> {
        let client = self.client.clone();
        let run = call_client(move || client.get_run(&args.run_id)).await?;
        Ok(Json(GetRunResponse { data: run }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn run_show_tool_is_independently_locatable() {
        let tools = KanbanMcp::run_show_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "run_show");
    }
}
