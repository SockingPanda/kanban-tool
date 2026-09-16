//! 聚合规则通过关系端点和成员字段定义，不依赖容器类型名。
use super::{catalog, error::*, model::*, relations, store::*, validation, workflow};
use turso::Connection;
pub(super) async fn definitions(c: &Connection) -> ObjectResult<Vec<RollupDefinition>> {
    rows(
        c,
        "SELECT definition_json FROM object_rollups ORDER BY key",
        vec![],
    )
    .await?
    .iter()
    .map(|r| Ok(serde_json::from_str(&text(&r[0])?)?))
    .collect()
}
pub(super) async fn validate_definition(c: &Connection, d: &RollupDefinition) -> ObjectResult<()> {
    if catalog::type_record(c, &d.type_key).await?.retired {
        return Err(ObjectError::precondition("聚合类型已停用"));
    }
    let (r, forward) = relations::field(c, &d.type_key, &d.members_property).await?;
    let member = if forward {
        r.target_type
    } else {
        Some(r.source_type)
    }
    .ok_or_else(|| ObjectError::invalid("聚合需要具体成员类型"))?;
    workflow::validate_status(
        c,
        &member,
        &d.status_property,
        &d.done_values,
        &d.excluded_values,
    )
    .await?;
    if let Some(w) = workflow::for_type(c, &d.type_key).await? {
        compatible_workflow(d, &w)?;
    }
    let p = catalog::property(c, &d.status_property).await?;
    for state in &d.blocked_values {
        validation::scalar_values(&p.definition, &[PropertyValue::Select(state.clone())], true)?;
    }
    Ok(())
}
/// 冻结快照只保存生命周期定义中的成员状态；聚合不能换用另一批成员或状态字段。
pub(super) fn compatible_workflow(
    d: &RollupDefinition,
    w: &WorkflowDefinition,
) -> ObjectResult<()> {
    if d.members_property != w.members_property || d.status_property != w.member_status_property {
        return Err(ObjectError::invalid(
            "聚合与生命周期必须使用相同的成员关系和成员状态字段",
        ));
    }
    Ok(())
}
pub(super) async fn define(c: &Connection, d: &RollupDefinition, system: bool) -> ObjectResult<()> {
    if !system {
        validation::custom_key(&d.key)?;
    }
    validate_definition(c, d).await?;
    exec(c,"INSERT INTO object_rollups(key,type_key,definition_json,system,version) VALUES (?1,?2,?3,?4,1)",vec![s(&d.key),s(&d.type_key),s(&serde_json::to_string(d)?),n(i64::from(system))]).await?;
    Ok(())
}
pub(super) async fn overview(
    c: &Connection,
    board: &str,
    id: &str,
) -> ObjectResult<PlanningOverview> {
    let object = get(c, board, id).await?;
    let d = definitions(c)
        .await?
        .into_iter()
        .find(|d| d.type_key == object.type_key)
        .ok_or_else(|| ObjectError::invalid("类型没有聚合定义"))?;
    let closure = workflow::closure(c, board, id).await?;
    let frozen = closure.is_some();
    let statuses = if let Some(closure) = closure {
        closure
            .members
            .into_iter()
            .map(|m| m.status)
            .collect::<Vec<_>>()
    } else {
        let ids = relations::ids(c, board, id, &object.type_key, &d.members_property).await?;
        let mut out = Vec::new();
        for id in ids {
            let member = get(c, board, &id).await?;
            out.push(
                one_select(&member, &d.status_property)
                    .ok_or_else(|| ObjectError::precondition("聚合成员缺少状态"))?,
            );
        }
        out
    };
    let mut progress = Progress::default();
    for status in statuses {
        progress.total += 1;
        if d.done_values.contains(&status) {
            progress.done += 1;
        }
        if d.excluded_values.contains(&status) {
            progress.archived += 1;
        }
        if d.blocked_values.contains(&status) {
            progress.blocked += 1;
        }
    }
    let denominator = progress.total - progress.archived;
    progress.completion_ratio = if denominator == 0 {
        None
    } else {
        Some(progress.done as f64 / denominator as f64)
    };
    Ok(PlanningOverview {
        object,
        progress,
        frozen,
    })
}
