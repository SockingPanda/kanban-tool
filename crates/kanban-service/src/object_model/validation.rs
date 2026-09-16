use super::{catalog, error::*, model::*, store::*};
use std::collections::BTreeSet;
use turso::Connection;

pub(super) fn label(v: &str, field: &str, max: usize) -> ObjectResult<()> {
    if v.trim().is_empty() || v.chars().count() > max || v.contains('\0') {
        return Err(ObjectError::invalid(format!(
            "{field} 不能为空、不能包含 NUL，且最多 {max} 个字符"
        )));
    }
    Ok(())
}
pub(super) fn custom_key(v: &str) -> ObjectResult<()> {
    if !v.starts_with("custom.")
        || v.len() <= 7
        || v.len() > 100
        || !v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(ObjectError::invalid(
            "自定义 key 必须以 custom. 开头，使用 ASCII 字母、数字、点、下划线或连字符，长度不超过 100",
        ));
    }
    Ok(())
}
pub(super) fn layout(v: &serde_json::Value) -> ObjectResult<()> {
    if !v.is_object() || serde_json::to_vec(v)?.len() > 16_384 {
        return Err(ObjectError::invalid(
            "layout 必须是 16 KiB 以内的 JSON 对象",
        ));
    }
    Ok(())
}
pub(super) fn property_definition(p: &PropertyDefinition) -> ObjectResult<()> {
    label(&p.name, "属性名称", 200)?;
    if p.reference_type.is_some() && p.kind != PropertyKind::Object {
        return Err(ObjectError::invalid(
            "只有对象引用属性可以指定 reference_type",
        ));
    }
    if !p.options.is_empty() && p.kind != PropertyKind::Select {
        return Err(ObjectError::invalid("只有选择属性可以定义 options"));
    }
    if p.kind == PropertyKind::Select && (p.options.is_empty() || p.options.len() > 256) {
        return Err(ObjectError::invalid("选择属性需要 1 到 256 个选项"));
    }
    let mut keys = BTreeSet::new();
    for opt in &p.options {
        label(&opt.key, "选项 key", 100)?;
        label(&opt.name, "选项名称", 200)?;
        if !keys.insert(&opt.key) {
            return Err(ObjectError::invalid("选项 key 重复"));
        }
    }
    Ok(())
}
pub(super) fn scalar_values(
    p: &PropertyDefinition,
    values: &[PropertyValue],
    required: bool,
) -> ObjectResult<()> {
    if values.len()
        > (if p.kind == PropertyKind::Object {
            super::relations::MAX_MEMBERS as usize
        } else {
            256
        })
        || (p.cardinality == Cardinality::One && values.len() > 1)
    {
        return Err(ObjectError::invalid(
            "属性值数量超过该字段的 cardinality 或项目上限",
        ));
    }
    if required && values.is_empty() {
        return Err(ObjectError::invalid(format!("属性 {} 必填", p.key)));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        if value.kind() != p.kind {
            return Err(ObjectError::invalid(format!(
                "属性 {} 的值类型不匹配",
                p.key
            )));
        }
        match value {
            PropertyValue::Text(v) if required && v.trim().is_empty() => {
                return Err(ObjectError::invalid("必填文本不能只包含空白"));
            }
            PropertyValue::Text(v) if v.len() > 65_536 || v.contains('\0') => {
                return Err(ObjectError::invalid("text 属性超过 64 KiB 或包含 NUL"));
            }
            PropertyValue::Number(v) if !v.is_finite() => {
                return Err(ObjectError::invalid("number 必须是有限数"));
            }
            PropertyValue::Object(v) => {
                label(v, "引用 ID", 128)?;
            }
            PropertyValue::Select(v) if !p.options.iter().any(|opt| opt.key == *v) => {
                return Err(ObjectError::invalid(format!(
                    "属性 {} 中不存在选项 {v}",
                    p.key
                )));
            }
            _ => {}
        }
        // Text 列表允许相同文本；对象/选项引用采用集合语义。
        if matches!(value, PropertyValue::Object(_) | PropertyValue::Select(_))
            && !unique.insert(serde_json::to_string(value)?)
        {
            return Err(ObjectError::invalid("重复对象引用或选择项"));
        }
    }
    Ok(())
}
pub(super) async fn values(
    c: &Connection,
    board: &str,
    p: &PropertyDefinition,
    v: &[PropertyValue],
    required: bool,
    allow_archived: bool,
) -> ObjectResult<()> {
    scalar_values(p, v, required)?;
    for value in v {
        if let PropertyValue::Object(id) = value {
            let object = get(c, board, id).await?;
            if p.reference_type
                .as_ref()
                .is_some_and(|ty| ty != &object.type_key)
            {
                return Err(ObjectError::invalid("引用目标的类型不匹配"));
            }
            if !allow_archived && object.archived_at.is_some() {
                return Err(ObjectError::precondition("不能新增指向已归档对象的引用"));
            }
        }
    }
    Ok(())
}
pub(super) async fn object(c: &Connection, o: &ObjectRecord) -> ObjectResult<()> {
    // 既有任务正文仍按 Task capability 的原规则验收，不给历史数据追加新的长度限制。
    let encoded = if o.type_key == "task" {
        serde_json::to_vec(&o.properties)?
    } else {
        label(&o.title, "标题", 200)?;
        body(o.body.as_deref())?;
        serde_json::to_vec(o)?
    };
    if encoded.len() > 2_097_152 {
        return Err(ObjectError::invalid("对象字段编码超过 2 MiB"));
    }
    for b in catalog::bindings(c, &o.type_key).await? {
        if b.storage != "value" {
            continue;
        }
        let p = catalog::property(c, &b.binding.property_key).await?;
        let values = o
            .properties
            .get(&b.binding.property_key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        // 已有引用允许保留已归档对象；新建引用在命令边界单独拒绝。
        self::values(
            c,
            &o.board_id,
            &p.definition,
            values,
            b.binding.required,
            true,
        )
        .await?;
    }
    super::relations::validate_object(c, o).await?;
    super::workflow::validate_object(c, o).await?;
    Ok(())
}
pub(super) fn body(v: Option<&str>) -> ObjectResult<()> {
    if v.is_some_and(|v| v.len() > 1_048_576 || v.contains('\0')) {
        return Err(ObjectError::invalid("正文超过 1 MiB 或包含 NUL"));
    }
    Ok(())
}
