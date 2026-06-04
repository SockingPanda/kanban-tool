use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardColumn {
    pub id: String,
    pub board_id: String,
    pub status: super::TaskStatus,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
}
