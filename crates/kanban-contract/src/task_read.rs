use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ApiTaskPriority, ApiTaskStatus};

pub const DEFAULT_TASK_READ_LIMIT: usize = 100;
pub const MAX_TASK_READ_LIMIT: usize = 1_000;
pub const MAX_TASK_READ_QUERY_BYTES: usize = 8_192;
pub const MAX_TASK_READ_STATUSES: usize = 9;
pub const MAX_TASK_READ_PRIORITIES: usize = 4;
pub const MAX_TASK_READ_PLAN_FILTERS: usize = 3;
pub const MAX_TASK_READ_LABELS: usize = 32;
pub const MAX_TASK_READ_Q_CHARS: usize = 1_024;
pub const MAX_TASK_READ_ASSIGNEE_CHARS: usize = 128;
pub const MAX_TASK_READ_LABEL_CHARS: usize = 128;
const TASK_READ_SCALAR_PARAMETER_COUNT: usize = 6;
pub const MAX_TASK_READ_QUERY_PAIRS: usize = MAX_TASK_READ_STATUSES
    + MAX_TASK_READ_PRIORITIES
    + MAX_TASK_READ_PLAN_FILTERS
    + MAX_TASK_READ_LABELS
    + TASK_READ_SCALAR_PARAMETER_COUNT;

macro_rules! string_wire_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[serde(rename = $wire)]
                $variant,
            )+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl FromStr for $name {
            type Err = ();

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(()),
                }
            }
        }
    };
}

string_wire_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
    pub enum TaskReadPlanFilter {
        PlanNeeded => "plan_needed",
        HasSteps => "has_steps",
        IncompleteRequiredSteps => "incomplete_required_steps",
    }
}

string_wire_enum! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
    pub enum TaskReadSort {
        Seq => "seq",
        SeqDesc => "-seq",
        Title => "title",
        TitleDesc => "-title",
        Status => "status",
        StatusDesc => "-status",
        #[default]
        Position => "position",
        PositionDesc => "-position",
        Priority => "priority",
        PriorityDesc => "-priority",
        Assignee => "assignee",
        AssigneeDesc => "-assignee",
        ScheduledAt => "scheduled_at",
        ScheduledAtDesc => "-scheduled_at",
        DueAt => "due_at",
        DueAtDesc => "-due_at",
        CreatedAt => "created_at",
        CreatedAtDesc => "-created_at",
        UpdatedAt => "updated_at",
        UpdatedAtDesc => "-updated_at",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct TaskReadLabel(
    #[cfg_attr(feature = "schema", schemars(length(min = 1, max = 128)))]
    #[cfg_attr(feature = "schema", schemars(regex(pattern = r"\P{White_Space}")))]
    String,
);

impl<'de> Deserialize<'de> for TaskReadLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            de::Error::custom(
                "task-read label must contain a non-whitespace character and be at most 128 Unicode characters",
            )
        })
    }
}

impl TaskReadLabel {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.chars().count() > MAX_TASK_READ_LABEL_CHARS {
            return None;
        }
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(Self(value.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksPath {
    #[cfg_attr(feature = "schema", schemars(length(min = 1)))]
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ListTasksQuery {
    #[cfg_attr(feature = "schema", schemars(length(max = 9)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub status: Vec<ApiTaskStatus>,
    #[cfg_attr(feature = "schema", schemars(length(max = 4)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub priority: Vec<ApiTaskPriority>,
    #[cfg_attr(feature = "schema", schemars(length(max = 32)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub label: Vec<TaskReadLabel>,
    #[cfg_attr(feature = "schema", schemars(length(max = 3)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub plan_filter: Vec<TaskReadPlanFilter>,
    #[cfg_attr(feature = "schema", schemars(length(max = 128)))]
    pub assignee: Option<String>,
    #[cfg_attr(feature = "schema", schemars(length(max = 1_024)))]
    pub q: Option<String>,
    pub include_archived: bool,
    #[cfg_attr(feature = "schema", schemars(range(max = 1_000)))]
    pub limit: usize,
    #[cfg_attr(
        feature = "schema",
        schemars(range(max = 9_223_372_036_854_775_807u64))
    )]
    pub offset: usize,
    pub sort: TaskReadSort,
}

impl Default for ListTasksQuery {
    fn default() -> Self {
        Self {
            status: Vec::new(),
            priority: Vec::new(),
            label: Vec::new(),
            plan_filter: Vec::new(),
            assignee: None,
            q: None,
            include_archived: false,
            limit: DEFAULT_TASK_READ_LIMIT,
            offset: 0,
            sort: TaskReadSort::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListTasksByStatusPath {
    #[cfg_attr(feature = "schema", schemars(length(min = 1)))]
    pub board: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ListTasksByStatusQuery {
    #[cfg_attr(feature = "schema", schemars(length(max = 9)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub status: Vec<ApiTaskStatus>,
    #[cfg_attr(feature = "schema", schemars(length(max = 4)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub priority: Vec<ApiTaskPriority>,
    #[cfg_attr(feature = "schema", schemars(length(max = 32)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub label: Vec<TaskReadLabel>,
    #[cfg_attr(feature = "schema", schemars(length(max = 3)))]
    #[cfg_attr(feature = "schema", schemars(extend("uniqueItems" = true)))]
    pub plan_filter: Vec<TaskReadPlanFilter>,
    #[cfg_attr(feature = "schema", schemars(length(max = 128)))]
    pub assignee: Option<String>,
    #[cfg_attr(feature = "schema", schemars(length(max = 1_024)))]
    pub q: Option<String>,
    pub include_archived: bool,
    #[cfg_attr(feature = "schema", schemars(range(max = 1_000)))]
    pub limit: usize,
    #[cfg_attr(
        feature = "schema",
        schemars(range(max = 9_223_372_036_854_775_807u64))
    )]
    pub offset: usize,
    pub sort: TaskReadSort,
}

impl Default for ListTasksByStatusQuery {
    fn default() -> Self {
        Self {
            status: Vec::new(),
            priority: Vec::new(),
            label: Vec::new(),
            plan_filter: Vec::new(),
            assignee: None,
            q: None,
            include_archived: false,
            limit: DEFAULT_TASK_READ_LIMIT,
            offset: 0,
            sort: TaskReadSort::default(),
        }
    }
}
