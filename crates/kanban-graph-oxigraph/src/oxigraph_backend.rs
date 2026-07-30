use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use oxigraph::{
    model::{GraphName, GraphNameRef, NamedNode, NamedNodeRef, NamedOrBlankNodeRef, Quad, Term},
    sparql::{QueryResults, SparqlEvaluator},
    store::Store,
};

use kanban_graph::{GraphError, GraphQueryBinding, GraphQueryRow, GraphStoreStatus, RelationGraph};

const PREDICATE_BASE: &str = "kb://predicate/";

pub struct OxigraphStore {
    store: Store,
    backend: String,
    snapshot_path: Option<PathBuf>,
    relations: Mutex<BTreeMap<String, Vec<Relation>>>,
}

impl OxigraphStore {
    pub fn in_memory() -> Result<Self, GraphError> {
        Ok(Self {
            store: Store::new().map_err(store_error)?,
            backend: "oxigraph-memory".to_owned(),
            snapshot_path: None,
            relations: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, GraphError> {
        let snapshot_path = path.as_ref().join("relations.json");
        let relations = read_snapshot(&snapshot_path)?;
        Self::with_snapshot(path.as_ref(), relations)
    }

    pub fn replace(path: impl AsRef<Path>, relations: &[Relation]) -> Result<Self, GraphError> {
        let graph = Self::with_snapshot(path.as_ref(), Vec::new())?;
        graph.rebuild(relations)?;
        Ok(graph)
    }

    pub fn replace_entities(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> Result<(), GraphError> {
        self.replace_entities_optimized(entity_uris, relations)
    }

    fn replace_entities_optimized(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> Result<(), GraphError> {
        let incoming = group_relations_by_subject(relations.to_vec());
        let mut clear_subjects = entity_uris
            .iter()
            .map(|entity_uri| entity_uri.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        clear_subjects.extend(incoming.keys().cloned());

        let mut stored = self.relations.lock().map_err(lock_error)?;
        for subject in clear_subjects {
            self.clear_entity_graph(&EntityUri::new(subject.clone()).map_err(store_error)?)?;
            stored.remove(&subject);
        }
        for relation in relations {
            let quad = Self::relation_quad(relation)?;
            self.store.insert(&quad).map_err(store_error)?;
        }
        for (subject, relations) in incoming {
            stored.insert(subject, relations);
        }
        self.write_snapshot(&stored)?;
        Ok(())
    }

    fn with_snapshot(path: &Path, relations: Vec<Relation>) -> Result<Self, GraphError> {
        let snapshot_path = path.join("relations.json");
        let store = Store::new().map_err(store_error)?;
        for relation in &relations {
            store
                .insert(&Self::relation_quad(relation)?)
                .map_err(store_error)?;
        }
        Ok(Self {
            store,
            backend: "oxigraph".to_owned(),
            snapshot_path: Some(snapshot_path),
            relations: Mutex::new(group_relations_by_subject(relations)),
        })
    }

    fn relation_quad(relation: &Relation) -> Result<Quad, GraphError> {
        Ok(Quad::new(
            named_node(relation.subject_uri.as_str())?,
            predicate_node(relation.predicate)?,
            Term::NamedNode(named_node(relation.object_uri.as_str())?),
            GraphName::NamedNode(named_node(&entity_graph_uri(&relation.subject_uri))?),
        ))
    }
}

impl RelationGraph for OxigraphStore {
    fn status(&self) -> GraphStoreStatus {
        GraphStoreStatus {
            backend: self.backend.clone(),
            enabled: true,
            message:
                "Oxigraph relation store is enabled; SQLite remains the operational source of truth"
                    .to_owned(),
        }
    }

    fn init(&self) -> Result<(), GraphError> {
        Ok(())
    }

    fn upsert(&self, relations: &[Relation]) -> Result<(), GraphError> {
        let incoming = group_relations_by_subject(relations.to_vec());
        let mut stored = self.relations.lock().map_err(lock_error)?;
        let subjects = incoming.keys().cloned().collect::<Vec<_>>();
        for subject in &subjects {
            self.clear_entity_graph(&EntityUri::new(subject.clone()).map_err(store_error)?)?;
        }
        for relation in relations {
            let quad = Self::relation_quad(relation)?;
            self.store.insert(&quad).map_err(store_error)?;
        }
        for (subject, relations) in incoming {
            stored.insert(subject, relations);
        }
        self.write_snapshot(&stored)?;
        Ok(())
    }

    fn delete(&self, entity_uri: &EntityUri) -> Result<(), GraphError> {
        let mut stored = self.relations.lock().map_err(lock_error)?;
        self.clear_entity_graph(entity_uri)?;
        stored.remove(entity_uri.as_str());
        self.write_snapshot(&stored)?;
        Ok(())
    }

    fn rebuild(&self, relations: &[Relation]) -> Result<(), GraphError> {
        self.store.clear().map_err(store_error)?;
        for relation in relations {
            let quad = Self::relation_quad(relation)?;
            self.store.insert(&quad).map_err(store_error)?;
        }
        let mut stored = self.relations.lock().map_err(lock_error)?;
        *stored = group_relations_by_subject(relations.to_vec());
        self.write_snapshot(&stored)?;
        Ok(())
    }

    fn replace_entities(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> Result<(), GraphError> {
        self.replace_entities_optimized(entity_uris, relations)
    }

    fn neighbors(
        &self,
        entity_uri: &EntityUri,
        predicate: Option<Predicate>,
        limit: usize,
    ) -> Result<Vec<Relation>, GraphError> {
        let subject = named_node(entity_uri.as_str())?;
        let predicate_node = predicate.map(predicate_node).transpose()?;
        let mut out = Vec::new();
        for quad in self
            .store
            .quads_for_pattern(
                Some(NamedOrBlankNodeRef::NamedNode(subject.as_ref())),
                predicate_node.as_ref().map(NamedNode::as_ref),
                None,
                None,
            )
            .take(limit)
        {
            let quad = quad.map_err(store_error)?;
            let object = match quad.object {
                Term::NamedNode(node) => {
                    EntityUri::new(node.as_str().to_owned()).map_err(store_error)?
                }
                _ => continue,
            };
            let graph_uri = match quad.graph_name {
                GraphName::NamedNode(node) => {
                    EntityUri::new(node.as_str().to_owned()).map_err(store_error)?
                }
                GraphName::BlankNode(_) | GraphName::DefaultGraph => {
                    EntityUri::new("kb://graph/indexed").map_err(store_error)?
                }
            };
            out.push(Relation {
                subject_uri: entity_uri.clone(),
                predicate: predicate_from_node(&quad.predicate)?,
                object_uri: object,
                graph_uri,
                provenance: Provenance {
                    source_table: None,
                    source_id: None,
                    source_event_id: None,
                    authoritative_store: "sqlite".to_owned(),
                },
                metadata_json: "{}".to_owned(),
                created_at: 0,
                updated_at: 0,
            });
        }
        Ok(out)
    }

    fn query(&self, sparql: &str, limit: usize) -> Result<Vec<GraphQueryRow>, GraphError> {
        match SparqlEvaluator::new()
            .parse_query(sparql)
            .map_err(|error| GraphError::Store(error.to_string()))?
            .on_store(&self.store)
            .execute()
            .map_err(|error| GraphError::Store(error.to_string()))?
        {
            QueryResults::Solutions(solutions) => solutions
                .take(limit)
                .map(|solution| {
                    let solution =
                        solution.map_err(|error| GraphError::Store(error.to_string()))?;
                    Ok(GraphQueryRow {
                        bindings: solution
                            .iter()
                            .map(|(name, value)| GraphQueryBinding {
                                name: name.as_str().to_owned(),
                                value: value.to_string(),
                            })
                            .collect(),
                    })
                })
                .collect(),
            QueryResults::Boolean(value) => Ok(vec![GraphQueryRow {
                bindings: vec![GraphQueryBinding {
                    name: "boolean".to_owned(),
                    value: value.to_string(),
                }],
            }]),
            QueryResults::Graph(triples) => triples
                .take(limit)
                .map(|triple| {
                    let triple = triple.map_err(|error| GraphError::Store(error.to_string()))?;
                    Ok(GraphQueryRow {
                        bindings: vec![GraphQueryBinding {
                            name: "triple".to_owned(),
                            value: triple.to_string(),
                        }],
                    })
                })
                .collect(),
        }
    }
}

impl OxigraphStore {
    fn clear_entity_graph(&self, entity_uri: &EntityUri) -> Result<(), GraphError> {
        self.store
            .clear_graph(GraphNameRef::NamedNode(
                NamedNodeRef::new(&entity_graph_uri(entity_uri)).map_err(store_error)?,
            ))
            .map_err(store_error)?;
        Ok(())
    }

    fn write_snapshot(
        &self,
        relations: &BTreeMap<String, Vec<Relation>>,
    ) -> Result<(), GraphError> {
        let Some(path) = &self.snapshot_path else {
            return Ok(());
        };
        let snapshot = relations
            .values()
            .flat_map(|relations| relations.iter().cloned())
            .collect::<Vec<_>>();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let bytes = serde_json::to_vec_pretty(&snapshot).map_err(json_error)?;
        let temp_path = path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        fs::rename(&temp_path, path).map_err(io_error)?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(io_error)?;
        }
        Ok(())
    }
}

fn read_snapshot(path: &Path) -> Result<Vec<Relation>, GraphError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(json_error),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(io_error(error)),
    }
}

fn group_relations_by_subject(relations: Vec<Relation>) -> BTreeMap<String, Vec<Relation>> {
    let mut grouped = BTreeMap::<String, Vec<Relation>>::new();
    for relation in relations {
        grouped
            .entry(relation.subject_uri.as_str().to_owned())
            .or_default()
            .push(relation);
    }
    grouped
}

fn named_node(value: &str) -> Result<NamedNode, GraphError> {
    NamedNode::new(value).map_err(|error| GraphError::Store(error.to_string()))
}

fn predicate_node(predicate: Predicate) -> Result<NamedNode, GraphError> {
    named_node(&format!("{PREDICATE_BASE}{}", predicate.as_str()))
}

fn predicate_from_node(node: &NamedNode) -> Result<Predicate, GraphError> {
    let value = node
        .as_str()
        .strip_prefix(PREDICATE_BASE)
        .ok_or_else(|| GraphError::Store(format!("unknown predicate IRI: {node}")))?;
    predicate_from_str(value)
}

fn predicate_from_str(value: &str) -> Result<Predicate, GraphError> {
    match value {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        _ => Err(GraphError::Store(format!("unknown predicate: {value}"))),
    }
}

fn entity_graph_uri(entity_uri: &EntityUri) -> String {
    format!(
        "kb://graph/entity/{}",
        encode_uri_component(entity_uri.as_str())
    )
}

fn encode_uri_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex(byte >> 4));
            out.push(hex(byte & 0x0f));
        }
    }
    out
}

fn hex(value: u8) -> char {
    b"0123456789ABCDEF"[value as usize] as char
}

fn store_error(error: impl std::fmt::Display) -> GraphError {
    GraphError::Store(error.to_string())
}

fn io_error(error: impl std::fmt::Display) -> GraphError {
    GraphError::Store(error.to_string())
}

fn json_error(error: impl std::fmt::Display) -> GraphError {
    GraphError::Store(error.to_string())
}

fn lock_error(error: impl std::fmt::Display) -> GraphError {
    GraphError::Store(error.to_string())
}

#[cfg(test)]
mod tests {
    use kanban_entity::{EntityUri, Predicate, Provenance, Relation};

    use crate::OxigraphStore;
    use kanban_graph::RelationGraph;

    fn relation(subject: &str, predicate: Predicate, object: &str) -> Relation {
        Relation {
            subject_uri: EntityUri::new(subject).unwrap(),
            predicate,
            object_uri: EntityUri::new(object).unwrap(),
            graph_uri: EntityUri::new("kb://graph/indexed").unwrap(),
            provenance: Provenance {
                source_table: Some("test".to_owned()),
                source_id: Some("test".to_owned()),
                source_event_id: Some(1),
                authoritative_store: "sqlite".to_owned(),
            },
            metadata_json: "{}".to_owned(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn upsert_replaces_entity_scoped_named_graph() {
        let graph = OxigraphStore::in_memory().unwrap();
        graph
            .upsert(&[
                relation(
                    "kb://task/child",
                    Predicate::DependsOn,
                    "kb://task/old-parent",
                ),
                relation(
                    "kb://task/child",
                    Predicate::BelongsToBoard,
                    "kb://board/default",
                ),
            ])
            .unwrap();

        graph
            .upsert(&[relation(
                "kb://task/child",
                Predicate::DependsOn,
                "kb://task/new-parent",
            )])
            .unwrap();

        let neighbors = graph
            .neighbors(&EntityUri::new("kb://task/child").unwrap(), None, 10)
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(
            neighbors[0].object_uri,
            EntityUri::new("kb://task/new-parent").unwrap()
        );
        assert!(
            neighbors[0]
                .graph_uri
                .as_str()
                .starts_with("kb://graph/entity/kb%3A%2F%2Ftask%2Fchild")
        );
    }

    #[test]
    fn trait_replace_entities_replaces_incoming_subjects_not_listed_for_delete() {
        let graph = OxigraphStore::in_memory().unwrap();
        let child = EntityUri::new("kb://task/child").unwrap();
        graph
            .upsert(&[relation(
                "kb://task/child",
                Predicate::DependsOn,
                "kb://task/old-parent",
            )])
            .unwrap();

        let graph_trait: &dyn RelationGraph = &graph;
        graph_trait
            .replace_entities(
                &[],
                &[relation(
                    "kb://task/child",
                    Predicate::DependsOn,
                    "kb://task/new-parent",
                )],
            )
            .unwrap();

        let neighbors = graph.neighbors(&child, None, 10).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(
            neighbors[0].object_uri,
            EntityUri::new("kb://task/new-parent").unwrap()
        );
    }

    #[test]
    fn delete_rebuild_neighbors_and_query_use_oxigraph_store() {
        let graph = OxigraphStore::in_memory().unwrap();
        let child = EntityUri::new("kb://task/child").unwrap();
        graph
            .rebuild(&[
                relation("kb://task/child", Predicate::DependsOn, "kb://task/parent"),
                relation("kb://task/child", Predicate::RelatedTo, "kb://task/peer"),
            ])
            .unwrap();

        let depends = graph
            .neighbors(&child, Some(Predicate::DependsOn), 10)
            .unwrap();
        assert_eq!(depends.len(), 1);
        assert_eq!(
            depends[0].object_uri,
            EntityUri::new("kb://task/parent").unwrap()
        );

        let rows = graph
            .query(
                "SELECT ?o WHERE { GRAPH ?g { <kb://task/child> <kb://predicate/depends_on> ?o } }",
                10,
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].bindings[0].value.contains("kb://task/parent"));

        graph.delete(&child).unwrap();
        assert!(graph.neighbors(&child, None, 10).unwrap().is_empty());
    }

    #[test]
    fn open_reloads_json_snapshot_store() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("graph");
        let child = EntityUri::new("kb://task/child").unwrap();
        {
            let graph = OxigraphStore::open(&path).unwrap();
            graph
                .upsert(&[relation(
                    "kb://task/child",
                    Predicate::DependsOn,
                    "kb://task/parent",
                )])
                .unwrap();
        }

        let graph = OxigraphStore::open(&path).unwrap();
        let neighbors = graph.neighbors(&child, None, 10).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(
            neighbors[0].object_uri,
            EntityUri::new("kb://task/parent").unwrap()
        );
    }
}
