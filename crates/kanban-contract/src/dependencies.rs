use serde::{Deserialize, Serialize};

macro_rules! task_path {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub task_id: String,
        }
    };
}
task_path!(ListDependenciesPath);
task_path!(AddDependencyPath);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RemoveDependencyPath {
    pub child_task_id: String,
    pub parent_task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiDependencyTask {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
    pub title: String,
    pub status: crate::ApiTaskStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiDependencyEdge {
    pub parent: ApiDependencyTask,
    pub child: ApiDependencyTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiDependencies {
    pub task: ApiDependencyTask,
    pub parents: Vec<crate::ApiTask>,
    pub children: Vec<crate::ApiTask>,
    pub edges: Vec<ApiDependencyEdge>,
}

macro_rules! dependency_response {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub data: ApiDependencies,
        }
    };
}
dependency_response!(ListDependenciesResponse);
dependency_response!(AddDependencyResponse);
dependency_response!(RemoveDependencyResponse);
