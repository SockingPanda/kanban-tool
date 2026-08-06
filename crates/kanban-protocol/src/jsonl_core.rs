//! 核心 portable JSONL record 的封闭 wire DTO。
//!
//! 这些类型拥有公开的 `{"type", "data"}` shape。SQLite 列名和 JSON-in-TEXT 存储
//! 仍属于 `kanban-sqlite` portable adapter 的私有实现。

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{ApiTaskPriority, CommentAuthorType, CommentKind};

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

macro_rules! jsonl_roots {
    ($type_name:ident, $variant:ident, $wire_name:literal, $data:ident, $input:ident, $output:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        pub enum $type_name {
            #[serde(rename = $wire_name)]
            $variant,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $input {
            #[serde(rename = "type")]
            pub record_type: $type_name,
            pub data: $data,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $output {
            #[serde(rename = "type")]
            pub record_type: $type_name,
            pub data: $data,
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PortableTaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PortableRunStatus {
    Running,
    Succeeded,
    Failed,
    Canceled,
    Expired,
}

/// 无 inhabitant 的值让必需的可空字段只能接受 JSON null。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PortableNever {}

/// 仅供 schema 使用的标记，表示 key 必须存在但可以包含 JSON null。
///
/// schemars 会把裸 `Option<T>` 视为可省略，而 `#[schemars(required)]` 会有意移除
/// null 分支。这个透明的非 Option wrapper 保留 wire contract 的两部分。
#[cfg(feature = "schema")]
#[derive(schemars::JsonSchema)]
#[serde(transparent)]
struct RequiredNullableSchema<T>(Option<T>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoardJsonlData {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub archived_at: Option<i64>,
}
jsonl_roots!(
    BoardJsonlType,
    Board,
    "board",
    BoardJsonlData,
    BoardJsonlInput,
    BoardJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ColumnJsonlData {
    pub id: String,
    pub board_id: String,
    pub status: PortableTaskStatus,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}
jsonl_roots!(
    ColumnJsonlType,
    Column,
    "column",
    ColumnJsonlData,
    ColumnJsonlInput,
    ColumnJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskJsonlData {
    pub id: String,
    pub board_id: String,
    pub seq: i64,
    pub title: String,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub description: Option<String>,
    pub status: PortableTaskStatus,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub status_reason: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub assignee: Option<String>,
    pub priority: ApiTaskPriority,
    pub position: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub scheduled_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub due_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub started_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub completed_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub archived_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub claim_token: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub claim_owner: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub claim_expires_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub last_heartbeat_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub max_retries: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub result_summary: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<Value>"))]
    pub result: Option<Value>,
    pub metadata: Value,
    pub lock_version: i64,
}
jsonl_roots!(
    TaskJsonlType,
    Task,
    "task",
    TaskJsonlData,
    TaskJsonlInput,
    TaskJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DependencyJsonlData {
    pub board_id: String,
    pub parent_task_id: String,
    pub child_task_id: String,
    pub created_at: i64,
}
jsonl_roots!(
    DependencyJsonlType,
    Dependency,
    "dependency",
    DependencyJsonlData,
    DependencyJsonlInput,
    DependencyJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RunJsonlData {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: PortableRunStatus,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub worker_profile: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub worker_pid: Option<i64>,
    pub claim_token: String,
    pub claim_owner: String,
    pub claim_expires_at: i64,
    pub started_at: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub last_heartbeat_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub finished_at: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<i64>"))]
    pub exit_code: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub summary: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub error: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "RequiredNullableSchema<PortableNever>")
    )]
    pub log_path: Option<PortableNever>,
    pub metadata: Value,
}
jsonl_roots!(
    RunJsonlType,
    Run,
    "run",
    RunJsonlData,
    RunJsonlInput,
    RunJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommentJsonlData {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub author: String,
    pub author_type: CommentAuthorType,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata: BTreeMap<String, Value>,
    pub created_at: i64,
}
jsonl_roots!(
    CommentJsonlType,
    Comment,
    "comment",
    CommentJsonlData,
    CommentJsonlInput,
    CommentJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventJsonlData {
    pub id: i64,
    pub event_id: String,
    pub board_id: String,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub task_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub run_id: Option<String>,
    pub kind: String,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub actor: Option<String>,
    pub payload: Value,
    pub created_at: i64,
}
jsonl_roots!(
    EventJsonlType,
    Event,
    "event",
    EventJsonlData,
    EventJsonlInput,
    EventJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttachmentJsonlData {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub filename: String,
    pub rel_path: String,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub content_type: Option<String>,
    pub size_bytes: i64,
    #[serde(deserialize_with = "required_nullable")]
    #[cfg_attr(feature = "schema", schemars(with = "RequiredNullableSchema<String>"))]
    pub sha256: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}
jsonl_roots!(
    AttachmentJsonlType,
    Attachment,
    "attachment",
    AttachmentJsonlData,
    AttachmentJsonlInput,
    AttachmentJsonlOutput
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskLabelJsonlData {
    pub board_id: String,
    pub task_id: String,
    pub label_id: String,
    pub created_at: i64,
}
jsonl_roots!(
    TaskLabelJsonlType,
    TaskLabel,
    "task_label",
    TaskLabelJsonlData,
    TaskLabelJsonlInput,
    TaskLabelJsonlOutput
);

#[cfg(all(test, feature = "schema"))]
mod tests {
    use super::{CommentJsonlInput, TaskJsonlInput};

    #[test]
    fn task_priority_schema_is_bounded_to_canonical_p0_through_p3() {
        let schema = serde_json::to_value(schemars::schema_for!(TaskJsonlInput))
            .expect("serialize task JSONL schema");
        let text = schema.to_string();
        assert!(text.contains(r#""minimum":0"#), "{text}");
        assert!(text.contains(r#""maximum":3"#), "{text}");
    }

    #[test]
    fn comment_author_and_kind_schema_vocabulary_is_closed() {
        let schema = serde_json::to_value(schemars::schema_for!(CommentJsonlInput))
            .expect("serialize comment JSONL schema");
        let text = schema.to_string();
        for value in ["user", "agent", "note", "decision", "signal"] {
            assert!(text.contains(&format!(r#""{value}""#)), "{text}");
        }
        for legacy in ["system", "human", "worker", "text"] {
            assert!(!text.contains(&format!(r#""{legacy}""#)), "{text}");
        }
    }
}
