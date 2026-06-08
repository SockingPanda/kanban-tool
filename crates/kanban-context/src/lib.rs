use kanban_entity::EntityUri;
use kanban_search::SearchResults;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub lexical_limit: usize,
    pub graph_limit: usize,
    pub vector_limit: usize,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            lexical_limit: 5,
            graph_limit: 10,
            vector_limit: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextItem {
    pub entity_uri: EntityUri,
    pub source: String,
    pub score: Option<f64>,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPack {
    pub subject: EntityUri,
    pub policy: ContextPolicy,
    pub items: Vec<ContextItem>,
    pub degraded: Vec<String>,
}

pub trait ContextRetriever {
    fn retrieve(
        &self,
        subject: &EntityUri,
        policy: &ContextPolicy,
    ) -> Result<ContextPack, ContextError>;
}

#[derive(Debug, Clone)]
pub struct SearchContextRetriever {
    results: SearchResults,
}

impl SearchContextRetriever {
    pub fn new(results: SearchResults) -> Self {
        Self { results }
    }
}

impl ContextRetriever for SearchContextRetriever {
    fn retrieve(
        &self,
        subject: &EntityUri,
        policy: &ContextPolicy,
    ) -> Result<ContextPack, ContextError> {
        let items = self
            .results
            .hits
            .iter()
            .take(policy.lexical_limit)
            .map(|hit| ContextItem {
                entity_uri: EntityUri::task(&hit.task_id),
                source: "search".to_owned(),
                score: Some(hit.score),
                title: Some(format!("#{}", hit.seq)),
                snippet: hit.snippet.clone(),
            })
            .collect();
        Ok(ContextPack {
            subject: subject.clone(),
            policy: policy.clone(),
            items,
            degraded: vec!["graph_disabled".to_owned(), "vector_disabled".to_owned()],
        })
    }
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("context retrieval error: {0}")]
    Retrieval(String),
}
