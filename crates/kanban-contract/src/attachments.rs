use serde::{Deserialize, Serialize};

use crate::{DataEnvelope, DeleteResponse};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListAttachmentsPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateAttachmentPath {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GetAttachmentPath {
    pub task_id: String,
    pub attachment_id: String,
}

pub type DeleteAttachmentPath = GetAttachmentPath;

/// 附件内容通过 download endpoint 的 bytes 返回；这个 DTO 仅代表 canonical metadata。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiAttachment {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub filename: String,
    pub rel_path: String,
    pub content_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateAttachmentRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub filename: String,
    #[serde(default)]
    pub content: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

pub type CreateAttachmentResponse = DataEnvelope<ApiAttachment>;
pub type ListAttachmentsResponse = DataEnvelope<Vec<ApiAttachment>>;
pub type DeleteAttachmentResponse = DeleteResponse;

/// Download success payload is raw bytes on HTTP; this alias is the logical
/// typed payload used by schema tooling and non-HTTP adapters.
pub type AttachmentDownloadResponse = Vec<u8>;
