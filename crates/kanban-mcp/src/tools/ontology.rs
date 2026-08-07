//! 提供 label semantics、atoms、proposals 和 ontology ledger 的 MCP tools。

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
        description = "列出 canonical label semantics"
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
        description = "查看一条 label semantics 记录"
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
        description = "使用 CAS upsert label semantics"
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
        description = "使用 CAS 删除 label semantics"
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

    #[tool(name = "label_atoms_list", description = "列出 canonical label atoms")]
    async fn label_atoms_list(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.list_label_atoms(&board)).await
    }

    #[tool(name = "label_atom_explain", description = "解释 atom provenance")]
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
        description = "查看 degraded atom index 状态"
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
        description = "重建 atom index projection"
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
        description = "查询 atom index，必要时使用 degraded fallback"
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

    #[tool(name = "label_suggest", description = "为任务建议 labels")]
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

    #[tool(name = "label_propose", description = "提出任务 label")]
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
        description = "列出任务 label proposals"
    )]
    async fn label_proposals_list(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        call(move || client.list_label_proposals(&board, args.task_ref.as_deref(), None)).await
    }

    #[tool(name = "label_proposal_show", description = "查看一条 label proposal")]
    async fn label_proposal_show(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        call(move || client.get_label_proposal(&proposal)).await
    }

    #[tool(name = "label_proposal_accept", description = "接受 label proposal")]
    async fn label_proposal_accept(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.decide_label_proposal(&proposal, true, payload)).await
    }

    #[tool(name = "label_proposal_reject", description = "拒绝 label proposal")]
    async fn label_proposal_reject(
        &self,
        Parameters(args): Parameters<OntologyArgs>,
    ) -> Result<Json<Value>, McpError> {
        let client = self.client.clone();
        let proposal = args.proposal_id.or(args.reference).unwrap_or_default();
        let payload = args.payload.unwrap_or_else(|| json!({}));
        call(move || client.decide_label_proposal(&proposal, false, payload)).await
    }

    #[tool(name = "label_ontology_signals", description = "列出 ontology signals")]
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
        description = "查看一条 ontology signal"
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
        description = "审阅 ontology signal groups"
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
        description = "记录 confirm/reject/resolve ontology action"
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

    #[tool(name = "label_ontology_apply_atom", description = "应用 atom mutation")]
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
        description = "使用 baseline CAS 回滚 ontology mutation"
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

    #[tool(name = "label_ontology_validate", description = "校验 ontology action")]
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
        description = "报告 ontology quality evidence"
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
        description = "记录 ontology observation 和 signals"
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
