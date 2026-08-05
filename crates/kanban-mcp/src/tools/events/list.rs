use kanban_contract::{ListEventsQuery, ListEventsResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

fn default_after() -> i64 {
    0
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EventListArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: Option<String>,
    #[serde(default = "default_after")]
    after: i64,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[tool_router(router = event_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "event_list",
        description = "List canonical task events through the application service"
    )]
    async fn event_list(
        &self,
        Parameters(args): Parameters<EventListArgs>,
    ) -> Result<Json<ListEventsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let after = args.after;
        let limit = args.limit;
        let client = self.client.clone();
        let response = call_client(move || {
            let task_id = task_ref
                .as_deref()
                .map(|selector| client.resolve_task_id(&board, selector))
                .transpose()?;
            client.list_events(&ListEventsQuery {
                board,
                task_id,
                after,
                limit,
            })
        })
        .await?;
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn event_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::event_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "event_list");
    }
}
