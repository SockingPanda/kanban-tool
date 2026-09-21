//! 模块和迭代的具名 application 能力；所有写入复用对象关系事务。
mod reads;
#[cfg(test)]
mod tests;
mod writes;

use super::{ObjectReceipt, ObjectVersion, Progress};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningKind {
    Module,
    Cycle,
}

impl PlanningKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Cycle => "cycle",
        }
    }
    pub(super) fn members(self) -> &'static str {
        match self {
            Self::Module => "module_members",
            Self::Cycle => "cycle_members",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningVersions {
    pub expected_version: Option<ObjectVersion>,
    /// 可选的旧、新关系端点双版本。省略的端点在同一写事务中读取。
    pub expected_versions: BTreeMap<String, ObjectVersion>,
    pub expected_catalog_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningCommand {
    pub board_id: String,
    pub kind: PlanningKind,
    pub actor: String,
    pub request_id: String,
    pub versions: PlanningVersions,
    pub mutation: PlanningMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PlanningMutation {
    Create {
        title: String,
        body: Option<String>,
        parent_id: Option<String>,
        starts_at: Option<i64>,
        ends_at: Option<i64>,
    },
    Update {
        id: String,
        title: Option<String>,
        body: Option<Option<String>>,
        parent_id: Option<Option<String>>,
        starts_at: Option<i64>,
        ends_at: Option<i64>,
    },
    Archive {
        id: String,
    },
    Restore {
        id: String,
    },
    TaskAdd {
        id: String,
        task_id: String,
    },
    TaskRemove {
        id: String,
        task_id: String,
    },
    Start {
        id: String,
    },
    Close {
        id: String,
        carry_to: Option<String>,
    },
    Cancel {
        id: String,
    },
}

#[derive(Debug, Clone)]
pub struct PlanningRecord {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub body: Option<String>,
    pub parent_id: Option<String>,
    pub status: Option<String>,
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub started_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub version: ObjectVersion,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PlanningListOptions {
    pub q: Option<String>,
    pub parent_id: Option<String>,
    pub status: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
}
impl Default for PlanningListOptions {
    fn default() -> Self {
        Self {
            q: None,
            parent_id: None,
            status: None,
            include_archived: false,
            limit: 100,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanningPage {
    pub items: Vec<PlanningRecord>,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct PlanningMember {
    pub id: String,
    pub title: String,
    pub status: String,
    pub version: ObjectVersion,
    pub carried_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlanningMemberPage {
    pub items: Vec<PlanningMember>,
    pub total: usize,
    pub frozen: bool,
    pub captured_at: Option<i64>,
    pub version: ObjectVersion,
}

#[derive(Debug, Clone)]
pub struct PlanningProgress {
    pub object: PlanningRecord,
    pub progress: Progress,
    pub frozen: bool,
    pub captured_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PlanningResult {
    pub object: PlanningRecord,
    pub receipt: ObjectReceipt,
}
