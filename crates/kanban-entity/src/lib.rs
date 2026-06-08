use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Board,
    Column,
    Task,
    Run,
    Event,
    Comment,
    Attachment,
    Label,
    TaskLabel,
    Setting,
    Skill,
    Context,
    File,
    Model,
    Chunk,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Board => "board",
            Self::Column => "column",
            Self::Task => "task",
            Self::Run => "run",
            Self::Event => "event",
            Self::Comment => "comment",
            Self::Attachment => "attachment",
            Self::Label => "label",
            Self::TaskLabel => "task_label",
            Self::Setting => "setting",
            Self::Skill => "skill",
            Self::Context => "context",
            Self::File => "file",
            Self::Model => "model",
            Self::Chunk => "chunk",
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unsupported entity kind: {0}")]
pub struct EntityKindParseError(String);

impl FromStr for EntityKind {
    type Err = EntityKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "board" => Ok(Self::Board),
            "column" => Ok(Self::Column),
            "task" => Ok(Self::Task),
            "run" => Ok(Self::Run),
            "event" => Ok(Self::Event),
            "comment" => Ok(Self::Comment),
            "attachment" => Ok(Self::Attachment),
            "label" => Ok(Self::Label),
            "task_label" => Ok(Self::TaskLabel),
            "setting" => Ok(Self::Setting),
            "skill" => Ok(Self::Skill),
            "context" => Ok(Self::Context),
            "file" => Ok(Self::File),
            "model" => Ok(Self::Model),
            "chunk" => Ok(Self::Chunk),
            _ => Err(EntityKindParseError(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityUri(String);

impl EntityUri {
    pub fn new(value: impl Into<String>) -> Result<Self, EntityUriError> {
        let value = value.into();
        if !value.starts_with("kb://") {
            return Err(EntityUriError::MissingScheme(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn board(id: &str) -> Self {
        Self(format!("kb://board/{id}"))
    }

    pub fn column(id: &str) -> Self {
        Self(format!("kb://column/{id}"))
    }

    pub fn task(id: &str) -> Self {
        Self(format!("kb://task/{id}"))
    }

    pub fn run(id: &str) -> Self {
        Self(format!("kb://run/{id}"))
    }

    pub fn event(id: &str) -> Self {
        Self(format!("kb://event/{id}"))
    }

    pub fn comment(id: &str) -> Self {
        Self(format!("kb://comment/{id}"))
    }

    pub fn attachment(id: &str) -> Self {
        Self(format!("kb://artifact/{id}"))
    }

    pub fn label(id: &str) -> Self {
        Self(format!("kb://label/{id}"))
    }

    pub fn chunk(id: &str) -> Self {
        Self(format!("kb://chunk/{id}"))
    }

    pub fn setting(key: &str) -> Self {
        Self(format!("kb://setting/{key}"))
    }
}

impl fmt::Display for EntityUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EntityUriError {
    #[error("entity uri must start with kb://: {0}")]
    MissingScheme(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    BelongsToBoard,
    BelongsToTask,
    DependsOn,
    ProducedBy,
    GeneratedBy,
    ReferencesArtifact,
    RelatedTo,
    UsesSkill,
    UsesContext,
    DerivedFrom,
    Supersedes,
    SimilarTo,
    RequiresReview,
    WaitingForUser,
}

impl Predicate {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BelongsToBoard => "belongs_to_board",
            Self::BelongsToTask => "belongs_to_task",
            Self::DependsOn => "depends_on",
            Self::ProducedBy => "produced_by",
            Self::GeneratedBy => "generated_by",
            Self::ReferencesArtifact => "references_artifact",
            Self::RelatedTo => "related_to",
            Self::UsesSkill => "uses_skill",
            Self::UsesContext => "uses_context",
            Self::DerivedFrom => "derived_from",
            Self::Supersedes => "supersedes",
            Self::SimilarTo => "similar_to",
            Self::RequiresReview => "requires_review",
            Self::WaitingForUser => "waiting_for_user",
        }
    }

    pub fn seed(self) -> RelationPredicateSeed {
        match self {
            Self::BelongsToBoard => {
                RelationPredicateSeed::new(self, Some("task"), Some("board"), "sqlite")
            }
            Self::BelongsToTask => RelationPredicateSeed::new(self, None, Some("task"), "sqlite"),
            Self::DependsOn => {
                RelationPredicateSeed::new(self, Some("task"), Some("task"), "sqlite")
            }
            Self::ProducedBy => RelationPredicateSeed::new(self, None, Some("run"), "sqlite"),
            Self::GeneratedBy => RelationPredicateSeed::new(self, None, None, "graph"),
            Self::ReferencesArtifact => {
                RelationPredicateSeed::new(self, None, Some("attachment"), "graph")
            }
            Self::RelatedTo => RelationPredicateSeed::new(self, None, None, "graph"),
            Self::UsesSkill => RelationPredicateSeed::new(self, None, Some("skill"), "graph"),
            Self::UsesContext => RelationPredicateSeed::new(self, None, Some("context"), "graph"),
            Self::DerivedFrom => RelationPredicateSeed::new(self, None, None, "graph"),
            Self::Supersedes => RelationPredicateSeed::new(self, None, None, "graph"),
            Self::SimilarTo => RelationPredicateSeed::new(self, None, None, "lancedb_derived"),
            Self::RequiresReview => RelationPredicateSeed::new(self, Some("task"), None, "sqlite"),
            Self::WaitingForUser => RelationPredicateSeed::new(self, Some("task"), None, "graph"),
        }
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const PREDICATE_SEEDS: &[Predicate] = &[
    Predicate::BelongsToBoard,
    Predicate::BelongsToTask,
    Predicate::DependsOn,
    Predicate::ProducedBy,
    Predicate::GeneratedBy,
    Predicate::ReferencesArtifact,
    Predicate::RelatedTo,
    Predicate::UsesSkill,
    Predicate::UsesContext,
    Predicate::DerivedFrom,
    Predicate::Supersedes,
    Predicate::SimilarTo,
    Predicate::RequiresReview,
    Predicate::WaitingForUser,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationPredicateSeed {
    pub name: &'static str,
    pub domain_kind: Option<&'static str>,
    pub range_kind: Option<&'static str>,
    pub authoritative_store: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub uri: EntityUri,
    pub kind: EntityKind,
    pub source_table: String,
    pub source_id: String,
    pub board_id: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_table: Option<String>,
    pub source_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub authoritative_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub subject_uri: EntityUri,
    pub predicate: Predicate,
    pub object_uri: EntityUri,
    pub graph_uri: EntityUri,
    pub provenance: Provenance,
    pub metadata_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRef {
    pub uri: EntityUri,
    pub entity_uri: EntityUri,
    pub ordinal: i64,
    pub content_hash: Option<String>,
}

impl RelationPredicateSeed {
    pub fn new(
        predicate: Predicate,
        domain_kind: Option<&'static str>,
        range_kind: Option<&'static str>,
        authoritative_store: &'static str,
    ) -> Self {
        Self {
            name: predicate.as_str(),
            domain_kind,
            range_kind,
            authoritative_store,
        }
    }
}
