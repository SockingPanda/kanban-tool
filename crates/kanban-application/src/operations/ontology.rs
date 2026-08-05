//! 标签本体的 application boundary。
//!
//! 这里不携带 Turso row 或 HTTP DTO；host adapter 负责把稳定的 JSON
//! operation payload 转换成 canonical store input。这样 CLI、MCP 与 HTTP
//! 仍然共享同一条 application mutation gate。

use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};
use serde_json::Value;

use crate::{ApplicationService, ApplicationStore};

/// 由唯一 host 实现的标签本体 operation port。
pub trait LabelOntologyOperations: ApplicationStore {
    fn label_ontology(
        &self,
        operation: &str,
        board: &str,
        input: Value,
    ) -> impl Future<Output = Result<Value>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: LabelOntologyOperations,
    C: Clock,
{
    /// 执行一个已登记的 ontology operation。
    pub async fn label_ontology(
        &self,
        operation: &str,
        board: &str,
        input: Value,
    ) -> Result<Value> {
        let operation = operation.trim();
        if operation.is_empty() {
            return Err(KanbanError::InvalidInput(
                "label ontology operation is required".to_owned(),
            ));
        }
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput(
                "board is required for label ontology operation".to_owned(),
            ));
        }
        let mutating = matches!(
            operation,
            "upsert_semantics"
                | "delete_semantics"
                | "rebuild_atom_index"
                | "propose_label"
                | "decide_proposal"
                | "record_observation"
                | "create_action"
                | "apply_atom"
                | "revert_mutation"
                | "validate_action"
        );
        if mutating {
            let _mutation = self.mutation_gate.lock().await;
            self.store.label_ontology(operation, board, input).await
        } else {
            self.store.label_ontology(operation, board, input).await
        }
    }
}
