use serde::{Deserialize, Serialize};

/// The original filename and blob identity are immutable. Object.title is a separate display title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub board_id: String,
    pub owner_id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub object_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileUploadSpec {
    pub board_id: String,
    pub owner_id: String,
    pub file_id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub actor: String,
}

pub struct FileDownload {
    pub info: FileInfo,
    pub file: std::fs::File,
}

pub(crate) struct StoredFile {
    pub info: FileInfo,
    pub storage_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePage {
    pub items: Vec<FileInfo>,
    pub next_id: Option<String>,
}
