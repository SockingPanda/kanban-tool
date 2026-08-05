use std::collections::BTreeMap;

use kanban_protocol::{
    ConfirmSignalsResponse, GetSignalResponse, ListSignalsResponse, RecordSignalRequest,
    RecordSignalResponse, RejectSignalsResponse, ResolveSignalsResponse, ReviewSignalsRequest,
    ReviewSignalsResponse, SignalCommentRequest, SignalQuery, SupersedeSignalsResponse,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

fn default_limit() -> usize {
    100
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalRecordArgs {
    board: Option<String>,
    kind: String,
    title: String,
    summary: String,
    severity: Option<String>,
    task_ref: Option<String>,
    task_id: Option<String>,
    run_id: Option<String>,
    comment_id: Option<String>,
    agent_type: Option<String>,
    dedupe_key: Option<String>,
    source: Option<String>,
    evidence: Option<BTreeMap<String, serde_json::Value>>,
    comment_body: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalListArgs {
    board: Option<String>,
    #[serde(default)]
    status: Vec<String>,
    #[serde(default)]
    kind: Vec<String>,
    task_ref: Option<String>,
    #[serde(default)]
    include_all: bool,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalShowArgs {
    signal_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalReviewArgs {
    board: Option<String>,
    #[serde(default)]
    status: Vec<String>,
    #[serde(default)]
    kind: Vec<String>,
    task_ref: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalActionArgs {
    board: Option<String>,
    signal_ids: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SignalSupersedeArgs {
    board: Option<String>,
    signal_ids: Vec<String>,
    by: String,
    reason: String,
}

#[tool_router(router = signal_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "signal_record",
        description = "Record a generic signal through the canonical application service"
    )]
    async fn signal_record(
        &self,
        Parameters(args): Parameters<SignalRecordArgs>,
    ) -> Result<Json<RecordSignalResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let request = RecordSignalRequest {
            kind: args.kind,
            title: args.title,
            summary: args.summary,
            severity: args.severity,
            task_ref: args.task_ref,
            task_id: args.task_id,
            run_id: args.run_id,
            comment_id: args.comment_id,
            actor: None,
            agent_type: args.agent_type,
            dedupe_key: args.dedupe_key,
            source: args.source,
            evidence: args
                .evidence
                .map(kanban_protocol::structured_metadata::JsonObject),
            comment: args
                .comment_body
                .map(|body| SignalCommentRequest { body: Some(body) }),
        };
        let response = call_client(move || client.record_signal(&board, &request)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_list",
        description = "List generic signals from the canonical application service"
    )]
    async fn signal_list(
        &self,
        Parameters(args): Parameters<SignalListArgs>,
    ) -> Result<Json<ListSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let query = SignalQuery {
            status: args.status,
            kind: args.kind,
            task_ref: args.task_ref,
            include_all: args.include_all,
            limit: args.limit,
        };
        let response = call_client(move || client.list_signals(&board, &query)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_show",
        description = "Show one generic signal by global id"
    )]
    async fn signal_show(
        &self,
        Parameters(args): Parameters<SignalShowArgs>,
    ) -> Result<Json<GetSignalResponse>, McpError> {
        let client = self.client.clone();
        let response = call_client(move || client.get_signal(&args.signal_id)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_review",
        description = "List generic signals eligible for review"
    )]
    async fn signal_review(
        &self,
        Parameters(args): Parameters<SignalReviewArgs>,
    ) -> Result<Json<ReviewSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let query = SignalQuery {
            status: args.status,
            kind: args.kind,
            task_ref: args.task_ref,
            include_all: false,
            limit: args.limit,
        };
        let response = call_client(move || client.review_signals(&board, &query)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_confirm",
        description = "Confirm one or more generic signals"
    )]
    async fn signal_confirm(
        &self,
        Parameters(args): Parameters<SignalActionArgs>,
    ) -> Result<Json<ConfirmSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let request = action_request(args.signal_ids, args.reason);
        let response = call_client(move || client.confirm_signals(&board, &request)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_reject",
        description = "Reject one or more generic signals"
    )]
    async fn signal_reject(
        &self,
        Parameters(args): Parameters<SignalActionArgs>,
    ) -> Result<Json<RejectSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let request = action_request(args.signal_ids, args.reason);
        let response = call_client(move || client.reject_signals(&board, &request)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_resolve",
        description = "Resolve one or more generic signals"
    )]
    async fn signal_resolve(
        &self,
        Parameters(args): Parameters<SignalActionArgs>,
    ) -> Result<Json<ResolveSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let request = action_request(args.signal_ids, args.reason);
        let response = call_client(move || client.resolve_signals(&board, &request)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "signal_supersede",
        description = "Supersede one or more generic signals with another signal"
    )]
    async fn signal_supersede(
        &self,
        Parameters(args): Parameters<SignalSupersedeArgs>,
    ) -> Result<Json<SupersedeSignalsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let request = ReviewSignalsRequest {
            signal_ids: args.signal_ids,
            reason: args.reason,
            replacement_signal_id: Some(args.by),
            actor: None,
            expected_updated_at: None,
        };
        let response = call_client(move || client.supersede_signals(&board, &request)).await?;
        Ok(Json(response))
    }
}

fn action_request(signal_ids: Vec<String>, reason: String) -> ReviewSignalsRequest {
    ReviewSignalsRequest {
        signal_ids,
        reason,
        replacement_signal_id: None,
        actor: None,
        expected_updated_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn signal_tools_are_independently_locatable() {
        let tools = KanbanMcp::signal_tools().list_all();
        assert_eq!(tools.len(), 8);
        assert_eq!(tools[0].name.as_ref(), "signal_confirm");
        assert_eq!(tools[7].name.as_ref(), "signal_supersede");
    }
}
