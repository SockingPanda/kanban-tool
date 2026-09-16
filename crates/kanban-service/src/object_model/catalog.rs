//! 类型定义是数据。字段的 kind/cardinality 不允许在已有数据上隐式转换。
use super::{error::*, model::*, relations, rollup, store::*, validation, workflow};
use serde::Deserialize;
use turso::Connection;

pub(crate) const SEED: &str = include_str!("seed.json");
#[derive(Deserialize)]
pub(super) struct SeedType {
    pub definition: TypeDefinition,
    pub capability: String,
}
#[derive(Deserialize)]
pub(super) struct Seed {
    pub types: Vec<SeedType>,
    pub properties: Vec<PropertyDefinition>,
    pub bindings: Vec<BindingRecord>,
    #[serde(default)]
    pub relations: Vec<RelationDefinition>,
    #[serde(default)]
    pub workflows: Vec<WorkflowDefinition>,
    #[serde(default)]
    pub rollups: Vec<RollupDefinition>,
}
pub(super) async fn seed(c: &Connection) -> ObjectResult<()> {
    let seed: Seed = serde_json::from_str(SEED)?;
    for ty in &seed.types {
        insert_type(c, &ty.definition, &ty.capability, true).await?;
    }
    for prop in &seed.properties {
        insert_property(c, prop, true).await?;
    }
    for binding in &seed.bindings {
        write_binding(c, binding).await?;
    }
    for relation in &seed.relations {
        relations::define(c, relation, true).await?;
    }
    for behavior in &seed.workflows {
        workflow::define(c, behavior, true).await?;
    }
    for behavior in &seed.rollups {
        rollup::define(c, behavior, true).await?;
    }
    Ok(())
}
pub(super) async fn type_record(c: &Connection, key: &str) -> ObjectResult<TypeRecord> {
    let r=rows(c,"SELECT key,name,layout_json,capability,system,retired,version FROM object_types WHERE key=?1",vec![s(key)]).await?;
    let r = r
        .first()
        .ok_or_else(|| ObjectError::missing(format!("类型 {key} 不存在")))?;
    Ok(TypeRecord {
        definition: TypeDefinition {
            key: text(&r[0])?,
            name: text(&r[1])?,
            layout: serde_json::from_str(&text(&r[2])?)?,
        },
        capability: text(&r[3])?,
        system: int(&r[4])? != 0,
        retired: int(&r[5])? != 0,
        version: int(&r[6])?,
    })
}
pub(super) async fn property(c: &Connection, key: &str) -> ObjectResult<PropertyRecord> {
    let r=rows(c,"SELECT key,name,kind,cardinality,reference_type,system,version FROM object_properties WHERE key=?1",vec![s(key)]).await?;
    let r = r
        .first()
        .ok_or_else(|| ObjectError::missing(format!("属性 {key} 不存在")))?;
    let mut options = Vec::new();
    for row in rows(
        c,
        "SELECT key,name FROM object_options WHERE property_key=?1 ORDER BY key",
        vec![s(key)],
    )
    .await?
    {
        options.push(SelectOption {
            key: text(&row[0])?,
            name: text(&row[1])?,
        });
    }
    Ok(PropertyRecord {
        definition: PropertyDefinition {
            key: text(&r[0])?,
            name: text(&r[1])?,
            kind: serde_json::from_value(serde_json::Value::String(text(&r[2])?))?,
            cardinality: serde_json::from_value(serde_json::Value::String(text(&r[3])?))?,
            reference_type: opt_text(&r[4])?,
            options,
        },
        system: int(&r[5])? != 0,
        version: int(&r[6])?,
    })
}
pub(super) async fn bindings(c: &Connection, ty: &str) -> ObjectResult<Vec<BindingRecord>> {
    let mut out = Vec::new();
    for row in rows(c,"SELECT type_key,property_key,required,position,defaults_json,storage,writable FROM object_type_properties WHERE type_key=?1 ORDER BY position,property_key",vec![s(ty)]).await? {
        out.push(BindingRecord{binding:PropertyBinding{type_key:text(&row[0])?,property_key:text(&row[1])?,required:int(&row[2])?!=0,position:int(&row[3])?,defaults:serde_json::from_str(&text(&row[4])?)?},storage:text(&row[5])?,writable:int(&row[6])?!=0});
    }
    Ok(out)
}
pub(super) async fn binding(c: &Connection, ty: &str, key: &str) -> ObjectResult<BindingRecord> {
    bindings(c, ty)
        .await?
        .into_iter()
        .find(|b| b.binding.property_key == key)
        .ok_or_else(|| ObjectError::invalid(format!("属性 {key} 未绑定到类型 {ty}")))
}
pub(super) async fn read(c: &Connection) -> ObjectResult<ObjectCatalog> {
    let mut types = Vec::new();
    let mut properties = Vec::new();
    let mut all_bindings = Vec::new();
    for row in rows(c, "SELECT key FROM object_types ORDER BY key", vec![]).await? {
        let key = text(&row[0])?;
        types.push(type_record(c, &key).await?);
        all_bindings.extend(bindings(c, &key).await?);
    }
    for row in rows(c, "SELECT key FROM object_properties ORDER BY key", vec![]).await? {
        properties.push(property(c, &text(&row[0])?).await?);
    }
    Ok(ObjectCatalog {
        version: catalog_version(c).await?,
        types,
        properties,
        bindings: all_bindings,
        relations: relations::definitions(c).await?,
        workflows: workflow::definitions(c).await?,
        rollups: rollup::definitions(c).await?,
    })
}
pub(super) async fn insert_type(
    c: &Connection,
    def: &TypeDefinition,
    cap: &str,
    system: bool,
) -> ObjectResult<()> {
    exec(c,"INSERT INTO object_types(key,name,layout_json,capability,system,retired,version) VALUES (?1,?2,?3,?4,?5,0,1)",vec![s(&def.key),s(&def.name),s(&serde_json::to_string(&def.layout)?),s(cap),n(i64::from(system))]).await?;
    Ok(())
}
pub(super) async fn insert_property(
    c: &Connection,
    def: &PropertyDefinition,
    system: bool,
) -> ObjectResult<()> {
    exec(c,"INSERT INTO object_properties(key,name,kind,cardinality,reference_type,system,version) VALUES (?1,?2,?3,?4,?5,?6,1)",vec![s(&def.key),s(&def.name),s(def.kind.key()),s(def.cardinality.key()),os(def.reference_type.as_deref()),n(i64::from(system))]).await?;
    for option in &def.options {
        exec(
            c,
            "INSERT INTO object_options(property_key,key,name) VALUES (?1,?2,?3)",
            vec![s(&def.key), s(&option.key), s(&option.name)],
        )
        .await?;
    }
    Ok(())
}
pub(super) async fn write_binding(c: &Connection, b: &BindingRecord) -> ObjectResult<()> {
    exec(c,"INSERT INTO object_type_properties(type_key,property_key,required,position,defaults_json,storage,writable) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(type_key,property_key) DO UPDATE SET required=excluded.required,position=excluded.position,defaults_json=excluded.defaults_json",
        vec![s(&b.binding.type_key),s(&b.binding.property_key),n(i64::from(b.binding.required)),n(b.binding.position),s(&serde_json::to_string(&b.binding.defaults)?),s(&b.storage),n(i64::from(b.writable))]).await?;
    Ok(())
}
pub(super) async fn define_type(c: &Connection, def: &TypeDefinition) -> ObjectResult<()> {
    validation::custom_key(&def.key)?;
    validation::label(&def.name, "类型名称", 200)?;
    validation::layout(&def.layout)?;
    if exists(
        c,
        "SELECT 1 FROM object_types WHERE key=?1",
        vec![s(&def.key)],
    )
    .await?
    {
        return Err(ObjectError::conflict("类型 key 已存在"));
    }
    insert_type(c, def, "plain", false).await
}
pub(super) async fn define_property(c: &Connection, def: &PropertyDefinition) -> ObjectResult<()> {
    validation::custom_key(&def.key)?;
    validation::property_definition(def)?;
    if def.kind == PropertyKind::Object {
        return Err(ObjectError::invalid(
            "引用属性请用 DefineRelation 同时定义两端和基数",
        ));
    }
    if exists(
        c,
        "SELECT 1 FROM object_properties WHERE key=?1",
        vec![s(&def.key)],
    )
    .await?
    {
        return Err(ObjectError::conflict(
            "属性 key 已存在，字段类型变更需要显式的数据转换",
        ));
    }
    if let Some(key) = &def.reference_type {
        type_record(c, key).await?;
    }
    insert_property(c, def, false).await
}
pub(super) async fn bind(c: &Connection, b: &PropertyBinding) -> ObjectResult<()> {
    let ty = type_record(c, &b.type_key).await?;
    if ty.retired {
        return Err(ObjectError::precondition("不能修改已停用类型的属性绑定"));
    }
    let prop = property(c, &b.property_key).await?;
    if prop.definition.kind == PropertyKind::Object
        || protected_binding(c, &b.type_key, &b.property_key).await?
    {
        return Err(ObjectError::precondition("关系或行为字段绑定不可就地修改"));
    }
    // 固定能力依赖的字段定义和绑定不能被目录命令改写。
    if prop.system {
        return Err(ObjectError::precondition("内置属性绑定由能力定义管理"));
    }
    if b.type_key == "task" && (b.required || !b.defaults.is_empty()) {
        return Err(ObjectError::precondition(
            "现有 create_task 不接受通用属性输入；任务自定义绑定只能是可选且无默认值",
        ));
    }
    validation::scalar_values(&prop.definition, &b.defaults, false)?;
    if b.defaults
        .iter()
        .any(|v| matches!(v, PropertyValue::Object(_)))
    {
        return Err(ObjectError::invalid(
            "全局类型默认值不能引用某个项目中的对象",
        ));
    }
    let existing = bindings(c, &b.type_key).await?;
    if !existing
        .iter()
        .any(|x| x.binding.property_key == b.property_key)
        && existing.len() >= 128
    {
        return Err(ObjectError::precondition("每个类型最多绑定 128 个属性"));
    }
    if b.required && exists(c,"SELECT 1 FROM objects o WHERE o.type_key=?1 AND NOT EXISTS (SELECT 1 FROM object_property_values v WHERE v.object_id=o.id AND v.property_key=?2) LIMIT 1",vec![s(&b.type_key),s(&b.property_key)]).await? {
        return Err(ObjectError::precondition("现有对象缺少该必填值；先添加可选绑定并补值，再设为必填。默认值只应用于新对象"));
    }
    write_binding(
        c,
        &BindingRecord {
            binding: b.clone(),
            storage: "value".into(),
            writable: true,
        },
    )
    .await
}
pub(super) async fn unbind(c: &Connection, ty: &str, key: &str) -> ObjectResult<()> {
    let p = property(c, key).await?;
    if p.definition.kind == PropertyKind::Object || protected_binding(c, ty, key).await? {
        return Err(ObjectError::precondition("关系或行为字段不能单独解绑"));
    }
    if p.system {
        return Err(ObjectError::precondition("内置属性不能解绑"));
    }
    binding(c, ty, key).await?;
    if exists(
        c,
        "SELECT 1 FROM object_property_slots WHERE type_key=?1 AND property_key=?2 LIMIT 1",
        vec![s(ty), s(key)],
    )
    .await?
    {
        return Err(ObjectError::precondition(
            "属性槽位仍被使用，拒绝隐式删除历史数据",
        ));
    }
    exec(
        c,
        "DELETE FROM object_type_properties WHERE type_key=?1 AND property_key=?2",
        vec![s(ty), s(key)],
    )
    .await?;
    Ok(())
}
pub(super) async fn rename_type(
    c: &Connection,
    key: &str,
    name: &str,
    layout: &serde_json::Value,
) -> ObjectResult<()> {
    let ty = type_record(c, key).await?;
    if ty.system {
        return Err(ObjectError::precondition("内置类型定义不能改写"));
    }
    validation::label(name, "类型名称", 200)?;
    validation::layout(layout)?;
    exec(
        c,
        "UPDATE object_types SET name=?1,layout_json=?2,version=version+1 WHERE key=?3",
        vec![s(name), s(&serde_json::to_string(layout)?), s(key)],
    )
    .await?;
    Ok(())
}
pub(super) async fn retire_type(c: &Connection, key: &str, retired: bool) -> ObjectResult<()> {
    let ty = type_record(c, key).await?;
    if ty.system {
        return Err(ObjectError::precondition("内置类型不能停用"));
    }
    if retired
        && (exists(
            c,
            "SELECT 1 FROM object_workflows WHERE type_key=?1",
            vec![s(key)],
        )
        .await?
            || exists(
                c,
                "SELECT 1 FROM object_rollups WHERE type_key=?1",
                vec![s(key)],
            )
            .await?)
    {
        return Err(ObjectError::precondition(
            "该类型存在行为定义，停用需要显式目录迁移",
        ));
    }
    exec(
        c,
        "UPDATE object_types SET retired=?1,version=version+1 WHERE key=?2",
        vec![n(i64::from(retired)), s(key)],
    )
    .await?;
    Ok(())
}
pub(super) async fn changed(c: &Connection) -> ObjectResult<()> {
    let version = catalog_version(c)
        .await?
        .checked_add(1)
        .ok_or_else(|| ObjectError::precondition("目录版本已耗尽"))?;
    exec(
        c,
        "UPDATE object_model_schema SET catalog_version=?1 WHERE singleton=1",
        vec![n(version)],
    )
    .await?;
    Ok(())
}
/// 关系、生命周期和聚合依赖的字段不能被目录写入绕过。
async fn protected_binding(c: &Connection, ty: &str, key: &str) -> ObjectResult<bool> {
    for d in workflow::definitions(c).await? {
        if d.type_key == ty
            && [
                &d.status_property,
                &d.starts_property,
                &d.ends_property,
                &d.started_property,
                &d.closed_property,
                &d.members_property,
            ]
            .iter()
            .any(|field| field.as_str() == key)
        {
            return Ok(true);
        }
        let (r, _) = relations::field(c, &d.type_key, &d.members_property).await?;
        if r.target_type.as_deref() == Some(ty) && d.member_status_property == key {
            return Ok(true);
        }
    }
    for d in rollup::definitions(c).await? {
        if d.type_key == ty && d.members_property == key {
            return Ok(true);
        }
        let (r, f) = relations::field(c, &d.type_key, &d.members_property).await?;
        if (if f {
            r.target_type
        } else {
            Some(r.source_type)
        })
        .as_deref()
            == Some(ty)
            && d.status_property == key
        {
            return Ok(true);
        }
    }
    Ok(false)
}
/// 校验整个目录，也检查所有内置定义未被导入覆盖。自定义引用与内置引用走相同检查。
pub(super) async fn validate_system(c: &Connection) -> ObjectResult<()> {
    let seed = seed_with_extensions(c).await?;
    let all = read(c).await?;
    for ty in &seed.types {
        let actual = type_record(c, &ty.definition.key).await?;
        if !actual.system
            || actual.retired
            || actual.capability != ty.capability
            || actual.definition != ty.definition
        {
            return Err(ObjectError::precondition("内置类型定义漂移"));
        }
    }
    for p in &seed.properties {
        let actual = property(c, &p.key).await?;
        let mut expected = p.clone();
        expected.options.sort_by(|a, b| a.key.cmp(&b.key));
        if !actual.system || actual.definition != expected {
            return Err(ObjectError::precondition("内置标量属性漂移"));
        }
    }
    for b in &seed.bindings {
        let actual = binding(c, &b.binding.type_key, &b.binding.property_key).await?;
        if actual.binding != b.binding
            || actual.writable != b.writable
            || actual.storage != b.storage
        {
            return Err(ObjectError::precondition("内置标量绑定漂移"));
        }
    }
    for expected in &seed.relations {
        if !all.relations.contains(expected) {
            return Err(ObjectError::precondition("内置关系定义漂移"));
        }
    }
    for expected in &seed.workflows {
        if !all.workflows.contains(expected) {
            return Err(ObjectError::precondition("内置生命周期定义漂移"));
        }
    }
    for expected in &seed.rollups {
        if !all.rollups.contains(expected) {
            return Err(ObjectError::precondition("内置聚合定义漂移"));
        }
    }
    let mut fields = std::collections::BTreeSet::new();
    for r in &all.relations {
        validation::label(&r.name, "关系名称", 200)?;
        if !seed.relations.iter().any(|x| x.key == r.key) {
            validation::custom_key(&r.key)?;
        }
        if r.acyclic && r.target_type.as_deref() != Some(r.source_type.as_str()) {
            return Err(ObjectError::precondition("无环定义端点不一致"));
        }
        let mut ends = vec![(
            &r.source_type,
            &r.source_property,
            r.target_type.clone(),
            r.source_cardinality,
        )];
        if let Some(key) = &r.target_property {
            let ty = r
                .target_type
                .as_ref()
                .ok_or_else(|| ObjectError::precondition("反向属性缺少目标类型"))?;
            ends.push((ty, key, Some(r.source_type.clone()), r.target_cardinality));
        }
        for (ty, key, target, cardinality) in ends {
            if !fields.insert(key.clone()) {
                return Err(ObjectError::precondition("关系属性被多个端点复用"));
            }
            let p = property(c, key).await?;
            let b = binding(c, ty, key).await?;
            if p.definition.kind != PropertyKind::Object
                || p.definition.cardinality != cardinality
                || p.definition.reference_type != target
                || !p.definition.options.is_empty()
                || b.binding.required
                || !b.binding.defaults.is_empty()
                || b.storage != "value"
                || !b.writable
            {
                return Err(ObjectError::precondition("关系字段和定义不一致"));
            }
        }
    }
    for ty in &all.types {
        if !seed
            .types
            .iter()
            .any(|t| t.definition.key == ty.definition.key)
        {
            validation::custom_key(&ty.definition.key)?;
            validation::layout(&ty.definition.layout)?;
            if ty.system || ty.capability != "plain" {
                return Err(ObjectError::precondition("自定义类型越过执行域边界"));
            }
        }
    }
    for p in &all.properties {
        validation::property_definition(&p.definition)?;
        if p.definition.kind == PropertyKind::Object && !fields.contains(&p.definition.key) {
            return Err(ObjectError::precondition("引用属性缺少唯一关系定义"));
        }
        if !seed.properties.iter().any(|v| v.key == p.definition.key)
            && !fields.contains(&p.definition.key)
        {
            validation::custom_key(&p.definition.key)?;
            if p.system {
                return Err(ObjectError::precondition("自定义属性错误声明 system"));
            }
        }
    }
    for b in &all.bindings {
        if !seed.bindings.iter().any(|v| {
            v.binding.type_key == b.binding.type_key
                && v.binding.property_key == b.binding.property_key
        }) && !fields.contains(&b.binding.property_key)
            && (b.storage != "value"
                || !b.writable
                || (b.binding.type_key == "task"
                    && (b.binding.required || !b.binding.defaults.is_empty())))
        {
            return Err(ObjectError::precondition("自定义绑定越过执行域边界"));
        }
        if fields.contains(&b.binding.property_key) {
            relations::field(c, &b.binding.type_key, &b.binding.property_key).await?;
        }
    }
    for d in &all.workflows {
        if !seed.workflows.iter().any(|x| x.key == d.key) {
            validation::custom_key(&d.key)?;
        }
        workflow::validate_definition(c, d).await?;
        if !exists(
            c,
            "SELECT 1 FROM object_workflows WHERE key=?1 AND type_key=?2 AND version>=1",
            vec![s(&d.key), s(&d.type_key)],
        )
        .await?
        {
            return Err(ObjectError::precondition("生命周期目录键与定义不一致"));
        }
    }
    for d in &all.rollups {
        if !seed.rollups.iter().any(|x| x.key == d.key) {
            validation::custom_key(&d.key)?;
        }
        rollup::validate_definition(c, d).await?;
        if !exists(
            c,
            "SELECT 1 FROM object_rollups WHERE key=?1 AND type_key=?2 AND version>=1",
            vec![s(&d.key), s(&d.type_key)],
        )
        .await?
        {
            return Err(ObjectError::precondition("聚合目录键与定义不一致"));
        }
    }
    Ok(())
}

/// 扩展的内建类型只在对应 lineage 标记存在后参与校验。
/// 保留原 SEED 字节，维持对象模型 v2 的指纹。
pub(super) async fn seed_with_extensions(c: &Connection) -> ObjectResult<Seed> {
    let mut seed: Seed = serde_json::from_str(SEED)?;
    if exists(
        c,
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='file_model_schema'",
        vec![],
    )
    .await?
    {
        let extra: Seed = serde_json::from_str(super::files::migration::SEED)?;
        seed.types.extend(extra.types);
        seed.properties.extend(extra.properties);
        seed.bindings.extend(extra.bindings);
        seed.relations.extend(extra.relations);
        seed.workflows.extend(extra.workflows);
        seed.rollups.extend(extra.rollups);
    }
    Ok(seed)
}
