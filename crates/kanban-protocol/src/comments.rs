use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CommentAuthorType {
    User,
    Agent,
}
impl CommentAuthorType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Note,
    Decision,
    Signal,
}
impl CommentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Decision => "decision",
            Self::Signal => "signal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListCommentsPath {
    pub task_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateCommentPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateCommentRequest {
    /// Entity-local retry key scoped to this task.
    pub idempotency_key: Option<String>,
    pub author: Option<String>,
    pub body: String,
    pub kind: Option<CommentKind>,
    pub author_type: Option<CommentAuthorType>,
    pub agent_type: Option<String>,
    #[cfg_attr(
        feature = "schema",
        schemars(with = "Option<crate::structured_metadata::JsonObject>")
    )]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiComment {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub author: String,
    pub author_type: CommentAuthorType,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata: crate::structured_metadata::JsonObject,
    pub created_at: i64,
}
fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListCommentsResponse {
    pub data: Vec<ApiComment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateCommentResponse {
    pub data: ApiComment,
}
