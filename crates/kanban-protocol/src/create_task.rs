use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ApiTask;

const fn default_priority() -> i64 {
    3
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateTaskPath {
    pub board: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiCreateTaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateTaskRequest {
    /// 可选的 client 生成 typed ID；省略时由 host 生成。
    pub task_id: Option<String>,
    /// 作用域限定在所选看板上的 entity-local 重试 key。
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<ApiCreateTaskStatus>,
    pub assignee: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateTaskResponse {
    pub data: ApiTask,
}
