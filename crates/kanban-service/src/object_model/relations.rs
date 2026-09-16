//! 关系定义、双端读取及通用事务写入。任何模块、迭代名称都不进入本模块。
use super::{catalog, error::*, model::*, store::*, validation, workflow};
use std::collections::{BTreeMap, BTreeSet};
use turso::Connection;

pub(super) const MAX_MEMBERS: i64 = 5_000;

pub(super) async fn definitions(c: &Connection) -> ObjectResult<Vec<RelationDefinition>> {
    let mut result = Vec::new();
    for r in rows(c, "SELECT key,name,source_type,target_type,source_property,target_property,source_cardinality,target_cardinality,acyclic FROM object_relation_types ORDER BY key", vec![]).await? {
        result.push(RelationDefinition {
            key: text(&r[0])?, name: text(&r[1])?, source_type: text(&r[2])?, target_type: opt_text(&r[3])?,
            source_property: text(&r[4])?, target_property: opt_text(&r[5])?,
            source_cardinality: serde_json::from_value(serde_json::json!(text(&r[6])?))?,
            target_cardinality: serde_json::from_value(serde_json::json!(text(&r[7])?))?, acyclic: int(&r[8])? != 0,
        });
    }
    Ok(result)
}
pub(super) async fn definition(c: &Connection, key: &str) -> ObjectResult<RelationDefinition> {
    definitions(c)
        .await?
        .into_iter()
        .find(|r| r.key == key)
        .ok_or_else(|| ObjectError::missing("关系定义不存在"))
}
pub(super) async fn field(
    c: &Connection,
    ty: &str,
    key: &str,
) -> ObjectResult<(RelationDefinition, bool)> {
    for r in definitions(c).await? {
        if r.source_type == ty && r.source_property == key {
            return Ok((r, true));
        }
        if r.target_type.as_deref() == Some(ty) && r.target_property.as_deref() == Some(key) {
            return Ok((r, false));
        }
    }
    Err(ObjectError::invalid("字段不是该类型的关系端点"))
}
/// schema 定义一旦建立即不可就地修改。收紧约束必须另行做显式数据迁移。
pub(super) async fn define(
    c: &Connection,
    d: &RelationDefinition,
    system: bool,
) -> ObjectResult<()> {
    if !system {
        validation::custom_key(&d.key)?;
        validation::custom_key(&d.source_property)?;
        if let Some(key) = &d.target_property {
            validation::custom_key(key)?;
        }
    }
    validation::label(&d.name, "关系名称", 200)?;
    if catalog::type_record(c, &d.source_type).await?.retired {
        return Err(ObjectError::precondition("来源类型已停用"));
    }
    if let Some(ty) = &d.target_type
        && catalog::type_record(c, ty).await?.retired
    {
        return Err(ObjectError::precondition("目标类型已停用"));
    }
    if d.target_property.is_some() && d.target_type.is_none() {
        return Err(ObjectError::invalid("反向属性需要明确的目标类型"));
    }
    if d.target_property.as_deref() == Some(d.source_property.as_str()) {
        return Err(ObjectError::invalid("正反属性 key 必须不同"));
    }
    if d.acyclic && d.target_type.as_deref() != Some(d.source_type.as_str()) {
        return Err(ObjectError::invalid("无环关系必须连接同一类型"));
    }
    if exists(
        c,
        "SELECT 1 FROM object_relation_types WHERE key=?1",
        vec![s(&d.key)],
    )
    .await?
    {
        return Err(ObjectError::conflict("关系 key 已存在"));
    }
    for key in std::iter::once(&d.source_property).chain(d.target_property.iter()) {
        if exists(
            c,
            "SELECT 1 FROM object_properties WHERE key=?1",
            vec![s(key)],
        )
        .await?
        {
            return Err(ObjectError::conflict("关系属性 key 已被占用"));
        }
    }
    insert_fields(c, d, system).await?;
    insert_definition(c, d, system).await
}
pub(super) async fn insert_fields(
    c: &Connection,
    d: &RelationDefinition,
    system: bool,
) -> ObjectResult<()> {
    let mut fields = vec![(
        &d.source_type,
        &d.source_property,
        d.target_type.clone(),
        d.source_cardinality,
    )];
    if let (Some(ty), Some(key)) = (&d.target_type, &d.target_property) {
        fields.push((ty, key, Some(d.source_type.clone()), d.target_cardinality));
    }
    for (ty, key, target_type, cardinality) in fields {
        if catalog::bindings(c, ty).await?.len() >= 128 {
            return Err(ObjectError::precondition("类型字段数量达到上限"));
        }
        let p = PropertyDefinition {
            key: key.clone(),
            name: d.name.clone(),
            kind: PropertyKind::Object,
            cardinality,
            reference_type: target_type,
            options: vec![],
        };
        catalog::insert_property(c, &p, system).await?;
        catalog::write_binding(
            c,
            &BindingRecord {
                binding: PropertyBinding {
                    type_key: ty.clone(),
                    property_key: key.clone(),
                    required: false,
                    position: 1000,
                    defaults: vec![],
                },
                storage: "value".into(),
                writable: true,
            },
        )
        .await?;
    }
    Ok(())
}
pub(super) async fn insert_definition(
    c: &Connection,
    d: &RelationDefinition,
    system: bool,
) -> ObjectResult<()> {
    exec(c, "INSERT INTO object_relation_types(key,name,source_type,target_type,source_property,target_property,source_cardinality,target_cardinality,acyclic,system,version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1)",
        vec![s(&d.key),s(&d.name),s(&d.source_type),os(d.target_type.as_deref()),s(&d.source_property),os(d.target_property.as_deref()),s(d.source_cardinality.key()),s(d.target_cardinality.key()),n(i64::from(d.acyclic)),n(i64::from(system))]).await?;
    Ok(())
}
/// 返回字段的精确引用集合。反向字段直接读取同一批边，不保留第二份数组。
pub(super) async fn ids(
    c: &Connection,
    board: &str,
    id: &str,
    ty: &str,
    property: &str,
) -> ObjectResult<Vec<String>> {
    let (d, forward) = field(c, ty, property).await?;
    let sql = if forward {
        "SELECT target_id FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3 ORDER BY target_id"
    } else {
        "SELECT source_id FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND target_id=?3 ORDER BY source_id"
    };
    rows(c, sql, vec![s(board), s(&d.key), s(id)])
        .await?
        .iter()
        .map(|r| text(&r[0]))
        .collect()
}
pub(super) async fn read_fields(
    c: &Connection,
    board: &str,
    id: &str,
    ty: &str,
) -> ObjectResult<BTreeMap<String, Vec<PropertyValue>>> {
    let mut out = BTreeMap::new();
    for d in definitions(c).await? {
        let mut keys = vec![];
        if d.source_type == ty {
            keys.push(d.source_property.clone());
        }
        if d.target_type.as_deref() == Some(ty) {
            keys.extend(d.target_property.clone());
        }
        for key in keys {
            let values = ids(c, board, id, ty, &key)
                .await?
                .into_iter()
                .map(PropertyValue::Object)
                .collect();
            out.insert(key, values);
        }
    }
    Ok(out)
}
pub(super) async fn validate_link(
    c: &Connection,
    board: &str,
    link: &RelationLink,
    adding: bool,
    internal: bool,
) -> ObjectResult<()> {
    let d = definition(c, &link.relation_key).await?;
    let source = get(c, board, &link.source_id).await?;
    let target = get(c, board, &link.target_id).await?;
    if source.type_key != d.source_type
        || d.target_type
            .as_ref()
            .is_some_and(|ty| ty != &target.type_key)
    {
        return Err(ObjectError::invalid("关系两端类型不匹配"));
    }
    if adding && (source.archived_at.is_some() || target.archived_at.is_some()) {
        return Err(ObjectError::precondition("不能为已归档对象新增关系"));
    }
    if !internal {
        workflow::ensure_editable(c, &source).await?;
        workflow::ensure_editable(c, &target).await?;
    }
    Ok(())
}
/// 同一事务先删除再添加。数据库的两条通用 UNIQUE 索引决定 one / many。
pub(super) async fn write_links(
    c: &Connection,
    board: &str,
    remove: &[RelationLink],
    add: &[RelationLink],
    now: i64,
    internal: bool,
) -> ObjectResult<Vec<String>> {
    let mut touched = BTreeSet::new();
    for link in remove.iter().chain(add) {
        touched.insert(link.source_id.clone());
        touched.insert(link.target_id.clone());
    }
    for link in remove {
        validate_link(c, board, link, false, internal).await?;
    }
    for link in add {
        validate_link(c, board, link, true, internal).await?;
    }
    for link in remove {
        if exec(c,"DELETE FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3 AND target_id=?4",vec![s(board),s(&link.relation_key),s(&link.source_id),s(&link.target_id)]).await? != 1 { return Err(ObjectError::conflict("待删除关系不存在")); }
    }
    for link in add {
        let d = definition(c, &link.relation_key).await?;
        if exists(c,"SELECT 1 FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3 AND target_id=?4",vec![s(board),s(&d.key),s(&link.source_id),s(&link.target_id)]).await? {return Err(ObjectError::conflict("关系已存在"));}
        if (d.source_cardinality==Cardinality::One && exists(c,"SELECT 1 FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3",vec![s(board),s(&d.key),s(&link.source_id)]).await?)
          || (d.target_cardinality==Cardinality::One && exists(c,"SELECT 1 FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND target_id=?3",vec![s(board),s(&d.key),s(&link.target_id)]).await?) {
            return Err(ObjectError::conflict("关系单值端已经被占用；换属必须在同一请求中显式删除原关系"));
        }
        if d.acyclic
            && (link.source_id == link.target_id
                || reaches(c, board, &d.key, &link.target_id, &link.source_id).await?)
        {
            return Err(ObjectError::precondition("该引用会在无环关系中形成闭环"));
        }
        exec(c,"INSERT INTO object_relation_edges(board_id,relation_key,source_id,target_id,source_type,source_cardinality,target_cardinality,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",vec![s(board),s(&d.key),s(&link.source_id),s(&link.target_id),s(&d.source_type),s(d.source_cardinality.key()),s(d.target_cardinality.key()),n(now)]).await?;
    }
    for id in &touched {
        validate_object(c, &get(c, board, id).await?).await?;
    }
    Ok(touched.into_iter().collect())
}
pub(super) async fn change(
    c: &Connection,
    board: &str,
    change: &RelationChange,
    now: i64,
) -> ObjectResult<Vec<String>> {
    expect_catalog(c, change.expected_catalog_version).await?;
    if change.add.len() + change.remove.len() > 200 {
        return Err(ObjectError::invalid("一次最多变更 200 条边"));
    }
    let remove = change.remove.iter().collect::<BTreeSet<_>>();
    let add = change.add.iter().collect::<BTreeSet<_>>();
    if remove.len() != change.remove.len()
        || add.len() != change.add.len()
        || !remove.is_disjoint(&add)
    {
        return Err(ObjectError::invalid("重复或互相抵消的关系变更"));
    }
    let touched = change
        .add
        .iter()
        .chain(&change.remove)
        .flat_map(|l| [l.source_id.clone(), l.target_id.clone()])
        .collect::<BTreeSet<_>>();
    let mut before = BTreeMap::new();
    for expected in &change.expected {
        let object = target(c, board, expected, false).await?;
        if before.insert(object.id.clone(), object).is_some() {
            return Err(ObjectError::invalid("重复的 expected 对象"));
        }
    }
    if before.keys().cloned().collect::<BTreeSet<_>>() != touched {
        return Err(ObjectError::invalid(
            "expected 必须精确包含所有变动端点，包括原归属对象",
        ));
    }
    let ids = write_links(c, board, &change.remove, &change.add, now, false).await?;
    for id in &ids {
        bump(
            c,
            before
                .get(id)
                .ok_or_else(|| ObjectError::storage("缺少端点预读"))?,
            now,
        )
        .await?;
    }
    Ok(ids)
}
#[expect(
    clippy::too_many_arguments,
    reason = "同时校验关系双方版本、目录版本和事务时钟"
)]
pub(super) async fn set_references(
    c: &Connection,
    board: &str,
    root: &ObjectTarget,
    property: &str,
    values: &[String],
    expected: &[ObjectTarget],
    catalog: i64,
    now: i64,
) -> ObjectResult<Vec<String>> {
    expect_catalog(c, catalog).await?;
    let object = target(c, board, root, false).await?;
    let (d, forward) = field(c, &object.type_key, property).await?;
    let old = ids(c, board, &root.id, &object.type_key, property)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let new = values.iter().cloned().collect::<BTreeSet<_>>();
    if new.len() != values.len() || new.len() > MAX_MEMBERS as usize {
        return Err(ObjectError::invalid("引用集合重复或超过上限"));
    }
    let make = |peer: &String| RelationLink {
        relation_key: d.key.clone(),
        source_id: if forward {
            root.id.clone()
        } else {
            peer.clone()
        },
        target_id: if forward {
            peer.clone()
        } else {
            root.id.clone()
        },
    };
    let remove = old.difference(&new).map(make).collect::<Vec<_>>();
    let add = new.difference(&old).map(make).collect::<Vec<_>>();
    if remove.is_empty() && add.is_empty() {
        if !expected.is_empty() {
            return Err(ObjectError::invalid("无变化时 expected 应为空"));
        }
        return Ok(vec![]);
    }
    let mut all = expected.to_vec();
    all.push(root.clone());
    change(
        c,
        board,
        &RelationChange {
            expected_catalog_version: catalog,
            expected: all,
            remove,
            add,
        },
        now,
    )
    .await
}
pub(super) async fn validate_object(c: &Connection, object: &ObjectRecord) -> ObjectResult<()> {
    for d in definitions(c).await? {
        for (forward, applies) in [
            (true, d.source_type == object.type_key),
            (
                false,
                d.target_type
                    .as_ref()
                    .is_none_or(|ty| ty == &object.type_key),
            ),
        ] {
            if !applies {
                continue;
            }
            let sql = if forward {
                "SELECT COUNT(*) FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3"
            } else {
                "SELECT COUNT(*) FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND target_id=?3"
            };
            let number = count(c, sql, vec![s(&object.board_id), s(&d.key), s(&object.id)]).await?;
            let cardinality = if forward {
                d.source_cardinality
            } else {
                d.target_cardinality
            };
            if number > MAX_MEMBERS || (cardinality == Cardinality::One && number > 1) {
                return Err(ObjectError::precondition("关系基数或成员上限不符合定义"));
            }
        }
        if d.acyclic && d.source_type == object.type_key {
            if reaches(c, &object.board_id, &d.key, &object.id, &object.id).await? {
                return Err(ObjectError::precondition("无环关系中发现环"));
            }
            if object.archived_at.is_none() && exists(c,"SELECT 1 FROM object_relation_edges e JOIN objects p ON p.id=e.source_id WHERE e.board_id=?1 AND e.relation_key=?2 AND e.target_id=?3 AND p.archived_at IS NOT NULL",vec![s(&object.board_id),s(&d.key),s(&object.id)]).await? {return Err(ObjectError::precondition("有效层级对象的上级已归档"));}
        }
    }
    Ok(())
}
pub(super) async fn ensure_archivable(c: &Connection, o: &ObjectRecord) -> ObjectResult<()> {
    for d in definitions(c).await? {
        if d.acyclic && d.source_type==o.type_key && exists(c,"SELECT 1 FROM object_relation_edges e JOIN objects child ON child.id=e.target_id WHERE e.board_id=?1 AND e.relation_key=?2 AND e.source_id=?3 AND child.archived_at IS NULL LIMIT 1",vec![s(&o.board_id),s(&d.key),s(&o.id)]).await? {return Err(ObjectError::precondition("层级对象仍有未归档下级"));}
    }
    Ok(())
}

/// Turso 不支持递归 CTE；只沿当前 board / relation 遍历，超过预算即拒绝写入。
async fn reaches(
    c: &Connection,
    board: &str,
    relation: &str,
    from: &str,
    target: &str,
) -> ObjectResult<bool> {
    let mut pending = std::collections::VecDeque::from([from.to_owned()]);
    let mut visited = BTreeSet::from([from.to_owned()]);
    let mut edges = 0usize;
    while let Some(id) = pending.pop_front() {
        let next = rows(c, "SELECT target_id FROM object_relation_edges WHERE board_id=?1 AND relation_key=?2 AND source_id=?3 LIMIT 5001", vec![s(board), s(relation), s(&id)]).await?;
        edges += next.len();
        if next.len() > MAX_MEMBERS as usize || edges > 100_000 {
            return Err(ObjectError::precondition("关系遍历超过边数预算"));
        }
        for row in next {
            let id = text(&row[0])?;
            if id == target {
                return Ok(true);
            }
            if visited.insert(id.clone()) {
                if visited.len() > 10_000 {
                    return Err(ObjectError::precondition("关系遍历超过对象预算"));
                }
                pending.push_back(id);
            }
        }
    }
    Ok(false)
}
