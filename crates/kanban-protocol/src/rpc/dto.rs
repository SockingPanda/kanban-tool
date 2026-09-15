//! RPC 补齐的具名 wire DTO；不承载 persistence 或业务执行规则。

use serde::{Deserialize, Serialize};

/// 下载内容与 canonical 附件元数据一起传输。
#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentDownload {
    pub attachment: crate::ApiAttachment,
    pub content: Vec<u8>,
}

impl From<(crate::ApiAttachment, Vec<u8>)> for AttachmentDownload {
    fn from((attachment, content): (crate::ApiAttachment, Vec<u8>)) -> Self {
        Self {
            attachment,
            content,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyQualityQuery {
    #[serde(default = "default_quality_sample_limit")]
    pub sample_limit: usize,
}

fn default_quality_sample_limit() -> usize {
    20
}

/// task 路由的 board 是可选隔离校验条件，不替代 canonical task identity。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLabelBoardQuery {
    pub board: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLabelProposalQuery {
    pub board: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskLabelSuggestionQuery {
    pub board: Option<String>,
    #[serde(default = "default_suggestion_limit")]
    pub limit: usize,
    #[serde(default = "default_candidate_limit")]
    pub candidate_limit: usize,
    #[serde(default = "default_atom_limit")]
    pub atom_limit: usize,
    #[serde(default = "default_max_selected_labels")]
    pub max_selected_labels: usize,
    #[serde(default = "default_min_score")]
    pub min_score: f32,
}

fn default_suggestion_limit() -> usize {
    5
}
fn default_candidate_limit() -> usize {
    32
}
fn default_atom_limit() -> usize {
    80
}
fn default_max_selected_labels() -> usize {
    4
}
fn default_min_score() -> f32 {
    0.15
}

impl From<TaskLabelSuggestionQuery> for crate::LabelSuggestionQuery {
    fn from(query: TaskLabelSuggestionQuery) -> Self {
        Self {
            limit: query.limit,
            candidate_limit: query.candidate_limit,
            atom_limit: query.atom_limit,
            max_selected_labels: query.max_selected_labels,
            min_score: query.min_score,
        }
    }
}

impl From<crate::LabelSuggestionQuery> for TaskLabelSuggestionQuery {
    fn from(query: crate::LabelSuggestionQuery) -> Self {
        Self {
            board: None,
            limit: query.limit,
            candidate_limit: query.candidate_limit,
            atom_limit: query.atom_limit,
            max_selected_labels: query.max_selected_labels,
            min_score: query.min_score,
        }
    }
}

/// 原 atom-index 降级查询的固定结果字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelAtomIndexHit {
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
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelAtomIndexQueryData {
    pub data: Vec<LabelAtomIndexHit>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

pub type LabelAtomIndexQueryResponse = crate::DataEnvelope<LabelAtomIndexQueryData>;
