//! SQL 只出现在规范 service 内。表名和列名来自内部常量，用户值只作为参数绑定。
use super::{error::*, model::*};
use std::collections::BTreeMap;
use turso::{Connection, Value};

pub(super) fn s(v: &str) -> Value {
    Value::Text(v.to_owned())
}
pub(super) fn n(v: i64) -> Value {
    Value::Integer(v)
}
pub(super) fn os(v: Option<&str>) -> Value {
    v.map(s).unwrap_or(Value::Null)
}
pub(super) fn on(v: Option<i64>) -> Value {
    v.map(n).unwrap_or(Value::Null)
}
pub(super) async fn rows(
    c: &Connection,
    sql: &str,
    args: Vec<Value>,
) -> ObjectResult<Vec<Vec<Value>>> {
    let mut result = c.query(sql, turso::params_from_iter(args)).await?;
    let mut out = Vec::new();
    while let Some(row) = result.next().await? {
        let mut values = Vec::with_capacity(row.column_count());
        for index in 0..row.column_count() {
            values.push(row.get_value(index)?);
        }
        out.push(values);
    }
    Ok(out)
}
pub(super) async fn exec(c: &Connection, sql: &str, args: Vec<Value>) -> ObjectResult<u64> {
    Ok(c.execute(sql, turso::params_from_iter(args)).await?)
}
pub(super) fn text(v: &Value) -> ObjectResult<String> {
    match v {
        Value::Text(v) => Ok(v.clone()),
        _ => Err(ObjectError::storage("存储值不是 text")),
    }
}
pub(super) fn int(v: &Value) -> ObjectResult<i64> {
    match v {
        Value::Integer(v) => Ok(*v),
        _ => Err(ObjectError::storage("存储值不是 integer")),
    }
}
pub(super) fn opt_text(v: &Value) -> ObjectResult<Option<String>> {
    if matches!(v, Value::Null) {
        Ok(None)
    } else {
        text(v).map(Some)
    }
}
pub(super) fn opt_int(v: &Value) -> ObjectResult<Option<i64>> {
    if matches!(v, Value::Null) {
        Ok(None)
    } else {
        int(v).map(Some)
    }
}
pub(super) async fn exists(c: &Connection, sql: &str, args: Vec<Value>) -> ObjectResult<bool> {
    Ok(!rows(c, sql, args).await?.is_empty())
}
pub(super) async fn count(c: &Connection, sql: &str, args: Vec<Value>) -> ObjectResult<i64> {
    let r = rows(c, sql, args).await?;
    int(r
        .first()
        .and_then(|r| r.first())
        .ok_or_else(|| ObjectError::storage("计数查询没有返回值"))?)
}
pub(super) async fn board(c: &Connection, id: &str, write: bool) -> ObjectResult<()> {
    let rows = rows(c, "SELECT archived_at FROM boards WHERE id=?1", vec![s(id)]).await?;
    let row = rows
        .first()
        .ok_or_else(|| ObjectError::missing("项目不存在"))?;
    if write && !matches!(row[0], Value::Null) {
        return Err(ObjectError::precondition("项目已归档"));
    }
    Ok(())
}
pub(super) async fn catalog_version(c: &Connection) -> ObjectResult<i64> {
    count(
        c,
        "SELECT catalog_version FROM object_model_schema WHERE singleton=1",
        vec![],
    )
    .await
}
pub(super) async fn expect_catalog(c: &Connection, expected: i64) -> ObjectResult<()> {
    if catalog_version(c).await? != expected {
        return Err(ObjectError::conflict("类型目录版本已变化，请重新读取目录"));
    }
    Ok(())
}
pub(super) async fn get(c: &Connection, board_id: &str, id: &str) -> ObjectResult<ObjectRecord> {
    let r = rows(c, "SELECT o.id,o.board_id,o.type_key,COALESCE(t.title,o.title),CASE WHEN o.task_id IS NULL THEN o.body ELSE t.description END,o.version,t.lock_version,COALESCE(t.created_at,o.created_at),MAX(COALESCE(t.updated_at,o.updated_at),o.updated_at),CASE WHEN o.task_id IS NULL THEN o.archived_at ELSE t.archived_at END,t.status,t.priority,t.assignee,t.due_at FROM objects o LEFT JOIN tasks t ON t.id=o.task_id AND t.board_id=o.board_id WHERE o.board_id=?1 AND o.id=?2", vec![s(board_id),s(id)]).await?;
    let row = r
        .first()
        .ok_or_else(|| ObjectError::missing("对象不在当前项目中"))?;
    let mut properties = BTreeMap::new();
    for slot in rows(
        c,
        "SELECT property_key FROM object_property_slots WHERE object_id=?1 ORDER BY property_key",
        vec![s(id)],
    )
    .await?
    {
        properties.insert(text(&slot[0])?, Vec::new());
    }
    for v in rows(c, "SELECT property_key,kind,text_value,number_value,integer_value,option_key FROM object_property_values WHERE object_id=?1 ORDER BY property_key,ordinal", vec![s(id)]).await? {
        let value = match text(&v[1])?.as_str() {
            "text" => PropertyValue::Text(text(&v[2])?),
            "number" => PropertyValue::Number(match &v[3] { Value::Real(x) => *x, Value::Integer(x) => *x as f64, _ => return Err(ObjectError::storage("无效 number 属性")) }),
            "boolean" => PropertyValue::Boolean(int(&v[4])? != 0),
            "date" => PropertyValue::Date(int(&v[4])?),
            "select" => PropertyValue::Select(text(&v[5])?),
            _ => return Err(ObjectError::storage("未知属性类型")),
        };
        properties.entry(text(&v[0])?).or_insert_with(Vec::new).push(value);
    }
    properties.extend(super::relations::read_fields(c, board_id, id, &text(&row[2])?).await?);
    if text(&row[2])? == "task" {
        properties.insert(
            "task.status".into(),
            vec![PropertyValue::Select(text(&row[10])?)],
        );
        properties.insert(
            "task.priority".into(),
            vec![PropertyValue::Number(int(&row[11])? as f64)],
        );
        properties.insert(
            "task.assignee".into(),
            opt_text(&row[12])?
                .map(PropertyValue::Text)
                .into_iter()
                .collect(),
        );
        properties.insert(
            "task.due_at".into(),
            opt_int(&row[13])?
                .map(PropertyValue::Date)
                .into_iter()
                .collect(),
        );
    }
    Ok(ObjectRecord {
        id: text(&row[0])?,
        board_id: text(&row[1])?,
        type_key: text(&row[2])?,
        title: text(&row[3])?,
        body: opt_text(&row[4])?,
        version: ObjectVersion {
            object: int(&row[5])?,
            source: opt_int(&row[6])?,
        },
        created_at: int(&row[7])?,
        updated_at: int(&row[8])?,
        archived_at: opt_int(&row[9])?,
        properties,
    })
}
pub(super) async fn target(
    c: &Connection,
    board_id: &str,
    target: &ObjectTarget,
    writable: bool,
) -> ObjectResult<ObjectRecord> {
    let object = get(c, board_id, &target.id).await?;
    if object.version != target.expected {
        return Err(ObjectError::conflict(format!(
            "对象 {} 的版本已变化",
            target.id
        )));
    }
    if writable && object.archived_at.is_some() {
        return Err(ObjectError::precondition("对象已归档"));
    }
    Ok(object)
}
pub(super) async fn bump(c: &Connection, object: &ObjectRecord, now: i64) -> ObjectResult<()> {
    let next = object
        .version
        .object
        .checked_add(1)
        .ok_or_else(|| ObjectError::precondition("对象版本已耗尽"))?;
    let changed = exec(
        c,
        "UPDATE objects SET version=?1,updated_at=?2 WHERE id=?3 AND board_id=?4 AND version=?5",
        vec![
            n(next),
            n(now),
            s(&object.id),
            s(&object.board_id),
            n(object.version.object),
        ],
    )
    .await?;
    if changed != 1 {
        return Err(ObjectError::conflict("对象版本竞争"));
    }
    Ok(())
}
/// 内部已完成验证的原子槽位替换。空列表保留槽位；Unset 另行删除槽位。
pub(super) async fn set_values(
    c: &Connection,
    object: &ObjectRecord,
    property: &PropertyDefinition,
    values: &[PropertyValue],
) -> ObjectResult<()> {
    if property.kind == PropertyKind::Object {
        return Err(ObjectError::invalid(
            "引用必须写入通用关系边，不能写入标量值表",
        ));
    }
    exec(c, "INSERT INTO object_property_slots(object_id,board_id,type_key,property_key,kind,cardinality,storage) VALUES (?1,?2,?3,?4,?5,?6,'value') ON CONFLICT(object_id,property_key) DO NOTHING",
        vec![s(&object.id),s(&object.board_id),s(&object.type_key),s(&property.key),s(property.kind.key()),s(property.cardinality.key())]).await?;
    exec(
        c,
        "DELETE FROM object_property_values WHERE object_id=?1 AND property_key=?2",
        vec![s(&object.id), s(&property.key)],
    )
    .await?;
    for (ordinal, value) in values.iter().enumerate() {
        let mut typed = vec![Value::Null; 4];
        match value {
            PropertyValue::Text(v) => typed[0] = s(v),
            PropertyValue::Number(v) => typed[1] = Value::Real(*v),
            PropertyValue::Boolean(v) => typed[2] = n(i64::from(*v)),
            PropertyValue::Date(v) => typed[2] = n(*v),
            PropertyValue::Object(_) => return Err(ObjectError::invalid("引用不能写入标量表")),
            PropertyValue::Select(v) => typed[3] = s(v),
        }
        let mut args = vec![
            s(&object.id),
            s(&object.board_id),
            s(&property.key),
            s(property.kind.key()),
            s(property.cardinality.key()),
            n(ordinal as i64),
        ];
        args.extend(typed);
        exec(c, "INSERT INTO object_property_values(object_id,board_id,property_key,kind,cardinality,ordinal,text_value,number_value,integer_value,option_key) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", args).await?;
    }
    Ok(())
}
pub(super) async fn snapshot(
    c: &Connection,
    board: &str,
    id: &str,
    kind: &str,
    body: &serde_json::Value,
    now: i64,
) -> ObjectResult<()> {
    exec(c,"INSERT INTO object_snapshots(id,board_id,object_id,kind,body_json,created_at) VALUES (?1,?2,?3,?4,?5,?6)",vec![s(&format!("snap_{}",ulid::Ulid::new())),s(board),s(id),s(kind),s(&serde_json::to_string(body)?),n(now)]).await?;
    Ok(())
}
pub(super) fn one_date(object: &ObjectRecord, key: &str) -> Option<i64> {
    match object.properties.get(key).and_then(|v| v.first()) {
        Some(PropertyValue::Date(value)) => Some(*value),
        _ => None,
    }
}
pub(super) fn one_select(object: &ObjectRecord, key: &str) -> Option<String> {
    match object.properties.get(key).and_then(|v| v.first()) {
        Some(PropertyValue::Select(value)) => Some(value.clone()),
        _ => None,
    }
}
