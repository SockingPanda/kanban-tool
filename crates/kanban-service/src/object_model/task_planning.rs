//! 任务事务内的归属与回执。关系边仍是唯一事实，不保存归属副本。
use super::{
    CommandEvent, ObjectError, ObjectErrorCode, ObjectResult, model::*, queries, record_command,
    relations, replay, store::*,
};
use crate::error::StoreError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use turso::Connection;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPlanningInput {
    pub module_ids: Option<Vec<String>>,
    pub cycle_id: Option<Option<String>>,
    pub expected_object_version: Option<i64>,
    pub expected_versions: BTreeMap<String, ObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}
impl TaskPlanningInput {
    pub fn requested(&self) -> bool {
        self.module_ids.is_some() || self.cycle_id.is_some()
    }
}

impl From<ObjectError> for StoreError {
    fn from(error: ObjectError) -> Self {
        match error.code {
            ObjectErrorCode::InvalidArgument | ObjectErrorCode::NotFound => {
                Self::InvalidInput(error.message)
            }
            ObjectErrorCode::Conflict => Self::ClaimConflict(error.message),
            ObjectErrorCode::FailedPrecondition => Self::InvalidTransition(error.message),
            ObjectErrorCode::Storage | ObjectErrorCode::CommitUnknown => {
                Self::SchemaMismatch(error.message)
            }
        }
    }
}

pub(crate) fn digest<T: Serialize>(value: &T) -> Result<String, StoreError> {
    queries::digest(value).map_err(Into::into)
}

pub(crate) async fn replay_task(
    c: &Connection,
    board: &str,
    key: &str,
    hash: &str,
) -> Result<Option<String>, StoreError> {
    match replay(c, board, key, hash).await {
        Ok(Some(receipt)) => Ok(Some(receipt_task_id(receipt)?)),
        Ok(None) => Ok(None),
        Err(error) if error.code == ObjectErrorCode::Conflict => {
            let data = rows(
                c,
                "SELECT receipt_json FROM object_requests WHERE board_id=?1 AND request_id=?2",
                vec![s(board), s(key)],
            )
            .await?;
            let row = data
                .first()
                .ok_or_else(|| StoreError::SchemaMismatch("任务回执不存在".into()))?;
            let receipt = serde_json::from_str(&text(&row[0])?).map_err(ObjectError::from)?;
            Err(StoreError::IdempotencyConflict {
                board_id: board.into(),
                key: key.into(),
                existing_task_id: receipt_task_id(receipt)?,
            })
        }
        Err(error) => Err(error.into()),
    }
}

fn receipt_task_id(receipt: ObjectReceipt) -> Result<String, StoreError> {
    receipt
        .ids
        .into_iter()
        .find(|id| id.starts_with("t_"))
        .ok_or_else(|| StoreError::SchemaMismatch("任务回执缺少 ID".into()))
}

pub(crate) struct TaskRequestEvent<'a> {
    pub task_id: &'a str,
    pub event_id: &'a str,
    pub request_id: &'a str,
    pub hash: &'a str,
    pub actor: &'a str,
    pub touched: Vec<String>,
    pub now: i64,
}

pub(crate) async fn record_task(
    c: &Connection,
    board: &str,
    mut event: TaskRequestEvent<'_>,
) -> Result<(), StoreError> {
    if event.touched.is_empty() {
        // 未修改关系时复用任务已有事件，保留旧调用的事件数量与种类。
        let task = get(c, board, event.task_id).await?;
        let sequence = count(
            c,
            "SELECT id FROM task_events WHERE event_id=?1 AND board_id=?2",
            vec![s(event.event_id), s(board)],
        )
        .await?;
        let receipt = ObjectReceipt {
            request_id: event.request_id.into(),
            ids: vec![event.task_id.into()],
            versions: BTreeMap::from([(event.task_id.into(), task.version)]),
            event_sequence: sequence,
            catalog_version: catalog_version(c).await?,
            replayed: false,
        };
        exec(c, "INSERT INTO object_requests(board_id,request_id,request_hash,receipt_json,event_sequence,created_at) VALUES (?1,?2,?3,?4,?5,?6)", vec![s(board),s(event.request_id),s(event.hash),s(&serde_json::to_string(&receipt).map_err(ObjectError::from)?),n(sequence),n(event.now)]).await?;
        return Ok(());
    }
    // 旧、新归属都进入对象事件索引，已有 Web 对象查询可以观察换属。
    event.touched.push(event.task_id.into());
    record_command(
        c,
        CommandEvent {
            board_id: board,
            request_id: event.request_id,
            actor: event.actor,
            mutation: serde_json::json!({"operation":"task_request","task_id":event.task_id}),
        },
        event.hash,
        event.touched,
        false,
        event.now,
    )
    .await?;
    Ok(())
}

/// 在任务 SQL 更新前检查双版本及旧、新端点。缺省版本在当前写事务内解析。
pub(crate) async fn apply(
    c: &Connection,
    board: &str,
    task_id: &str,
    input: &TaskPlanningInput,
    now: i64,
) -> Result<Vec<String>, StoreError> {
    let task = get(c, board, task_id).await?;
    if input
        .expected_object_version
        .is_some_and(|v| v != task.version.object)
    {
        return Err(ObjectError::conflict("任务对象版本已变化").into());
    }
    if let Some(v) = input.expected_catalog_version {
        expect_catalog(c, v).await?;
    }
    for (id, expected) in &input.expected_versions {
        target(
            c,
            board,
            &ObjectTarget {
                id: id.clone(),
                expected: expected.clone(),
            },
            false,
        )
        .await?;
    }
    let mut touched = BTreeSet::new();
    if let Some(modules) = &input.module_ids {
        touched.extend(replace(c, board, task_id, "task.modules", modules, now).await?);
    }
    if let Some(cycle) = &input.cycle_id {
        touched.extend(
            replace(
                c,
                board,
                task_id,
                "task.cycle",
                &cycle.iter().cloned().collect::<Vec<_>>(),
                now,
            )
            .await?,
        );
    }
    Ok(touched.into_iter().collect())
}

pub(super) async fn replace(
    c: &Connection,
    board: &str,
    id: &str,
    property: &str,
    new: &[String],
    now: i64,
) -> ObjectResult<Vec<String>> {
    let object = get(c, board, id).await?;
    let old = relations::ids(c, board, id, &object.type_key, property)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let values = new.iter().cloned().collect::<BTreeSet<_>>();
    if values.len() != new.len() || values.len() > 100 {
        return Err(ObjectError::invalid("归属不能重复且最多 100 项"));
    }
    for peer in &values {
        if !peer.starts_with("obj_") {
            return Err(ObjectError::invalid("归属必须使用 obj_... ID"));
        }
        let target = get(c, board, peer).await?;
        if target.archived_at.is_some() {
            return Err(ObjectError::precondition("归属对象已归档"));
        }
        super::workflow::ensure_editable(c, &target).await?;
    }
    let mut expected = Vec::new();
    for peer in old.symmetric_difference(&values) {
        expected.push(ObjectTarget {
            id: peer.clone(),
            expected: get(c, board, peer).await?.version,
        });
    }
    relations::set_references(
        c,
        board,
        &ObjectTarget {
            id: id.into(),
            expected: object.version,
        },
        property,
        new,
        &expected,
        catalog_version(c).await?,
        now,
    )
    .await
}

/// 按页批量补齐关系；调用者需以读事务固定任务字段、总数与归属。
pub(crate) async fn hydrate(
    c: &Connection,
    tasks: &mut [crate::domain::TaskRecord],
) -> Result<(), StoreError> {
    for page in tasks.chunks_mut(1_000) {
        hydrate_page(c, page).await?;
    }
    Ok(())
}

async fn hydrate_page(
    c: &Connection,
    tasks: &mut [crate::domain::TaskRecord],
) -> Result<(), StoreError> {
    for task in tasks.iter_mut() {
        task.module_ids.clear();
        task.cycle_id = None;
    }
    let placeholders = (1..=tasks.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let params = tasks.iter().map(|t| s(&t.id)).collect::<Vec<_>>();
    let data = rows(c, &format!("SELECT o.id,o.version,e.relation_key,e.source_id FROM objects o LEFT JOIN object_relation_edges e ON e.target_id=o.id AND e.board_id=o.board_id AND e.relation_key IN ('module_members','cycle_members') WHERE o.id IN ({placeholders}) ORDER BY o.id,e.source_id"), params).await?;
    let indices: BTreeMap<_, _> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.clone(), i))
        .collect();
    for row in data {
        let id = text(&row[0])?;
        let task = &mut tasks[*indices
            .get(&id)
            .ok_or_else(|| StoreError::SchemaMismatch("归属读取不在任务页内".into()))?];
        task.object_version = int(&row[1])?;
        match opt_text(&row[2])?.as_deref() {
            Some("module_members") => task.module_ids.push(text(&row[3])?),
            Some("cycle_members") => task.cycle_id = opt_text(&row[3])?,
            _ => (),
        }
    }
    Ok(())
}
