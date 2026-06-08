use kanban_entity::EntityUri;
use kanban_graph::GraphStoreStatus;
use kanban_search::SearchResults;
use kanban_vector::VectorStoreStatus;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub lexical_limit: usize,
    pub graph_limit: usize,
    pub vector_limit: usize,
    pub max_items: usize,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            lexical_limit: 5,
            graph_limit: 10,
            vector_limit: 5,
            max_items: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextItem {
    pub entity_uri: EntityUri,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBrokerInput {
    pub subject_item: ContextItem,
    pub lexical: SearchResults,
    pub graph: Vec<ContextItem>,
    pub vector: Vec<ContextItem>,
    pub graph_status: GraphStoreStatus,
    pub vector_status: VectorStoreStatus,
}

pub fn build_context_pack(
    subject: EntityUri,
    policy: ContextPolicy,
    input: ContextBrokerInput,
) -> ContextPack {
    let mut degraded = Vec::new();
    if input.lexical.meta.backend == "sqlite" {
        degraded.push("search_sqlite_fallback".to_owned());
    }
    if input.lexical.meta.stale {
        degraded.push("search_stale".to_owned());
    }
    if !input.graph_status.enabled {
        degraded.push("graph_disabled".to_owned());
    }
    if !input.vector_status.enabled {
        degraded.push("vector_disabled".to_owned());
    }

    let lexical_items = input
        .lexical
        .hits
        .iter()
        .take(policy.lexical_limit)
        .map(|hit| ContextItem {
            entity_uri: EntityUri::task(&hit.task_id),
            source: "search".to_owned(),
            provenance: vec![format!("search:{}", input.lexical.meta.backend)],
            score: Some(hit.score),
            title: Some(format!("#{}", hit.seq)),
            snippet: hit.snippet.clone(),
        });

    let graph_items = input.graph.into_iter().take(policy.graph_limit);
    let vector_items = input.vector.into_iter().take(policy.vector_limit);

    let mut items = Vec::new();
    merge_item(&mut items, input.subject_item);
    for item in lexical_items.chain(graph_items).chain(vector_items) {
        merge_item(&mut items, item);
        if items.len() >= policy.max_items {
            break;
        }
    }

    ContextPack {
        subject,
        policy,
        items,
        degraded,
    }
}

fn merge_item(items: &mut Vec<ContextItem>, mut item: ContextItem) {
    if item.provenance.is_empty() {
        item.provenance.push(item.source.clone());
    }
    if let Some(existing) = items
        .iter_mut()
        .find(|existing| existing.entity_uri == item.entity_uri)
    {
        if !existing
            .provenance
            .iter()
            .any(|source| source == &item.source)
        {
            existing.provenance.push(item.source.clone());
        }
        for source in item.provenance {
            if !existing.provenance.iter().any(|value| value == &source) {
                existing.provenance.push(source);
            }
        }
        if existing.title.is_none() {
            existing.title = item.title;
        }
        if existing.snippet.is_none() {
            existing.snippet = item.snippet;
        }
        if existing.score.is_none() {
            existing.score = item.score;
        }
    } else {
        items.push(item);
    }
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
                provenance: vec!["search".to_owned()],
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

#[cfg(test)]
mod tests {
    use super::{ContextBrokerInput, ContextItem, ContextPolicy, build_context_pack};
    use kanban_entity::EntityUri;
    use kanban_graph::GraphStoreStatus;
    use kanban_search::{SearchHit, SearchMeta, SearchResults};
    use kanban_vector::VectorStoreStatus;

    fn status(enabled: bool, backend: &str) -> (GraphStoreStatus, VectorStoreStatus) {
        (
            GraphStoreStatus {
                backend: backend.to_owned(),
                enabled,
                message: String::new(),
            },
            VectorStoreStatus {
                backend: backend.to_owned(),
                enabled,
                message: String::new(),
            },
        )
    }

    #[test]
    fn broker_keeps_subject_first_and_dedupes_by_entity_uri() {
        let (graph_status, vector_status) = status(true, "test");
        let subject = EntityUri::task("t_subject");
        let pack = build_context_pack(
            subject.clone(),
            ContextPolicy {
                lexical_limit: 5,
                graph_limit: 5,
                vector_limit: 5,
                max_items: 10,
            },
            ContextBrokerInput {
                subject_item: ContextItem {
                    entity_uri: subject.clone(),
                    source: "subject".to_owned(),
                    provenance: vec![],
                    score: None,
                    title: Some("Subject".to_owned()),
                    snippet: None,
                },
                lexical: SearchResults {
                    hits: vec![
                        SearchHit {
                            task_id: "t_subject".to_owned(),
                            seq: 1,
                            score: 100.0,
                            snippet: Some("duplicate".to_owned()),
                        },
                        SearchHit {
                            task_id: "t_lexical".to_owned(),
                            seq: 2,
                            score: 50.0,
                            snippet: None,
                        },
                    ],
                    meta: SearchMeta {
                        backend: "tantivy".to_owned(),
                        stale: false,
                        index_version: None,
                        last_event_id: None,
                        index_lag_events: None,
                    },
                },
                graph: vec![ContextItem {
                    entity_uri: EntityUri::task("t_graph"),
                    source: "graph".to_owned(),
                    provenance: vec!["graph:depends_on".to_owned()],
                    score: None,
                    title: None,
                    snippet: Some("depends_on".to_owned()),
                }],
                vector: vec![ContextItem {
                    entity_uri: EntityUri::task("t_lexical"),
                    source: "vector".to_owned(),
                    provenance: vec!["vector:lancedb".to_owned()],
                    score: Some(0.9),
                    title: Some("semantic duplicate".to_owned()),
                    snippet: None,
                }],
                graph_status,
                vector_status,
            },
        );

        assert_eq!(pack.items.len(), 3);
        assert_eq!(pack.items[0].entity_uri, subject);
        assert_eq!(pack.items[1].entity_uri, EntityUri::task("t_lexical"));
        assert!(pack.items[1].provenance.iter().any(|p| p == "vector"));
        assert_eq!(pack.items[2].entity_uri, EntityUri::task("t_graph"));
        assert!(pack.degraded.is_empty());
    }

    #[test]
    fn broker_reports_disabled_stores_and_applies_budget() {
        let (graph_status, vector_status) = status(false, "disabled");
        let pack = build_context_pack(
            EntityUri::task("t_subject"),
            ContextPolicy {
                lexical_limit: 3,
                graph_limit: 3,
                vector_limit: 3,
                max_items: 2,
            },
            ContextBrokerInput {
                subject_item: ContextItem {
                    entity_uri: EntityUri::task("t_subject"),
                    source: "subject".to_owned(),
                    provenance: vec![],
                    score: None,
                    title: None,
                    snippet: None,
                },
                lexical: SearchResults {
                    hits: vec![
                        SearchHit {
                            task_id: "t_one".to_owned(),
                            seq: 1,
                            score: 1.0,
                            snippet: None,
                        },
                        SearchHit {
                            task_id: "t_two".to_owned(),
                            seq: 2,
                            score: 1.0,
                            snippet: None,
                        },
                    ],
                    meta: SearchMeta {
                        backend: "sqlite".to_owned(),
                        stale: true,
                        index_version: None,
                        last_event_id: None,
                        index_lag_events: None,
                    },
                },
                graph: vec![],
                vector: vec![],
                graph_status,
                vector_status,
            },
        );

        assert_eq!(pack.items.len(), 2);
        assert_eq!(
            pack.degraded,
            vec![
                "search_sqlite_fallback",
                "search_stale",
                "graph_disabled",
                "vector_disabled"
            ]
        );
    }
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("context retrieval error: {0}")]
    Retrieval(String),
}
