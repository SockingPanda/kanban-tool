use super::super::{
    CommandEvent, ObjectError, ObjectErrorCode, ObjectResult, commands, model::*, queries,
    record_command, replay, store::*, validation, workflow,
};
use super::*;
use crate::KanbanService;
use kanban_core::Clock;
use std::collections::BTreeSet;
use turso::{Connection, transaction::TransactionBehavior};

impl<C: Clock> KanbanService<C> {
    /// 回执先于缺省版本解析；任何校验失败回滚字段、关系和事件。
    pub async fn planning_execute(&self, request: PlanningCommand) -> ObjectResult<PlanningResult> {
        validation::label(&request.board_id, "board_id", 128)?;
        validation::label(&request.actor, "actor", 200)?;
        validation::label(&request.request_id, "request_id", 128)?;
        if serde_json::to_vec(&request)?.len() > 1_114_112 {
            return Err(ObjectError::invalid("命令超过 1088 KiB"));
        }
        let hash = queries::digest(&request)?;
        let _gate = self.mutation_gate.lock().await;
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let result = apply(&tx, &request, &hash, self.clock.now_ms()).await;
        match result {
            Ok(result) => {
                tx.commit().await.map_err(|e| ObjectError {
                    code: ObjectErrorCode::CommitUnknown,
                    message: format!(
                        "提交结果不确定，使用原 request_id {} 重试: {e}",
                        request.request_id
                    ),
                })?;
                Ok(result)
            }
            Err(error) => {
                tx.rollback().await?;
                Err(error)
            }
        }
    }
}

async fn apply(
    c: &Connection,
    r: &PlanningCommand,
    hash: &str,
    now: i64,
) -> ObjectResult<PlanningResult> {
    if let Some(receipt) = replay(c, &r.board_id, &r.request_id, hash).await? {
        let created;
        let id = match &r.mutation {
            PlanningMutation::Create { .. } => {
                let event = rows(c, "SELECT json_extract(payload_json,'$.mutation.object_id') FROM task_events WHERE id=?1 AND board_id=?2", vec![n(receipt.event_sequence),s(&r.board_id)]).await?;
                created = text(
                    event
                        .first()
                        .and_then(|row| row.first())
                        .ok_or_else(|| ObjectError::storage("创建回执缺少对象"))?,
                )?;
                &created
            }
            other => identity(other),
        };
        return Ok(PlanningResult {
            object: super::reads::record(c, &r.board_id, r.kind, id, true).await?,
            receipt,
        });
    }
    board(c, &r.board_id, true).await?;
    let catalog = catalog_version(c).await?;
    if let Some(expected) = r.versions.expected_catalog_version {
        expect_catalog(c, expected).await?;
    }
    let original = match &r.mutation {
        PlanningMutation::Create { .. } => {
            if r.versions.expected_version.is_some() {
                return Err(ObjectError::invalid("创建不接受 expected_version"));
            }
            None
        }
        mutation => {
            let id = identity(mutation);
            super::reads::record(c, &r.board_id, r.kind, id, false).await?;
            let object = get(c, &r.board_id, id).await?;
            if r.versions
                .expected_version
                .as_ref()
                .is_some_and(|v| *v != object.version)
            {
                return Err(ObjectError::conflict("对象版本已变化"));
            }
            Some(object)
        }
    };
    // 端点预读发生在任何字段改动之前，避免同一复合命令把自身产生的版本当作竞争。
    for (id, version) in &r.versions.expected_versions {
        target(
            c,
            &r.board_id,
            &ObjectTarget {
                id: id.clone(),
                expected: version.clone(),
            },
            false,
        )
        .await?;
    }
    let mut touched = BTreeSet::new();
    let id = match &r.mutation {
        PlanningMutation::Create {
            title,
            body,
            parent_id,
            starts_at,
            ends_at,
        } => {
            if r.kind == PlanningKind::Module && (starts_at.is_some() || ends_at.is_some())
                || r.kind == PlanningKind::Cycle && parent_id.is_some()
            {
                return Err(ObjectError::invalid("字段不适用于当前对象类型"));
            }
            let mut properties = BTreeMap::new();
            for (key, value) in [("cycle.starts_at", starts_at), ("cycle.ends_at", ends_at)] {
                if let Some(value) = value {
                    properties.insert(key.into(), vec![PropertyValue::Date(*value)]);
                }
            }
            let id = commands::create(
                c,
                &r.board_id,
                &ObjectCreate {
                    type_key: r.kind.key().into(),
                    title: title.clone(),
                    body: body.clone(),
                    properties,
                    expected_catalog_version: Some(catalog),
                },
                now,
            )
            .await?;
            if let Some(parent) = parent_id {
                touched.extend(
                    super::super::task_planning::replace(
                        c,
                        &r.board_id,
                        &id,
                        "module.parent",
                        std::slice::from_ref(parent),
                        now,
                    )
                    .await?,
                );
            }
            id
        }
        PlanningMutation::Update {
            id,
            title,
            body,
            parent_id,
            starts_at,
            ends_at,
        } => {
            let o = original
                .as_ref()
                .ok_or_else(|| ObjectError::storage("缺少对象"))?;
            if o.archived_at.is_some() {
                return Err(ObjectError::precondition("对象已归档"));
            }
            workflow::ensure_editable(c, o).await?;
            if title.is_none()
                && body.is_none()
                && parent_id.is_none()
                && starts_at.is_none()
                && ends_at.is_none()
            {
                return Err(ObjectError::invalid("至少需要一个修改字段"));
            }
            if r.kind == PlanningKind::Module && (starts_at.is_some() || ends_at.is_some())
                || r.kind == PlanningKind::Cycle && parent_id.is_some()
            {
                return Err(ObjectError::invalid("字段不适用于当前对象类型"));
            }
            let mut edits = Vec::new();
            for (key, value) in [("cycle.starts_at", starts_at), ("cycle.ends_at", ends_at)] {
                if let Some(value) = value {
                    edits.push(PropertyEdit::Set {
                        property_key: key.into(),
                        values: vec![PropertyValue::Date(*value)],
                    });
                }
            }
            if title.is_some() || !edits.is_empty() {
                commands::apply(
                    c,
                    &ObjectCommand {
                        board_id: r.board_id.clone(),
                        actor: r.actor.clone(),
                        request_id: r.request_id.clone(),
                        mutation: ObjectMutation::Patch {
                            patch: ObjectPatch {
                                target: ObjectTarget {
                                    id: id.clone(),
                                    expected: o.version.clone(),
                                },
                                title: title.clone(),
                                edits,
                                expected_catalog_version: Some(catalog),
                            },
                        },
                    },
                    now,
                )
                .await?;
            }
            if let Some(body) = body {
                validation::body(body.as_deref())?;
                exec(
                    c,
                    "UPDATE objects SET body=?1 WHERE id=?2 AND board_id=?3",
                    vec![os(body.as_deref()), s(id), s(&r.board_id)],
                )
                .await?;
                let current = get(c, &r.board_id, id).await?;
                validation::object(c, &current).await?;
                bump(c, &current, now).await?;
            }
            if let Some(parent) = parent_id {
                touched.extend(
                    super::super::task_planning::replace(
                        c,
                        &r.board_id,
                        id,
                        "module.parent",
                        &parent.iter().cloned().collect::<Vec<_>>(),
                        now,
                    )
                    .await?,
                );
            }
            id.clone()
        }
        mutation => {
            let o = original
                .as_ref()
                .ok_or_else(|| ObjectError::storage("缺少对象"))?;
            let root = ObjectTarget {
                id: o.id.clone(),
                expected: o.version.clone(),
            };
            let action = match mutation {
                PlanningMutation::Archive { .. } => ObjectMutation::SetArchived {
                    target: root,
                    archived: true,
                },
                PlanningMutation::Restore { .. } => ObjectMutation::SetArchived {
                    target: root,
                    archived: false,
                },
                PlanningMutation::TaskAdd { task_id, .. }
                | PlanningMutation::TaskRemove { task_id, .. } => {
                    if o.archived_at.is_some() {
                        return Err(ObjectError::precondition("对象已归档"));
                    }
                    workflow::ensure_editable(c, o).await?;
                    let task = get(c, &r.board_id, task_id).await?;
                    if task.type_key != "task" {
                        return Err(ObjectError::invalid("成员必须是任务"));
                    }
                    let link = RelationLink {
                        relation_key: r.kind.members().into(),
                        source_id: o.id.clone(),
                        target_id: task_id.clone(),
                    };
                    let adding = matches!(mutation, PlanningMutation::TaskAdd { .. });
                    ObjectMutation::ChangeRelations {
                        change: RelationChange {
                            expected_catalog_version: catalog,
                            expected: vec![
                                root,
                                ObjectTarget {
                                    id: task_id.clone(),
                                    expected: task.version,
                                },
                            ],
                            remove: if adding { vec![] } else { vec![link.clone()] },
                            add: if adding { vec![link] } else { vec![] },
                        },
                    }
                }
                PlanningMutation::Start { .. }
                | PlanningMutation::Close { .. }
                | PlanningMutation::Cancel { .. } => {
                    if r.kind != PlanningKind::Cycle {
                        return Err(ObjectError::invalid("模块没有迭代生命周期"));
                    }
                    match mutation {
                        PlanningMutation::Start { .. } => {
                            ObjectMutation::StartWorkflow { target: root }
                        }
                        PlanningMutation::Cancel { .. } => {
                            ObjectMutation::CancelWorkflow { target: root }
                        }
                        PlanningMutation::Close { carry_to, .. } => {
                            let carry_to = match carry_to {
                                Some(id) => {
                                    super::reads::record(
                                        c,
                                        &r.board_id,
                                        PlanningKind::Cycle,
                                        id,
                                        false,
                                    )
                                    .await?;
                                    Some(ObjectTarget {
                                        id: id.clone(),
                                        expected: get(c, &r.board_id, id).await?.version,
                                    })
                                }
                                None => None,
                            };
                            ObjectMutation::CloseWorkflow {
                                target: root,
                                carry_to,
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            };
            touched.extend(
                commands::apply(
                    c,
                    &ObjectCommand {
                        board_id: r.board_id.clone(),
                        actor: r.actor.clone(),
                        request_id: r.request_id.clone(),
                        mutation: action,
                    },
                    now,
                )
                .await?,
            );
            o.id.clone()
        }
    };
    touched.insert(id.clone());
    let mut mutation = serde_json::to_value(&r.mutation)?;
    mutation["operation"] = serde_json::json!(format!(
        "planning.{}.{}",
        r.kind.key(),
        mutation["operation"].as_str().unwrap_or("write")
    ));
    // 回执保存主对象标识，创建有父模块时也能稳定定位创建结果。
    mutation["object_id"] = serde_json::json!(&id);
    let mut ids: Vec<_> = touched.into_iter().collect();
    ids.retain(|other| other != &id);
    ids.insert(0, id.clone());
    let receipt = record_command(
        c,
        CommandEvent {
            board_id: &r.board_id,
            request_id: &r.request_id,
            actor: &r.actor,
            mutation,
        },
        hash,
        ids,
        false,
        now,
    )
    .await?;
    Ok(PlanningResult {
        object: super::reads::record(c, &r.board_id, r.kind, &id, true).await?,
        receipt,
    })
}

fn identity(m: &PlanningMutation) -> &String {
    match m {
        PlanningMutation::Update { id, .. }
        | PlanningMutation::Archive { id }
        | PlanningMutation::Restore { id }
        | PlanningMutation::TaskAdd { id, .. }
        | PlanningMutation::TaskRemove { id, .. }
        | PlanningMutation::Start { id }
        | PlanningMutation::Close { id, .. }
        | PlanningMutation::Cancel { id } => id,
        PlanningMutation::Create { .. } => unreachable!(),
    }
}
