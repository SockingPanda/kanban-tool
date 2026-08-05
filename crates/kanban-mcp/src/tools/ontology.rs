//! MCP tools for label semantics, atoms, proposals and ontology ledger.

use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::shared::{KanbanMcp, call_client_internal};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct OntologyArgs {
    board: Option<String>,
    reference: Option<String>,
    task_ref: Option<String>,
    proposal_id: Option<String>,
    signal_id: Option<String>,
    q: Option<String>,
    polarity: Option<String>,
    limit: Option<usize>,
    payload: Option<Value>,
}

async fn call<F>(operation: F) -> Result<Json<Value>, McpError>
where
    F: FnOnce() -> Result<Value, kanban_client::ClientError> + Send + 'static,
{
    Ok(Json(call_client_internal(operation).await?))
}

#[tool_router(router = ontology_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "label_semantics_list",
        description = "List canonical label semantics"
    )]
    async fn label_semantics_list(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.list_label_semantics(&board)).await
    }

    #[tool(
        name = "label_semantics_show",
        description = "Show one label semantics record"
    )]
    async fn label_semantics_show(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let reference = args.reference.unwrap_or_default();
        call(move || client.get_label_semantics(&board, &reference)).await
    }

    #[tool(
        name = "label_semantics_upsert",
        description = "CAS upsert label semantics"
    )]
    async fn label_semantics_upsert(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let reference = args.reference.unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.upsert_label_semantics(&board, &reference, payload)).await
    }

    #[tool(
        name = "label_semantics_delete",
        description = "CAS delete label semantics"
    )]
    async fn label_semantics_delete(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let reference = args.reference.unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        let expected = payload
            .get("expected_semantics_hash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let reason = payload
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("delete label semantics")
            .to_owned();
        call(move || client.delete_label_semantics(&board, &reference, &expected, &reason)).await
    }

    #[tool(name = "label_atoms_list", description = "List canonical label atoms")]
    async fn label_atoms_list(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.list_label_atoms(&board)).await
    }

    #[tool(name = "label_atom_explain", description = "Explain atom provenance")]
    async fn label_atom_explain(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let reference = args.reference.unwrap_or_default();
        call(move || client.explain_label_atom(&board, &reference)).await
    }

    #[tool(
        name = "label_atom_index_status",
        description = "Inspect degraded atom index status"
    )]
    async fn label_atom_index_status(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.label_atom_index_status(&board)).await
    }

    #[tool(
        name = "label_atom_index_rebuild",
        description = "Rebuild atom index projection"
    )]
    async fn label_atom_index_rebuild(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.rebuild_label_atom_index(&board)).await
    }

    #[tool(
        name = "label_atom_index_query",
        description = "Query atom index with degraded fallback"
    )]
    async fn label_atom_index_query(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || {
            client.query_label_atom_index(
                &board,
                args.q.as_deref(),
                args.polarity.as_deref(),
                args.limit.unwrap_or(24),
            )
        })
        .await
    }

    #[tool(name = "label_suggest", description = "Suggest labels for a task")]
    async fn label_suggest(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let task = args.task_ref.unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.suggest_task_labels(&task, Some(&board), payload)).await
    }

    #[tool(name = "label_propose", description = "Propose a task label")]
    async fn label_propose(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let task = args.task_ref.unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.propose_task_label(&board, &task, payload)).await
    }

    #[tool(
        name = "label_proposals_list",
        description = "List task label proposals"
    )]
    async fn label_proposals_list(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.list_label_proposals(&board, args.task_ref.as_deref(), None)).await
    }

    #[tool(name = "label_proposal_show", description = "Show one label proposal")]
    async fn label_proposal_show(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        call(move || client.get_label_proposal(&proposal)).await
    }

    #[tool(
        name = "label_proposal_accept",
        description = "Accept a label proposal"
    )]
    async fn label_proposal_accept(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.decide_label_proposal(&proposal, true, payload)).await
    }

    #[tool(
        name = "label_proposal_reject",
        description = "Reject a label proposal"
    )]
    async fn label_proposal_reject(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.decide_label_proposal(&proposal, false, payload)).await
    }

    #[tool(name = "label_ontology_signals", description = "List ontology signals")]
    async fn label_ontology_signals(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.list_label_ontology_signals(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_signal_show",
        description = "Show one ontology signal"
    )]
    async fn label_ontology_signal_show(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let signal = args.signal_id.or(args.reference).unwrap_or_default();
        call(move || client.get_label_ontology_signal(&signal)).await
    }

    #[tool(
        name = "label_ontology_review",
        description = "Review ontology signal groups"
    )]
    async fn label_ontology_review(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.review_label_ontology(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_action",
        description = "Record a confirm/reject/resolve ontology action"
    )]
    async fn label_ontology_action(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.create_label_ontology_action(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_apply_atom",
        description = "Apply an atom mutation"
    )]
    async fn label_ontology_apply_atom(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.apply_label_ontology_atom(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_revert",
        description = "Revert an ontology mutation with baseline CAS"
    )]
    async fn label_ontology_revert(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.revert_label_ontology(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_validate",
        description = "Validate an ontology action"
    )]
    async fn label_ontology_validate(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.validate_label_ontology(&board, payload)).await
    }

    #[tool(
        name = "label_ontology_quality",
        description = "Report ontology quality evidence"
    )]
    async fn label_ontology_quality(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.label_ontology_quality(&board, args.limit.unwrap_or(20))).await
    }

    #[tool(
        name = "label_ontology_observe",
        description = "Record an ontology observation and signals"
    )]
    async fn label_ontology_observe(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let task = args.task_ref.unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.record_label_ontology_observation(&board, &task, payload)).await
    }
}
