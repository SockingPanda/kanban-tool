//! 项目配置与 canonical host vector wire contract。
//!
//! 这些 DTO 只拥有已解码的配置和 host HTTP shape。路径解析、provider 可用性和
//! dispatcher 策略仍由使用它们的 runtime adapter 负责。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectConfigInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<ProjectVectorConfigInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectVectorConfigInput {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

impl Default for ProjectVectorConfigInput {
    fn default() -> Self {
        Self {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "qwen3-embedding:0.6b".to_owned(),
            dimensions: 1024,
        }
    }
}

/// host maintenance API 报告的 canonical projection corpus identity。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProjectionCorpusMetadata {
    pub corpus_schema: String,
    pub corpus_fingerprint: String,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
}

/// CLI 选中的单个 `[workers.<profile>]` 小节的严格解码结构。
/// 其它 profile 小节的选择与 opaque 处理仍属于 adapter 关注点。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WorkerProfileInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_ttl_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_interval_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_success: Option<WorkerFinishPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<WorkerFinishPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WorkerFinishPolicy {
    Done,
    Review,
    Blocked,
    Ready,
}

/// canonical host vector 状态查询。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorStatusQuery {
    #[serde(default = "default_vector_board")]
    pub board: String,
}

fn default_vector_board() -> String {
    "default".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorConfigureRequest {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorProjectionRequest {
    #[serde(default = "default_vector_board")]
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorQuery {
    #[serde(default = "default_vector_board")]
    pub board: String,
    pub q: String,
    #[serde(default = "default_vector_limit")]
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polarity: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_vector: bool,
}

fn default_vector_limit() -> usize {
    20
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorChunkResult {
    pub id: String,
    pub entity_uri: Option<String>,
    pub source_kind: String,
    pub content: String,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    pub score: f32,
}

pub type VectorConfigureResponse = crate::DataEnvelope<VectorConfigureRequest>;
pub type VectorProjectionResponse = crate::DataEnvelope<crate::VectorStatus>;
pub type VectorQueryChunksResponse = crate::DataEnvelope<Vec<VectorChunkResult>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VectorLabelAtomResult {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

pub type VectorQueryLabelAtomsResponse = crate::DataEnvelope<Vec<VectorLabelAtomResult>>;
