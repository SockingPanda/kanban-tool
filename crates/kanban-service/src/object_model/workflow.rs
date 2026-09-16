//! 配置驱动的有时间窗生命周期。状态迁移算法固定，类型和字段名称全部来自目录。
use super::{catalog, error::*, model::*, relations, store::*, validation};
use std::collections::{BTreeMap, BTreeSet};
use turso::Connection;

pub(super) async fn definitions(c: &Connection) -> ObjectResult<Vec<WorkflowDefinition>> {
    rows(
        c,
        "SELECT definition_json FROM object_workflows ORDER BY key",
        vec![],
    )
    .await?
    .iter()
    .map(|r| Ok(serde_json::from_str(&text(&r[0])?)?))
    .collect()
}
pub(super) async fn for_type(c: &Connection, ty: &str) -> ObjectResult<Option<WorkflowDefinition>> {
    let r = rows(
        c,
        "SELECT definition_json FROM object_workflows WHERE type_key=?1",
        vec![s(ty)],
    )
    .await?;
    r.first()
        .map(|r| Ok(serde_json::from_str(&text(&r[0])?)?))
        .transpose()
}
pub(super) async fn validate_definition(
    c: &Connection,
    d: &WorkflowDefinition,
) -> ObjectResult<()> {
    let ty = catalog::type_record(c, &d.type_key).await?;
    if ty.capability != "plain" || ty.retired {
        return Err(ObjectError::precondition("生命周期需要有效的普通对象类型"));
    }
    let keys = [
        &d.status_property,
        &d.starts_property,
        &d.ends_property,
        &d.started_property,
        &d.closed_property,
    ];
    if keys.iter().copied().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(ObjectError::invalid("生命周期字段映射必须互不重复"));
    }
    for key in keys {
        let b = catalog::binding(c, &d.type_key, key).await?;
        let p = catalog::property(c, key).await?;
        let kind = if key == &d.status_property {
            PropertyKind::Select
        } else {
            PropertyKind::Date
        };
        if b.storage != "value"
            || p.definition.kind != kind
            || p.definition.cardinality != Cardinality::One
        {
            return Err(ObjectError::invalid(
                "生命周期字段必须绑定到单值日期或状态选项",
            ));
        }
        if key == &d.started_property || key == &d.closed_property {
            if b.binding.required || !b.binding.defaults.is_empty() {
                return Err(ObjectError::invalid(
                    "开始/关闭的实际时间必须可空且无默认值",
                ));
            }
        } else if !b.binding.required {
            return Err(ObjectError::invalid("计划时间窗和状态必须必填"));
        }
    }
    let states = [
        &d.planned_value,
        &d.active_value,
        &d.completed_value,
        &d.cancelled_value,
    ];
    if states.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
        return Err(ObjectError::invalid("生命周期四个状态值必须不同"));
    }
    let status = catalog::property(c, &d.status_property).await?;
    for value in states {
        validation::scalar_values(
            &status.definition,
            &[PropertyValue::Select(value.clone())],
            true,
        )?;
    }
    if catalog::binding(c, &d.type_key, &d.status_property)
        .await?
        .binding
        .defaults
        != vec![PropertyValue::Select(d.planned_value.clone())]
    {
        return Err(ObjectError::invalid("状态默认值必须是 planned_value"));
    }
    let (r, forward) = relations::field(c, &d.type_key, &d.members_property).await?;
    if !forward || r.source_cardinality != Cardinality::Many {
        return Err(ObjectError::invalid(
            "生命周期成员字段必须是关系的正向多值端",
        ));
    }
    let member_type = r
        .target_type
        .as_ref()
        .ok_or_else(|| ObjectError::invalid("生命周期成员必须指定类型"))?;
    if member_type == &d.type_key {
        return Err(ObjectError::invalid(
            "生命周期成员类型不能是生命周期容器本身",
        ));
    }
    validate_status(
        c,
        member_type,
        &d.member_status_property,
        &d.done_values,
        &d.excluded_values,
    )
    .await?;
    for rollup in super::rollup::definitions(c).await? {
        if rollup.type_key == d.type_key {
            super::rollup::compatible_workflow(&rollup, d)?;
        }
    }
    Ok(())
}
pub(super) async fn define(
    c: &Connection,
    d: &WorkflowDefinition,
    system: bool,
) -> ObjectResult<()> {
    if !system {
        validation::custom_key(&d.key)?;
    }
    validate_definition(c, d).await?;
    if exists(
        c,
        "SELECT 1 FROM objects WHERE type_key=?1 LIMIT 1",
        vec![s(&d.type_key)],
    )
    .await?
    {
        return Err(ObjectError::precondition(
            "已有实例时不能追加生命周期约束；需要显式迁移",
        ));
    }
    exec(c,"INSERT INTO object_workflows(key,type_key,definition_json,system,version) VALUES (?1,?2,?3,?4,1)",vec![s(&d.key),s(&d.type_key),s(&serde_json::to_string(d)?),n(i64::from(system))]).await?;
    Ok(())
}
pub(super) fn protected(d: &WorkflowDefinition, key: &str) -> bool {
    key == d.status_property || key == d.started_property || key == d.closed_property
}
pub(super) async fn property_writable(c: &Connection, ty: &str, key: &str) -> ObjectResult<()> {
    if for_type(c, ty)
        .await?
        .as_ref()
        .is_some_and(|d| protected(d, key))
    {
        return Err(ObjectError::precondition(
            "生命周期状态和实际时间只能由生命周期动作写入",
        ));
    }
    Ok(())
}
pub(super) async fn ensure_editable(c: &Connection, o: &ObjectRecord) -> ObjectResult<()> {
    if let Some(d) = for_type(c, &o.type_key).await?
        && one_date(o, &d.closed_property).is_some()
    {
        return Err(ObjectError::precondition("已关闭的生命周期对象保持冻结"));
    }
    Ok(())
}
pub(super) async fn validate_object(c: &Connection, o: &ObjectRecord) -> ObjectResult<()> {
    if let Some(d) = for_type(c, &o.type_key).await? {
        let start =
            one_date(o, &d.starts_property).ok_or_else(|| ObjectError::invalid("缺少开始日期"))?;
        let end =
            one_date(o, &d.ends_property).ok_or_else(|| ObjectError::invalid("缺少结束日期"))?;
        if start >= end {
            return Err(ObjectError::invalid("时间窗必须满足 starts_at < ends_at"));
        }
        let status = one_select(o, &d.status_property)
            .ok_or_else(|| ObjectError::invalid("缺少生命周期状态"))?;
        if ![
            &d.planned_value,
            &d.active_value,
            &d.completed_value,
            &d.cancelled_value,
        ]
        .contains(&&status)
        {
            return Err(ObjectError::precondition("状态不在生命周期定义中"));
        }
        let closed = one_date(o, &d.closed_property).is_some();
        if closed != (status == d.completed_value || status == d.cancelled_value) {
            return Err(ObjectError::precondition("状态与关闭时间不一致"));
        }
        if status == d.active_value && one_date(o, &d.started_property).is_none() {
            return Err(ObjectError::precondition("活动状态缺少实际开始时间"));
        }
        if status == d.planned_value && one_date(o, &d.started_property).is_some() {
            return Err(ObjectError::precondition("计划状态不能包含实际开始时间"));
        }
        if status == d.completed_value && one_date(o, &d.started_property).is_none() {
            return Err(ObjectError::precondition("完成状态缺少实际开始时间"));
        }
        if one_date(o, &d.started_property)
            .zip(one_date(o, &d.closed_property))
            .is_some_and(|(a, b)| a > b)
        {
            return Err(ObjectError::precondition("实际关闭时间早于实际开始时间"));
        }
        if o.archived_at.is_some() && !closed {
            return Err(ObjectError::precondition("归档前先关闭生命周期"));
        }
        if closed
            && !relations::ids(c, &o.board_id, &o.id, &o.type_key, &d.members_property)
                .await?
                .is_empty()
        {
            return Err(ObjectError::precondition("关闭对象仍占用实时成员关系"));
        }
    }
    Ok(())
}
pub(super) async fn set(
    c: &Connection,
    o: &ObjectRecord,
    key: &str,
    v: &[PropertyValue],
) -> ObjectResult<()> {
    let p = catalog::property(c, key).await?;
    validation::values(c, &o.board_id, &p.definition, v, false, true).await?;
    if p.definition.kind == PropertyKind::Object {
        return Err(ObjectError::invalid("对象引用必须通过关系引擎写入"));
    }
    set_values(c, o, &p.definition, v).await
}
async fn open(
    c: &Connection,
    board: &str,
    expected: &ObjectTarget,
) -> ObjectResult<(ObjectRecord, WorkflowDefinition)> {
    let o = target(c, board, expected, true).await?;
    let d = for_type(c, &o.type_key)
        .await?
        .ok_or_else(|| ObjectError::invalid("类型未绑定生命周期"))?;
    ensure_editable(c, &o).await?;
    Ok((o, d))
}
pub(super) async fn start(
    c: &Connection,
    board: &str,
    expected: &ObjectTarget,
    now: i64,
) -> ObjectResult<Vec<String>> {
    let (o, d) = open(c, board, expected).await?;
    if one_select(&o, &d.status_property).as_deref() != Some(d.planned_value.as_str()) {
        return Err(ObjectError::precondition("只有计划状态可以启动"));
    }
    set(
        c,
        &o,
        &d.status_property,
        &[PropertyValue::Select(d.active_value)],
    )
    .await?;
    set(c, &o, &d.started_property, &[PropertyValue::Date(now)]).await?;
    bump(c, &o, now).await?;
    Ok(vec![o.id])
}
pub(super) async fn close(
    c: &Connection,
    board: &str,
    expected: &ObjectTarget,
    carry_to: Option<&ObjectTarget>,
    cancel: bool,
    now: i64,
) -> ObjectResult<Vec<String>> {
    let (o, d) = open(c, board, expected).await?;
    if !cancel && one_select(&o, &d.status_property).as_deref() != Some(d.active_value.as_str()) {
        return Err(ObjectError::precondition("只有活动状态可以完成"));
    }
    let carry = match carry_to {
        Some(t) => {
            if cancel || t.id == o.id {
                return Err(ObjectError::invalid("结转目标无效"));
            }
            let (v, other) = open(c, board, t).await?;
            if other.key != d.key {
                return Err(ObjectError::invalid("结转双方必须使用同一生命周期定义"));
            }
            Some(v)
        }
        None => None,
    };
    let (rel, _) = relations::field(c, &o.type_key, &d.members_property).await?;
    let mut before = BTreeMap::new();
    before.insert(o.id.clone(), o.clone());
    let mut remove = Vec::new();
    let mut add = Vec::new();
    let mut frozen = Vec::new();
    let member_ids = relations::ids(c, board, &o.id, &o.type_key, &d.members_property).await?;
    if member_ids.len() > relations::MAX_MEMBERS as usize {
        return Err(ObjectError::precondition("成员数量超过同步关闭上限"));
    }
    for id in member_ids {
        let member = get(c, board, &id).await?;
        let status = one_select(&member, &d.member_status_property)
            .ok_or_else(|| ObjectError::precondition("成员缺少用于结转判断的状态"))?;
        let to = carry
            .as_ref()
            .filter(|_| {
                !d.done_values.contains(&status)
                    && !d.excluded_values.contains(&status)
                    && member.archived_at.is_none()
            })
            .map(|v| v.id.clone());
        remove.push(RelationLink {
            relation_key: rel.key.clone(),
            source_id: o.id.clone(),
            target_id: id.clone(),
        });
        if let Some(to) = &to {
            add.push(RelationLink {
                relation_key: rel.key.clone(),
                source_id: to.clone(),
                target_id: id.clone(),
            });
        }
        frozen.push(ClosedMember {
            id: id.clone(),
            title: member.title.clone(),
            status,
            version: member.version.clone(),
            carried_to: to,
        });
        before.insert(id, member);
    }
    if !add.is_empty()
        && let Some(carry) = carry
    {
        before.insert(carry.id.clone(), carry);
    }
    relations::write_links(c, board, &remove, &add, now, true).await?;
    set(
        c,
        &o,
        &d.status_property,
        &[PropertyValue::Select(if cancel {
            d.cancelled_value
        } else {
            d.completed_value
        })],
    )
    .await?;
    set(c, &o, &d.closed_property, &[PropertyValue::Date(now)]).await?;
    for member in before.values() {
        bump(c, member, now).await?;
    }
    let object = get(c, board, &o.id).await?;
    validate_object(c, &object).await?;
    snapshot(
        c,
        board,
        &o.id,
        "workflow.closed",
        &serde_json::to_value(WorkflowClosure {
            object,
            members: frozen,
            captured_at: now,
        })?,
        now,
    )
    .await?;
    Ok(before.into_keys().collect())
}
pub(super) async fn closure(
    c: &Connection,
    board: &str,
    id: &str,
) -> ObjectResult<Option<WorkflowClosure>> {
    let r=rows(c,"SELECT body_json FROM object_snapshots WHERE board_id=?1 AND object_id=?2 AND kind='workflow.closed'",vec![s(board),s(id)]).await?;
    r.first()
        .map(|r| Ok(serde_json::from_str(&text(&r[0])?)?))
        .transpose()
}
pub(super) async fn validate_status(
    c: &Connection,
    ty: &str,
    key: &str,
    done: &[String],
    excluded: &[String],
) -> ObjectResult<()> {
    catalog::binding(c, ty, key).await?;
    let p = catalog::property(c, key).await?;
    if p.definition.kind != PropertyKind::Select || p.definition.cardinality != Cardinality::One {
        return Err(ObjectError::invalid("聚合状态必须是单值选择属性"));
    }
    for value in done.iter().chain(excluded) {
        validation::scalar_values(&p.definition, &[PropertyValue::Select(value.clone())], true)?;
    }
    if done.is_empty() || done.iter().any(|s| excluded.contains(s)) {
        return Err(ObjectError::invalid("完成状态不能为空且不能与排除状态相交"));
    }
    Ok(())
}

/// 新增空的关系端点只扩展读模型，不改变历史事实。已有属性必须保持相同；引用按集合比较。
pub(super) async fn frozen_properties_match(
    c: &Connection,
    frozen: &ObjectRecord,
    current: &ObjectRecord,
) -> ObjectResult<bool> {
    for (key, old) in &frozen.properties {
        let Some(new) = current.properties.get(key) else {
            return Ok(false);
        };
        if catalog::property(c, key).await?.definition.kind == PropertyKind::Object {
            let normalize = |v: &[PropertyValue]| -> ObjectResult<BTreeSet<String>> {
                v.iter()
                    .map(|v| serde_json::to_string(v).map_err(ObjectError::from))
                    .collect()
            };
            if normalize(old)? != normalize(new)? {
                return Ok(false);
            }
        } else if old != new {
            return Ok(false);
        }
    }
    for (key, values) in &current.properties {
        if !frozen.properties.contains_key(key)
            && (!values.is_empty()
                || catalog::property(c, key).await?.definition.kind != PropertyKind::Object)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
