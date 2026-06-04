use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use strum::{Display, EnumIter};

use crate::KanbanError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TaskStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

impl TaskStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Triage => "triage",
            Self::Todo => "todo",
            Self::Scheduled => "scheduled",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Review => "review",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }

    pub const fn can_be_created(self) -> bool {
        matches!(
            self,
            Self::Triage | Self::Todo | Self::Scheduled | Self::Ready
        )
    }

    pub const fn is_claimable(self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl FromStr for TaskStatus {
    type Err = KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "triage" => Ok(Self::Triage),
            "todo" => Ok(Self::Todo),
            "scheduled" => Ok(Self::Scheduled),
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "blocked" => Ok(Self::Blocked),
            "review" => Ok(Self::Review),
            "done" => Ok(Self::Done),
            "archived" => Ok(Self::Archived),
            other => Err(KanbanError::InvalidStatus(other.to_owned())),
        }
    }
}

impl TryFrom<&str> for TaskStatus {
    type Error = KanbanError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl fmt::LowerHex for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_status_names() {
        assert_eq!(TaskStatus::try_from("triage").unwrap(), TaskStatus::Triage);
        assert_eq!(TaskStatus::try_from("ready").unwrap(), TaskStatus::Ready);
        assert_eq!(
            TaskStatus::try_from("archived").unwrap(),
            TaskStatus::Archived
        );
    }

    #[test]
    fn exposes_creation_and_claim_rules() {
        assert!(TaskStatus::Ready.can_be_created());
        assert!(!TaskStatus::Running.can_be_created());
        assert!(TaskStatus::Ready.is_claimable());
        assert!(!TaskStatus::Review.is_claimable());
    }
}
