//! 可扩展对象系统。复用 canonical Turso、共享写锁、task_events 和现有 Task capability。
//! 类型、字段、双向关系、基数和行为绑定是数据。Task 执行器与通用生命周期操作器保留显式事务。
mod catalog;
mod commands;
mod diagnostics;
mod error;
pub mod files;
pub(crate) mod migration;
#[cfg(test)]
mod migration_tests;
mod model;
pub(crate) mod portable;
mod queries;
mod relations;
mod rollup;
mod store;
#[cfg(test)]
mod tests;
mod validation;
mod workflow;
pub use error::*;
pub use model::*;

use crate::KanbanService;
use kanban_core::Clock;
use std::collections::BTreeMap;
use store::*;
use turso::{Connection, transaction::TransactionBehavior};

impl<C: Clock> KanbanService<C> {
    pub async fn object_execute(&self, request: ObjectCommand) -> ObjectResult<ObjectReceipt> {
        validation::label(&request.board_id, "board_id", 128)?;
        validation::label(&request.request_id, "request_id", 128)?;
        validation::label(&request.actor, "actor", 200)?;
        if serde_json::to_vec(&request)?.len() > 1_114_112 {
            return Err(ObjectError::invalid("命令超过 1088 KiB"));
        }
        let hash = queries::digest(&request)?;
        let _gate = self.mutation_gate.lock().await;
        let mut connection = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        match execute_in_transaction(&tx, &request, &hash, self.clock.now_ms()).await {
            Ok(receipt) => {
                tx.commit().await.map_err(|e| ObjectError {
                    code: ObjectErrorCode::CommitUnknown,
                    message: format!(
                        "提交结果不确定，请使用原 request_id {} 和原完整命令重试: {e}",
                        request.request_id
                    ),
                })?;
                Ok(receipt)
            }
            Err(error) => {
                if let Err(rollback) = tx.rollback().await {
                    return Err(ObjectError::storage(format!(
                        "{error}; rollback: {rollback}"
                    )));
                }
                Err(error)
            }
        }
    }
    pub async fn object_get(&self, board_id: &str, id: &str) -> ObjectResult<ObjectRecord> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = store::get(&tx, board_id, id).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_catalog(&self) -> ObjectResult<ObjectCatalog> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = catalog::read(&tx).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_list(&self, query: ObjectQuery) -> ObjectResult<ObjectPage> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = queries::list(&tx, &query).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_overview(
        &self,
        board_id: &str,
        id: &str,
    ) -> ObjectResult<PlanningOverview> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = rollup::overview(&tx, board_id, id).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_workflow_closure(
        &self,
        board_id: &str,
        id: &str,
    ) -> ObjectResult<Option<WorkflowClosure>> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let object = store::get(&tx, board_id, id).await?;
        if workflow::for_type(&tx, &object.type_key).await?.is_none() {
            return Err(ObjectError::invalid("目标没有生命周期定义"));
        }
        let result = workflow::closure(&tx, board_id, id).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_references(&self, query: ReferenceQuery) -> ObjectResult<ReferencePage> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = queries::references(&tx, &query).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_snapshots(
        &self,
        board_id: &str,
        id: &str,
        before: Option<String>,
        limit: u32,
    ) -> ObjectResult<Vec<SnapshotRecord>> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = queries::snapshots(&tx, board_id, id, before, limit).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_history(
        &self,
        board_id: &str,
        id: &str,
        after: i64,
        limit: u32,
    ) -> ObjectResult<Vec<ObjectAuditEntry>> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = queries::history(&tx, board_id, id, after, limit).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn object_diagnostics(&self, board_id: &str) -> ObjectResult<Vec<ObjectDiagnostic>> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        board(&tx, board_id, false).await?;
        let result = diagnostics::board(&tx, board_id).await?;
        tx.commit().await?;
        Ok(result)
    }
}
async fn execute_in_transaction(
    c: &Connection,
    request: &ObjectCommand,
    hash: &str,
    now: i64,
) -> ObjectResult<ObjectReceipt> {
    let old = rows(
        c,
        "SELECT request_hash,receipt_json FROM object_requests WHERE board_id=?1 AND request_id=?2",
        vec![s(&request.board_id), s(&request.request_id)],
    )
    .await?;
    if let Some(row) = old.first() {
        if text(&row[0])? != hash {
            return Err(ObjectError::conflict(
                "request_id 已被不同命令或 actor 使用",
            ));
        }
        let mut receipt: ObjectReceipt = serde_json::from_str(&text(&row[1])?)?;
        receipt.replayed = true;
        return Ok(receipt);
    }
    board(c, &request.board_id, true).await?;
    let mut ids = commands::apply(c, request, now).await?;
    ids.sort();
    ids.dedup();
    let mut versions = BTreeMap::new();
    let full_revision = matches!(
        &request.mutation,
        ObjectMutation::Create { .. }
            | ObjectMutation::Patch { .. }
            | ObjectMutation::SetBody { .. }
            | ObjectMutation::SetArchived { .. }
    );
    for id in &ids {
        let object = get(c, &request.board_id, id).await?;
        validation::object(c, &object).await?;
        versions.insert(id.clone(), object.version.clone());
        // 任务成员的过程由事件和关闭快照记录；结转不复制所有任务正文与自定义字段。
        if full_revision || object.version.source.is_none() {
            snapshot(
                c,
                &request.board_id,
                id,
                "revision",
                &serde_json::json!({"request_id":request.request_id,"object":object}),
                now,
            )
            .await?;
        }
    }
    let catalog_version = catalog_version(c).await?;
    let payload = serde_json::json!({"format":"kanban.object-event.v2","request_id":request.request_id,"object_ids":ids,"versions":versions,"catalog_version":catalog_version,"scope":if matches!(&request.mutation,ObjectMutation::DefineType{..}|ObjectMutation::DefineProperty{..}|ObjectMutation::BindProperty{..}|ObjectMutation::UnbindProperty{..}|ObjectMutation::RenameType{..}|ObjectMutation::RetireType{..}|ObjectMutation::DefineRelation{..}|ObjectMutation::DefineWorkflow{..}|ObjectMutation::DefineRollup{..}){"catalog"}else{"board"},"invalidate_board":true,"mutation":request.mutation});
    let event_id = format!("e_{}", ulid::Ulid::new());
    let operation = serde_json::to_value(&request.mutation)?["operation"]
        .as_str()
        .ok_or_else(|| ObjectError::storage("命令缺少 operation"))?
        .to_owned();
    exec(c,"INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (?1,?2,NULL,NULL,?3,?4,?5,?6)",vec![s(&event_id),s(&request.board_id),s(&format!("object.{operation}")),s(&request.actor),s(&serde_json::to_string(&payload)?),n(now)]).await?;
    let sequence = count(
        c,
        "SELECT id FROM task_events WHERE event_id=?1",
        vec![s(&event_id)],
    )
    .await?;
    for id in &ids {
        exec(
            c,
            "INSERT INTO object_event_links(board_id,object_id,event_sequence) VALUES (?1,?2,?3)",
            vec![s(&request.board_id), s(id), n(sequence)],
        )
        .await?;
    }
    let receipt = ObjectReceipt {
        request_id: request.request_id.clone(),
        ids,
        event_sequence: sequence,
        catalog_version,
        replayed: false,
        versions,
    };
    exec(c,"INSERT INTO object_requests(board_id,request_id,request_hash,receipt_json,event_sequence,created_at) VALUES (?1,?2,?3,?4,?5,?6)",vec![s(&request.board_id),s(&request.request_id),s(hash),s(&serde_json::to_string(&receipt)?),n(sequence),n(now)]).await?;
    Ok(receipt)
}
