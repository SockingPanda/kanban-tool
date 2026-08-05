use kanban_contract::GetRunLogResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunLogArgs {
    /// Global r_... run id.
    run_id: String,
}

#[tool_router(router = run_log_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "run_log",
        description = "Read a task run log through the canonical kanban application service"
    )]
    async fn run_log(
        &self,
        Parameters(args): Parameters<RunLogArgs>,
    ) -> Result<Json<GetRunLogResponse>, McpError> {
        let client = self.client.clone();
        let log = call_client(move || client.get_run_log(&args.run_id)).await?;
        Ok(Json(GetRunLogResponse { data: log }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn run_log_tool_is_independently_locatable() {
        let tools = KanbanMcp::run_log_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "run_log");
    }
}
