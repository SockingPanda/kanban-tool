//! 通用类型、属性、对象及领域动作的进程内契约。此模块不定义网络协议。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    Text,
    Number,
    Boolean,
    Date,
    Object,
    Select,
}
impl PropertyKind {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Object => "object",
            Self::Select => "select",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    One,
    Many,
}
impl Cardinality {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Many => "many",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PropertyValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    Date(i64),
    Object(String),
    Select(String),
}
impl PropertyValue {
    pub(crate) fn kind(&self) -> PropertyKind {
        match self {
            Self::Text(_) => PropertyKind::Text,
            Self::Number(_) => PropertyKind::Number,
            Self::Boolean(_) => PropertyKind::Boolean,
            Self::Date(_) => PropertyKind::Date,
            Self::Object(_) => PropertyKind::Object,
            Self::Select(_) => PropertyKind::Select,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectOption {
    pub key: String,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyDefinition {
    pub key: String,
    pub name: String,
    pub kind: PropertyKind,
    pub cardinality: Cardinality,
    pub reference_type: Option<String>,
    #[serde(default)]
    pub options: Vec<SelectOption>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeDefinition {
    pub key: String,
    pub name: String,
    /// 只保存展示元数据。服务不执行这里的内容。
    #[serde(default = "empty_object")]
    pub layout: serde_json::Value,
}
fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyBinding {
    pub type_key: String,
    pub property_key: String,
    pub required: bool,
    pub position: i64,
    #[serde(default)]
    pub defaults: Vec<PropertyValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRecord {
    pub definition: TypeDefinition,
    pub version: i64,
    pub retired: bool,
    /// plain 为通用对象，task 为既有执行域适配器。模块和迭代均为 plain。
    pub capability: String,
    pub system: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyRecord {
    pub definition: PropertyDefinition,
    pub version: i64,
    pub system: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingRecord {
    pub binding: PropertyBinding,
    /// value / task.status / task.priority / task.assignee / task.due_at。
    pub storage: String,
    pub writable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectCatalog {
    pub version: i64,
    pub types: Vec<TypeRecord>,
    pub properties: Vec<PropertyRecord>,
    pub bindings: Vec<BindingRecord>,
    pub relations: Vec<RelationDefinition>,
    pub workflows: Vec<WorkflowDefinition>,
    pub rollups: Vec<RollupDefinition>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectVersion {
    pub object: i64,
    /// Task capability 的 lock_version。原任务路径更新时不用重复写对象表。
    pub source: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTarget {
    pub id: String,
    pub expected: ObjectVersion,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRecord {
    pub id: String,
    pub board_id: String,
    pub type_key: String,
    pub title: String,
    pub body: Option<String>,
    pub version: ObjectVersion,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    /// 标量缺失 key 表示没有槽位，空数组表示显式清空。关系字段总是返回数组，空数组表示没有边。
    pub properties: BTreeMap<String, Vec<PropertyValue>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyEdit {
    Set {
        property_key: String,
        values: Vec<PropertyValue>,
    },
    Unset {
        property_key: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectCreate {
    pub type_key: String,
    pub title: String,
    pub body: Option<String>,
    #[serde(default)]
    pub properties: BTreeMap<String, Vec<PropertyValue>>,
    /// 不提供时使用当前目录。提供时拒绝基于过期字段定义创建对象。
    pub expected_catalog_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectPatch {
    pub target: ObjectTarget,
    pub title: Option<String>,
    /// 省略不修改。SetBody 动作可设置文本或清空正文。
    #[serde(default)]
    pub edits: Vec<PropertyEdit>,
    pub expected_catalog_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectMutation {
    DefineType {
        definition: TypeDefinition,
        expected_catalog_version: i64,
    },
    DefineProperty {
        definition: PropertyDefinition,
        expected_catalog_version: i64,
    },
    BindProperty {
        binding: PropertyBinding,
        expected_catalog_version: i64,
    },
    UnbindProperty {
        type_key: String,
        property_key: String,
        expected_catalog_version: i64,
    },
    RenameType {
        key: String,
        name: String,
        layout: serde_json::Value,
        expected_catalog_version: i64,
    },
    RetireType {
        key: String,
        retired: bool,
        expected_catalog_version: i64,
    },
    Create {
        object: ObjectCreate,
    },
    Patch {
        patch: ObjectPatch,
    },
    SetBody {
        target: ObjectTarget,
        body: Option<String>,
    },
    SetArchived {
        target: ObjectTarget,
        archived: bool,
    },
    DefineRelation {
        definition: RelationDefinition,
        expected_catalog_version: i64,
    },
    DefineWorkflow {
        definition: WorkflowDefinition,
        expected_catalog_version: i64,
    },
    DefineRollup {
        definition: RollupDefinition,
        expected_catalog_version: i64,
    },
    /// 一组删除和添加在同一事务完成，支持原子换属。所有变动端点均需预期版本。
    ChangeRelations {
        change: RelationChange,
    },
    /// 任意关系端点的全量替换。source 与 inverse 使用相同命令。
    SetReferences {
        target: ObjectTarget,
        property_key: String,
        references: Vec<String>,
        expected: Vec<ObjectTarget>,
        expected_catalog_version: i64,
    },
    StartWorkflow {
        target: ObjectTarget,
    },
    CloseWorkflow {
        target: ObjectTarget,
        carry_to: Option<ObjectTarget>,
    },
    CancelWorkflow {
        target: ObjectTarget,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectCommand {
    pub board_id: String,
    pub actor: String,
    pub request_id: String,
    pub mutation: ObjectMutation,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectReceipt {
    pub request_id: String,
    pub ids: Vec<String>,
    pub event_sequence: i64,
    pub catalog_version: i64,
    pub replayed: bool,
    pub versions: BTreeMap<String, ObjectVersion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyFilter {
    Has {
        property_key: String,
    },
    IsEmpty {
        property_key: String,
    },
    IsMissing {
        property_key: String,
    },
    Equals {
        property_key: String,
        value: PropertyValue,
    },
    Contains {
        property_key: String,
        value: PropertyValue,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectQuery {
    pub board_id: String,
    pub type_key: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub filters: Vec<PropertyFilter>,
    pub include_archived: bool,
    pub limit: u32,
    pub after: Option<ObjectCursor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectCursor {
    pub query_hash: String,
    pub after_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPage {
    pub items: Vec<ObjectRecord>,
    pub next: Option<ObjectCursor>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub total: i64,
    pub done: i64,
    pub archived: i64,
    pub blocked: i64,
    /// done / (total - archived)，无可计算的任务时为 None。
    pub completion_ratio: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningOverview {
    pub object: ObjectRecord,
    pub progress: Progress,
    pub frozen: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub id: String,
    pub object_id: String,
    pub kind: String,
    pub body: serde_json::Value,
    pub created_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDiagnostic {
    pub code: String,
    pub object_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedMember {
    pub id: String,
    pub title: String,
    pub status: String,
    pub version: ObjectVersion,
    pub carried_to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowClosure {
    pub object: ObjectRecord,
    pub members: Vec<ClosedMember>,
    pub captured_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectAuditEntry {
    pub sequence: i64,
    pub kind: String,
    pub actor: Option<String>,
    pub payload: serde_json::Value,
    pub created_at: i64,
}

/// 一种关系的两个可命名端点。cardinality 表示该端对象能连接的对端数量。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationDefinition {
    pub key: String,
    pub name: String,
    pub source_type: String,
    pub target_type: Option<String>,
    pub source_property: String,
    pub target_property: Option<String>,
    pub source_cardinality: Cardinality,
    pub target_cardinality: Cardinality,
    pub acyclic: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationLink {
    pub relation_key: String,
    pub source_id: String,
    pub target_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationChange {
    pub expected_catalog_version: i64,
    pub expected: Vec<ObjectTarget>,
    #[serde(default)]
    pub remove: Vec<RelationLink>,
    #[serde(default)]
    pub add: Vec<RelationLink>,
}
/// 固定的生命周期操作器，字段映射和状态值来自定义，类型名不参与分支。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    pub key: String,
    pub type_key: String,
    pub status_property: String,
    pub starts_property: String,
    pub ends_property: String,
    pub started_property: String,
    pub closed_property: String,
    pub planned_value: String,
    pub active_value: String,
    pub completed_value: String,
    pub cancelled_value: String,
    pub members_property: String,
    pub member_status_property: String,
    pub done_values: Vec<String>,
    pub excluded_values: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollupDefinition {
    pub key: String,
    pub type_key: String,
    pub members_property: String,
    pub status_property: String,
    pub done_values: Vec<String>,
    pub excluded_values: Vec<String>,
    pub blocked_values: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceQuery {
    pub board_id: String,
    pub object_id: String,
    pub property_key: String,
    pub limit: u32,
    pub after_id: Option<String>,
    pub expected_version: ObjectVersion,
    pub expected_catalog_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencePage {
    pub items: Vec<ObjectRecord>,
    pub next_id: Option<String>,
}
