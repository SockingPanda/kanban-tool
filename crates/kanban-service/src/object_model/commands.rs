//! 普通对象只走类型目录、属性校验和通用写入。领域行为有固定的服务函数。
use super::{catalog, error::*, model::*, relations, rollup, store::*, validation, workflow};
use std::collections::BTreeSet;
use turso::Connection;

pub(super) async fn apply(
    c: &Connection,
    request: &ObjectCommand,
    now: i64,
) -> ObjectResult<Vec<String>> {
    let board = &request.board_id;
    match &request.mutation {
        ObjectMutation::DefineType {
            definition,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::define_type(c, definition).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::DefineProperty {
            definition,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::define_property(c, definition).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::BindProperty {
            binding,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::bind(c, binding).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::UnbindProperty {
            type_key,
            property_key,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::unbind(c, type_key, property_key).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::RenameType {
            key,
            name,
            layout,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::rename_type(c, key, name, layout).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::RetireType {
            key,
            retired,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            catalog::retire_type(c, key, *retired).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::Create { object } => Ok(vec![create(c, board, object, now).await?]),
        ObjectMutation::Patch { patch } => {
            patch_object(c, board, patch, now).await?;
            Ok(vec![patch.target.id.clone()])
        }
        ObjectMutation::SetBody {
            target: expected,
            body,
        } => {
            let object = target(c, board, expected, true).await?;
            workflow::ensure_editable(c, &object).await?;
            if object.type_key == "task" {
                return Err(ObjectError::precondition("任务正文由 update_task 管理"));
            }
            validation::body(body.as_deref())?;
            exec(
                c,
                "UPDATE objects SET body=?1 WHERE id=?2",
                vec![os(body.as_deref()), s(&object.id)],
            )
            .await?;
            validation::object(c, &get(c, board, &object.id).await?).await?;
            bump(c, &object, now).await?;
            Ok(vec![object.id])
        }
        ObjectMutation::SetArchived {
            target: expected,
            archived,
        } => {
            let object = target(c, board, expected, false).await?;
            archive(c, &object, *archived, now).await?;
            Ok(vec![object.id])
        }
        ObjectMutation::DefineRelation {
            definition,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            relations::define(c, definition, false).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::DefineWorkflow {
            definition,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            workflow::define(c, definition, false).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::DefineRollup {
            definition,
            expected_catalog_version,
        } => {
            expect_catalog(c, *expected_catalog_version).await?;
            rollup::define(c, definition, false).await?;
            catalog::changed(c).await?;
            Ok(vec![])
        }
        ObjectMutation::ChangeRelations { change } => {
            relations::change(c, board, change, now).await
        }
        ObjectMutation::SetReferences {
            target,
            property_key,
            references,
            expected,
            expected_catalog_version,
        } => {
            relations::set_references(
                c,
                board,
                target,
                property_key,
                references,
                expected,
                *expected_catalog_version,
                now,
            )
            .await
        }
        ObjectMutation::StartWorkflow { target } => workflow::start(c, board, target, now).await,
        ObjectMutation::CloseWorkflow { target, carry_to } => {
            workflow::close(c, board, target, carry_to.as_ref(), false, now).await
        }
        ObjectMutation::CancelWorkflow { target } => {
            workflow::close(c, board, target, None, true, now).await
        }
    }
}
pub(super) async fn create(
    c: &Connection,
    board: &str,
    input: &ObjectCreate,
    now: i64,
) -> ObjectResult<String> {
    if let Some(v) = input.expected_catalog_version {
        expect_catalog(c, v).await?;
    }
    let ty = catalog::type_record(c, &input.type_key).await?;
    if ty.retired {
        return Err(ObjectError::precondition("类型已停用"));
    }
    if input.type_key == "file" {
        return Err(ObjectError::precondition("file.create_requires_upload"));
    }
    if ty.capability == "task" {
        return Err(ObjectError::precondition(
            "创建任务必须调用现有 create_task，任务执行计划和事件由同一事务产生",
        ));
    }
    validation::label(&input.title, "标题", 200)?;
    validation::body(input.body.as_deref())?;
    if input.properties.len() > 128 {
        return Err(ObjectError::invalid("对象属性超过 128 项"));
    }
    let id = format!("obj_{}", ulid::Ulid::new());
    exec(c,"INSERT INTO objects(id,board_id,type_key,task_id,title,body,version,created_at,updated_at,archived_at) VALUES (?1,?2,?3,NULL,?4,?5,1,?6,?6,NULL)",vec![s(&id),s(board),s(&input.type_key),s(&input.title),os(input.body.as_deref()),n(now)]).await?;
    let object = get(c, board, &id).await?;
    for binding in catalog::bindings(c, &input.type_key).await? {
        if binding.storage == "value" && !binding.binding.defaults.is_empty() {
            let prop = catalog::property(c, &binding.binding.property_key).await?;
            validation::values(
                c,
                board,
                &prop.definition,
                &binding.binding.defaults,
                binding.binding.required,
                false,
            )
            .await?;
            set_values(c, &object, &prop.definition, &binding.binding.defaults).await?;
        }
    }
    for (key, values) in &input.properties {
        edit_value(c, &object, key, Some(values)).await?;
    }
    validation::object(c, &get(c, board, &id).await?).await?;
    Ok(id)
}
async fn patch_object(
    c: &Connection,
    board: &str,
    patch: &ObjectPatch,
    now: i64,
) -> ObjectResult<()> {
    if let Some(v) = patch.expected_catalog_version {
        expect_catalog(c, v).await?;
    }
    let object = target(c, board, &patch.target, true).await?;
    workflow::ensure_editable(c, &object).await?;
    if patch.title.is_none() && patch.edits.is_empty() {
        return Err(ObjectError::invalid("patch 没有修改项"));
    }
    if patch.edits.len() > 128 {
        return Err(ObjectError::invalid("patch 超过 128 项"));
    }
    let mut seen = BTreeSet::new();
    for edit in &patch.edits {
        let (key, value) = match edit {
            PropertyEdit::Set {
                property_key,
                values,
            } => (property_key, Some(values.as_slice())),
            PropertyEdit::Unset { property_key } => (property_key, None),
        };
        if !seen.insert(key) {
            return Err(ObjectError::invalid("一个 patch 中不能重复修改同一属性"));
        }
        edit_value(c, &object, key, value).await?;
    }
    if let Some(title) = &patch.title {
        if object.type_key == "task" {
            return Err(ObjectError::precondition("任务标题由 update_task 管理"));
        }
        validation::label(title, "标题", 200)?;
        exec(
            c,
            "UPDATE objects SET title=?1 WHERE id=?2",
            vec![s(title), s(&object.id)],
        )
        .await?;
    }
    validation::object(c, &get(c, board, &object.id).await?).await?;
    bump(c, &object, now).await
}
async fn edit_value(
    c: &Connection,
    object: &ObjectRecord,
    key: &str,
    values: Option<&[PropertyValue]>,
) -> ObjectResult<()> {
    workflow::property_writable(c, &object.type_key, key).await?;
    let binding = catalog::binding(c, &object.type_key, key).await?;
    if !binding.writable || binding.storage != "value" {
        return Err(ObjectError::precondition(format!(
            "属性 {key} 由领域动作管理，不接受通用属性写入"
        )));
    }
    let prop = catalog::property(c, key).await?;
    if prop.definition.kind == PropertyKind::Object {
        return Err(ObjectError::precondition(
            "引用字段使用 SetReferences 或 ChangeRelations，需提供所有变动端点版本",
        ));
    }
    match values {
        Some(values) => {
            // 引用保留值可指向已归档对象，新加入的引用不能指向已归档对象。
            validation::values(
                c,
                &object.board_id,
                &prop.definition,
                values,
                binding.binding.required,
                true,
            )
            .await?;
            let old = object.properties.get(key).map(Vec::as_slice).unwrap_or(&[]);
            for value in values {
                if let PropertyValue::Object(id) = value
                    && !old.contains(value)
                    && get(c, &object.board_id, id).await?.archived_at.is_some()
                {
                    return Err(ObjectError::precondition("不能新增指向已归档对象的引用"));
                }
            }
            set_values(c, object, &prop.definition, values).await?;
        }
        None => {
            if binding.binding.required {
                return Err(ObjectError::invalid(format!("必填属性 {key} 不能 unset")));
            }
            exec(
                c,
                "DELETE FROM object_property_slots WHERE object_id=?1 AND property_key=?2",
                vec![s(&object.id), s(key)],
            )
            .await?;
        }
    }
    Ok(())
}
async fn archive(
    c: &Connection,
    object: &ObjectRecord,
    archived: bool,
    now: i64,
) -> ObjectResult<()> {
    if object.type_key == "task" {
        return Err(ObjectError::precondition(
            "任务归档和恢复必须使用现有任务服务",
        ));
    }
    if let Some(d) = workflow::for_type(c, &object.type_key).await?
        && one_date(object, &d.closed_property).is_none()
    {
        return Err(ObjectError::precondition("归档前先关闭生命周期"));
    }
    if archived {
        relations::ensure_archivable(c, object).await?;
    }
    exec(
        c,
        "UPDATE objects SET archived_at=?1 WHERE id=?2",
        vec![on(if archived { Some(now) } else { None }), s(&object.id)],
    )
    .await?;
    validation::object(c, &get(c, &object.board_id, &object.id).await?).await?;
    bump(c, object, now).await
}
