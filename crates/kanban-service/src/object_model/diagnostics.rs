//! 目录、边、双向基数、生命周期和回执语义校验。启动和 portable 导入复用这些规则。
use super::{catalog, error::*, model::*, relations, store::*, validation, workflow};
use turso::Connection;

pub(super) async fn board(c: &Connection, board: &str) -> ObjectResult<Vec<ObjectDiagnostic>> {
    let mut issues = Vec::new();
    for row in rows(c,"SELECT t.id FROM tasks t LEFT JOIN objects o ON o.task_id=t.id WHERE t.board_id=?1 AND o.id IS NULL ORDER BY t.id",vec![s(board)]).await? {
        issues.push(ObjectDiagnostic{code:"missing_task_identity".into(),object_id:Some(text(&row[0])?),detail:"任务缺少通用对象身份".into()});
    }
    for row in rows(
        c,
        "SELECT id FROM objects WHERE board_id=?1 ORDER BY id",
        vec![s(board)],
    )
    .await?
    {
        let id = text(&row[0])?;
        let result = async {
            let object = get(c, board, &id).await?;
            validation::object(c, &object).await?;
            if let Some(d) = workflow::for_type(c, &object.type_key).await? {
                let closed = one_date(&object, &d.closed_property).is_some();
                let closure = workflow::closure(c, board, &id).await?;
                if closed != closure.is_some() {
                    return Err(ObjectError::precondition("生命周期关闭状态和快照不一致"));
                }
                if let Some(closure) = closure {
                    if closure.object.id != id
                        || closure.object.board_id != board
                        || closure.object.type_key != object.type_key
                        || closure.captured_at
                            != one_date(&object, &d.closed_property).unwrap_or(i64::MIN)
                        || closure.object.title != object.title
                        || closure.object.body != object.body
                        || !workflow::frozen_properties_match(c, &closure.object, &object).await?
                    {
                        return Err(ObjectError::precondition("冻结快照与对象不一致"));
                    }
                    if closure.members.len() > relations::MAX_MEMBERS as usize {
                        return Err(ObjectError::precondition("关闭快照成员超限"));
                    }
                    let (r, _) = relations::field(c, &object.type_key, &d.members_property).await?;
                    let mut seen = std::collections::BTreeSet::new();
                    for member in closure.members {
                        if !seen.insert(member.id.clone()) {
                            return Err(ObjectError::precondition("关闭快照成员重复"));
                        }
                        let source = get(c, board, &member.id).await?;
                        if r.target_type.as_deref() != Some(source.type_key.as_str())
                            || member.version.object < 1
                            || member.version.source.is_some_and(|v| v < 0)
                        {
                            return Err(ObjectError::precondition("冻结成员身份或版本无效"));
                        }
                        let p = catalog::property(c, &d.member_status_property).await?;
                        validation::scalar_values(
                            &p.definition,
                            &[PropertyValue::Select(member.status)],
                            true,
                        )?;
                        if let Some(to) = member.carried_to
                            && (to == id || get(c, board, &to).await?.type_key != object.type_key)
                        {
                            return Err(ObjectError::precondition("快照结转目标无效"));
                        }
                    }
                }
            }
            Ok::<(), ObjectError>(())
        }
        .await;
        if let Err(error) = result {
            issues.push(ObjectDiagnostic {
                code: "object_invariant".into(),
                object_id: Some(id),
                detail: error.to_string(),
            });
        }
    }
    Ok(issues)
}
pub(crate) async fn require_valid(c: &Connection) -> ObjectResult<()> {
    catalog::validate_system(c).await?;
    if exists(
        c,
        "SELECT 1 FROM object_property_values WHERE kind='object' LIMIT 1",
        vec![],
    )
    .await?
    {
        return Err(ObjectError::precondition("发现第二份标量引用存储"));
    }
    if exists(
        c,
        "SELECT 1 FROM object_property_slots WHERE kind='object' LIMIT 1",
        vec![],
    )
    .await?
    {
        return Err(ObjectError::precondition("引用字段不应拥有标量槽位"));
    }
    if exists(c,"SELECT 1 FROM object_relation_edges e JOIN object_relation_types r ON r.key=e.relation_key JOIN objects t ON t.id=e.target_id WHERE r.target_type IS NOT NULL AND r.target_type!=t.type_key LIMIT 1",vec![]).await? {return Err(ObjectError::precondition("关系目标类型不一致"));}
    if exists(c,"SELECT 1 FROM object_event_links l JOIN task_events e ON e.id=l.event_sequence WHERE l.board_id!=e.board_id LIMIT 1",vec![]).await?
        || exists(c,"SELECT 1 FROM object_requests r JOIN task_events e ON e.id=r.event_sequence WHERE r.board_id!=e.board_id LIMIT 1",vec![]).await? {return Err(ObjectError::precondition("事件或回执跨越项目"));}
    for row in rows(
        c,
        "SELECT request_id,receipt_json,event_sequence FROM object_requests",
        vec![],
    )
    .await?
    {
        let receipt: ObjectReceipt = serde_json::from_str(&text(&row[1])?)?;
        if receipt.request_id != text(&row[0])?
            || receipt.event_sequence != int(&row[2])?
            || receipt.replayed
            || receipt.catalog_version < 1
        {
            return Err(ObjectError::precondition("命令回执身份不匹配"));
        }
    }
    for b in catalog::read(c).await?.bindings {
        let p = catalog::property(c, &b.binding.property_key).await?;
        validation::scalar_values(&p.definition, &b.binding.defaults, false)?;
        if b.binding
            .defaults
            .iter()
            .any(|v| matches!(v, PropertyValue::Object(_)))
        {
            return Err(ObjectError::precondition("全局目录包含项目引用默认值"));
        }
    }
    for r in rows(c, "SELECT id FROM boards ORDER BY id", vec![]).await? {
        if let Some(issue) = board(c, &text(&r[0])?).await?.first() {
            return Err(ObjectError::precondition(issue.detail.clone()));
        }
    }
    if !rows(c, "PRAGMA foreign_key_check", vec![])
        .await?
        .is_empty()
    {
        return Err(ObjectError::precondition("外键校验失败"));
    }
    Ok(())
}
