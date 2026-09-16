//! 查询只接受白名单操作符。用户提供的 key 和 value 不拼接为 SQL 标识符。
use super::{catalog, error::*, model::*, relations, store::*, validation};
use sha2::{Digest, Sha256};
use turso::{Connection, Value};

pub(super) fn digest(value: &impl serde::Serialize) -> ObjectResult<String> {
    // 显式规范化 JSON 对象 key；不依赖 workspace 的 serde_json preserve_order feature。
    fn normalize(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Array(v) => {
                serde_json::Value::Array(v.into_iter().map(normalize).collect())
            }
            serde_json::Value::Object(m) => {
                let sorted = m.into_iter().collect::<std::collections::BTreeMap<_, _>>();
                serde_json::Value::Object(
                    sorted.into_iter().map(|(k, v)| (k, normalize(v))).collect(),
                )
            }
            other => other,
        }
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&normalize(serde_json::to_value(
            value
        )?))?)
    ))
}
fn param(args: &mut Vec<Value>, value: Value) -> String {
    args.push(value);
    format!("?{}", args.len())
}
fn native_value_column(value: &PropertyValue) -> ObjectResult<(&'static str, Value)> {
    Ok(match value {
        PropertyValue::Text(v) => ("text_value", s(v)),
        PropertyValue::Number(v) => ("number_value", Value::Real(*v)),
        PropertyValue::Boolean(v) => ("integer_value", n(i64::from(*v))),
        PropertyValue::Date(v) => ("integer_value", n(*v)),
        PropertyValue::Object(_) => return Err(ObjectError::invalid("引用查询必须通过关系边")),
        PropertyValue::Select(v) => ("option_key", s(v)),
    })
}
fn source_column(key: &str) -> Option<&'static str> {
    match key {
        "task.status" => Some("t.status"),
        "task.priority" => Some("t.priority"),
        "task.assignee" => Some("t.assignee"),
        "task.due_at" => Some("t.due_at"),
        _ => None,
    }
}
pub(super) async fn list(c: &Connection, query: &ObjectQuery) -> ObjectResult<ObjectPage> {
    board(c, &query.board_id, false).await?;
    if !(1..=200).contains(&query.limit) || query.filters.len() > 8 {
        return Err(ObjectError::invalid(
            "limit 应为 1 到 200，filters 最多 8 项",
        ));
    }
    if query.text.as_ref().is_some_and(|s| s.len() > 1024) {
        return Err(ObjectError::invalid("text 查询超过 1024 字节"));
    }
    let mut signature = query.clone();
    signature.after = None;
    let hash = digest(&signature)?;
    if query.after.as_ref().is_some_and(|c| c.query_hash != hash) {
        return Err(ObjectError::invalid("游标不属于当前查询条件"));
    }
    let mut args = vec![s(&query.board_id)];
    let mut clauses = vec!["o.board_id=?1".to_owned()];
    if !query.include_archived {
        clauses.push(
            "CASE WHEN o.task_id IS NULL THEN o.archived_at ELSE t.archived_at END IS NULL".into(),
        );
    }
    if let Some(ty) = &query.type_key {
        catalog::type_record(c, ty).await?;
        let p = param(&mut args, s(ty));
        clauses.push(format!("o.type_key={p}"));
    }
    if let Some(text) = &query.text {
        let p = param(&mut args, s(text));
        clauses.push(format!("instr(COALESCE(t.title,o.title),{p})>0"));
    }
    if let Some(cursor) = &query.after {
        let p = param(&mut args, s(&cursor.after_id));
        clauses.push(format!("o.id>{p}"));
    }
    for filter in &query.filters {
        let key = match filter {
            PropertyFilter::Has { property_key }
            | PropertyFilter::IsEmpty { property_key }
            | PropertyFilter::IsMissing { property_key }
            | PropertyFilter::Equals { property_key, .. }
            | PropertyFilter::Contains { property_key, .. } => property_key,
        };
        let property = catalog::property(c, key).await?;
        if let Some(ty) = &query.type_key {
            catalog::binding(c, ty, key).await?;
        }
        let value = match filter {
            PropertyFilter::Equals { value, .. } => {
                if property.definition.cardinality != Cardinality::One {
                    return Err(ObjectError::invalid("多值属性请使用 contains"));
                }
                Some(value)
            }
            PropertyFilter::Contains { value, .. } => {
                if property.definition.cardinality != Cardinality::Many {
                    return Err(ObjectError::invalid("单值属性请使用 equals"));
                }
                Some(value)
            }
            _ => None,
        };
        if let Some(value) = value {
            validation::scalar_values(&property.definition, std::slice::from_ref(value), false)?;
        }
        if let Some(column) = source_column(key) {
            clauses.push("o.type_key='task'".into());
            clauses.push(match filter {
                PropertyFilter::Has { .. } => "1=1".to_owned(),
                PropertyFilter::IsMissing { .. } => "1=0".to_owned(),
                PropertyFilter::IsEmpty { .. } => format!("{column} IS NULL"),
                _ => {
                    let (_, v) = native_value_column(
                        value.ok_or_else(|| ObjectError::invalid("查询缺少值"))?,
                    )?;
                    let p = param(&mut args, v);
                    format!("{column}={p}")
                }
            });
            continue;
        }
        if property.definition.kind == PropertyKind::Object {
            let p = param(&mut args, s(key));
            let bound = format!(
                "EXISTS (SELECT 1 FROM object_type_properties b WHERE b.type_key=o.type_key AND b.property_key={p})"
            );
            let mut matching = format!(
                "e.board_id=o.board_id AND ((r.source_type=o.type_key AND r.source_property={p} AND e.source_id=o.id) OR (r.target_type=o.type_key AND r.target_property={p} AND e.target_id=o.id))"
            );
            if let Some(PropertyValue::Object(id)) = value {
                let other = param(&mut args, s(id));
                matching.push_str(&format!(" AND CASE WHEN r.source_property={p} THEN e.target_id ELSE e.source_id END={other}"));
            }
            let found = format!(
                "EXISTS (SELECT 1 FROM object_relation_edges e JOIN object_relation_types r ON r.key=e.relation_key WHERE {matching})"
            );
            clauses.push(match filter {
                PropertyFilter::Has { .. } => bound,
                PropertyFilter::IsMissing { .. } => format!("NOT {bound}"),
                PropertyFilter::IsEmpty { .. } => format!("{bound} AND NOT {found}"),
                _ => found,
            });
            continue;
        }
        let p = param(&mut args, s(key));
        match filter {
            PropertyFilter::Has{..}=>clauses.push(format!("EXISTS (SELECT 1 FROM object_property_slots slot WHERE slot.object_id=o.id AND slot.property_key={p})")),
            PropertyFilter::IsMissing{..}=>clauses.push(format!("NOT EXISTS (SELECT 1 FROM object_property_slots slot WHERE slot.object_id=o.id AND slot.property_key={p})")),
            PropertyFilter::IsEmpty{..}=>clauses.push(format!("EXISTS (SELECT 1 FROM object_property_slots slot WHERE slot.object_id=o.id AND slot.property_key={p}) AND NOT EXISTS (SELECT 1 FROM object_property_values v WHERE v.object_id=o.id AND v.property_key={p})")),
            _=>{
                let (column,v)=native_value_column(value.ok_or_else(||ObjectError::invalid("查询缺少值"))?)?;
                let x=param(&mut args,v);
                clauses.push(format!("EXISTS (SELECT 1 FROM object_property_values v WHERE v.object_id=o.id AND v.property_key={p} AND v.{column}={x})"));
            }
        }
    }
    let limit = param(&mut args, n(i64::from(query.limit) + 1));
    let sql = format!(
        "SELECT o.id FROM objects o LEFT JOIN tasks t ON t.id=o.task_id AND t.board_id=o.board_id WHERE {} ORDER BY o.id LIMIT {limit}",
        clauses.join(" AND ")
    );
    let ids = rows(c, &sql, args).await?;
    let more = ids.len() > query.limit as usize;
    let mut items = Vec::new();
    for row in ids.into_iter().take(query.limit as usize) {
        items.push(get(c, &query.board_id, &text(&row[0])?).await?);
    }
    let next = if more {
        items.last().map(|o| ObjectCursor {
            query_hash: hash,
            after_id: o.id.clone(),
        })
    } else {
        None
    };
    Ok(ObjectPage { items, next })
}
pub(super) async fn references(c: &Connection, q: &ReferenceQuery) -> ObjectResult<ReferencePage> {
    if !(1..=200).contains(&q.limit) {
        return Err(ObjectError::invalid("引用分页 limit 应为 1 到 200"));
    }
    expect_catalog(c, q.expected_catalog_version).await?;
    let object = target(
        c,
        &q.board_id,
        &ObjectTarget {
            id: q.object_id.clone(),
            expected: q.expected_version.clone(),
        },
        false,
    )
    .await?;
    let ids = relations::ids(
        c,
        &q.board_id,
        &q.object_id,
        &object.type_key,
        &q.property_key,
    )
    .await?;
    let mut rest = ids
        .into_iter()
        .filter(|id| q.after_id.as_ref().is_none_or(|after| id > after))
        .take(q.limit as usize + 1)
        .collect::<Vec<_>>();
    let more = rest.len() > q.limit as usize;
    rest.truncate(q.limit as usize);
    let next_id = if more { rest.last().cloned() } else { None };
    let mut items = Vec::new();
    for id in rest {
        items.push(get(c, &q.board_id, &id).await?);
    }
    Ok(ReferencePage { items, next_id })
}
pub(super) async fn snapshots(
    c: &Connection,
    board: &str,
    id: &str,
    before: Option<String>,
    limit: u32,
) -> ObjectResult<Vec<SnapshotRecord>> {
    if !(1..=200).contains(&limit) {
        return Err(ObjectError::invalid("limit 应为 1 到 200"));
    }
    get(c, board, id).await?;
    let mut out = Vec::new();
    for r in rows(c,"SELECT id,object_id,kind,body_json,created_at FROM object_snapshots WHERE board_id=?1 AND object_id=?2 AND (?3 IS NULL OR id<?3) ORDER BY id DESC LIMIT ?4",vec![s(board),s(id),os(before.as_deref()),n(i64::from(limit))]).await? {
        out.push(SnapshotRecord{id:text(&r[0])?,object_id:text(&r[1])?,kind:text(&r[2])?,body:serde_json::from_str(&text(&r[3])?)?,created_at:int(&r[4])?});
    }
    Ok(out)
}
pub(super) async fn history(
    c: &Connection,
    board: &str,
    id: &str,
    after: i64,
    limit: u32,
) -> ObjectResult<Vec<ObjectAuditEntry>> {
    if !(1..=200).contains(&limit) || after < 0 {
        return Err(ObjectError::invalid("无效历史分页"));
    }
    get(c, board, id).await?;
    let mut out = Vec::new();
    for r in rows(c,"SELECT e.id,e.kind,e.actor,e.payload_json,e.created_at FROM task_events e WHERE e.board_id=?1 AND e.id>?2 AND (e.task_id=?3 OR EXISTS (SELECT 1 FROM object_event_links l WHERE l.object_id=?3 AND l.event_sequence=e.id)) ORDER BY e.id LIMIT ?4",vec![s(board),n(after),s(id),n(i64::from(limit))]).await? {
        out.push(ObjectAuditEntry{sequence:int(&r[0])?,kind:text(&r[1])?,actor:opt_text(&r[2])?,payload:serde_json::from_str(&text(&r[3])?)?,created_at:int(&r[4])?});
    }
    Ok(out)
}
