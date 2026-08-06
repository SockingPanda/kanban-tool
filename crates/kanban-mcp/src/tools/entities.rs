use kanban_protocol::{DataEnvelope, EntityListResponse, EntityResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

const fn default_limit() -> usize {
    100
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct EntityListArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
    kind: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EntityShowArgs {
    uri: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EntityUpsertArgs {
    uri: String,
    kind: String,
    source_table: String,
    source_id: String,
    board: Option<String>,
    task_id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    content_hash: Option<String>,
    archived_at: Option<i64>,
}

impl From<EntityUpsertArgs> for kanban_client::EntityUpsertRequest {
    fn from(args: EntityUpsertArgs) -> Self {
        Self {
            uri: args.uri,
            kind: args.kind,
            source_table: args.source_table,
            source_id: args.source_id,
            board: args.board,
            task_id: args.task_id,
            title: args.title,
            summary: args.summary,
            content_hash: args.content_hash,
            archived_at: args.archived_at,
        }
    }
}

#[tool_router(router = entity_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "entity_list", description = "列出 canonical entity records")]
    async fn entity_list(
        &self,
        Parameters(args): Parameters<EntityListArgs>,
    ) -> Result<Json<EntityListResponse>, McpError> {
        let board = args.board.or_else(|| Some(self.default_board.to_string()));
        let kind = args.kind;
        let limit = args.limit;
        let client = self.client.clone();
        let entities =
            call_client(move || client.list_entities(board.as_deref(), kind.as_deref(), limit))
                .await?;
        Ok(Json(DataEnvelope::new(entities)))
    }

    #[tool(name = "entity_show", description = "查看一条 canonical entity record")]
    async fn entity_show(
        &self,
        Parameters(args): Parameters<EntityShowArgs>,
    ) -> Result<Json<EntityResponse>, McpError> {
        let client = self.client.clone();
        let entity = call_client(move || client.get_entity(&args.uri)).await?;
        Ok(Json(DataEnvelope::new(entity)))
    }

    #[tool(
        name = "entity_upsert",
        description = "通过 canonical application service upsert entity"
    )]
    async fn entity_upsert(
        &self,
        Parameters(args): Parameters<EntityUpsertArgs>,
    ) -> Result<Json<EntityResponse>, McpError> {
        let client = self.client.clone();
        let request: kanban_client::EntityUpsertRequest = args.into();
        let entity = call_client(move || client.upsert_entity(request)).await?;
        Ok(Json(DataEnvelope::new(entity)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn entity_tools_are_independently_locatable() {
        let names = KanbanMcp::entity_tools()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["entity_list", "entity_show", "entity_upsert"]);
    }
}
