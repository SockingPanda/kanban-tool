use crate::shared::{KanbanMcp, call_client_internal};
use kanban_contract::{DataEnvelope, VectorQuery, VectorQueryLabelAtomsResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    q: String,
    board: Option<String>,
    limit: Option<usize>,
    polarity: Option<String>,
}

#[tool_router(router = vector_query_label_atoms_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "vector_query_label_atoms",
        description = "Query canonical label atoms by cosine similarity"
    )]
    async fn vector_query_label_atoms(
        &self,
        Parameters(args): Parameters<Args>,
    ) -> Result<Json<VectorQueryLabelAtomsResponse>, McpError> {
        let query = VectorQuery {
            board: self.board(args.board),
            q: args.q,
            limit: args.limit.unwrap_or(20),
            embedding_model: None,
            polarity: args.polarity,
            include_vector: false,
        };
        let client = self.client.clone();
        let hits = call_client_internal(move || client.query_vector_label_atoms(query)).await?;
        Ok(Json(DataEnvelope::new(hits)))
    }
}
