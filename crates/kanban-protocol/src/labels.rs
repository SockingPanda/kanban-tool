use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTaskLabelsPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AddTaskLabelPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RemoveTaskLabelPath {
    pub task_id: String,
    pub label_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AddTaskLabelRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<String>>,
    #[serde(default)]
    pub create_missing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

impl AddTaskLabelRequest {
    pub fn label_names(&self) -> Result<Vec<String>, &'static str> {
        match (&self.name, &self.names) {
            (Some(_), Some(_)) => Err("name 和 names 只能提供一个，不能同时提供"),
            (Some(name), None) => Ok(vec![name.clone()]),
            (None, Some(names)) if names.is_empty() => Err("names 至少要包含一个标签"),
            (None, Some(names)) => Ok(names.clone()),
            (None, None) => Err("必须提供 name 或 names"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTaskLabelsResponse {
    pub data: Vec<crate::ApiLabel>,
}

pub type AddTaskLabelResponse =
    crate::OptionalMetadataEnvelope<crate::ApiTask, crate::CreatedLabelsMeta<crate::ApiLabel>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RemoveTaskLabelResponse {
    pub data: crate::ApiTask,
}
