use crate::{ApiTask, ApiTaskStatus, DataEnvelope};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiTaskGraphNodeRole {
    Center,
    DependencyParent,
    DependencyChild,
    StepParent,
    StepChild,
    Active,
    Context,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApiTaskGraphEdgeKind {
    Dependency,
    Step,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskGraphNode {
    pub task: ApiTask,
    pub role: ApiTaskGraphNodeRole,
    pub context_only: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskGraphEdge {
    pub id: String,
    pub source_task_id: String,
    pub target_task_id: String,
    pub kind: ApiTaskGraphEdgeKind,
    pub required: bool,
    pub blocking: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskGraphMeta {
    pub depth: usize,
    pub context_depth: usize,
    pub generated_at: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub truncated: bool,
    pub active_statuses: Vec<ApiTaskStatus>,
    pub active_only: bool,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
    pub limit_nodes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskNeighborhood {
    pub center_task_id: String,
    pub nodes: Vec<TaskGraphNode>,
    pub edges: Vec<TaskGraphEdge>,
    pub meta: TaskGraphMeta,
}
pub type TaskNeighborhoodResponse = DataEnvelope<TaskNeighborhood>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoardTaskMap {
    pub nodes: Vec<TaskGraphNode>,
    pub edges: Vec<TaskGraphEdge>,
    pub meta: TaskGraphMeta,
}
pub type BoardTaskMapResponse = DataEnvelope<BoardTaskMap>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TaskNeighborhoodPath {
    pub task_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct TaskNeighborhoodQuery {
    pub depth: usize,
    pub limit_nodes: usize,
    pub include_archived_context: bool,
}
impl Default for TaskNeighborhoodQuery {
    fn default() -> Self {
        Self {
            depth: 1,
            limit_nodes: 250,
            include_archived_context: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoardTaskMapPath {
    pub board: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct BoardTaskMapQuery {
    pub active_only: bool,
    pub context_depth: usize,
    pub limit_nodes: usize,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
}
impl Default for BoardTaskMapQuery {
    fn default() -> Self {
        Self {
            active_only: true,
            context_depth: 1,
            limit_nodes: 250,
            include_done_context: true,
            include_archived_context: false,
            hide_isolated: false,
        }
    }
}
