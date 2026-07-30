use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Mutex,
};

use crate::common::*;

use kanban_local::DerivedStoreWriteGuard;
use kanban_sqlite::api::lifecycle::begin_database_replace;
use kanban_sqlite::api::provider::{
    ProjectionArtifactEvidence, ProjectionBatch, ProjectionBatchReceipt, ProjectionCorpusMetadata,
    ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor, begin_projection_generation, prepare_projection_snapshot_with,
    publish_projection_generation_with, reconcile_projection_generation_with,
    recover_projection_generation_with, run_projection_batch_with,
};
use kanban_sqlite::api::{
    abort_projection_generation, acquire_projection_lease, projection_status,
    release_projection_lease,
};

const STORE: &str = "tantivy_tasks";

struct FakeProjectionStore {
    descriptor: ProjectionStoreDescriptor,
    state: Mutex<FakeProjectionStoreState>,
}

impl Default for FakeProjectionStore {
    fn default() -> Self {
        Self::for_store(STORE)
    }
}

#[derive(Default)]
struct FakeProjectionStoreState {
    prepared: Option<ProjectionArtifactEvidence>,
    active: Option<ProjectionArtifactEvidence>,
    generations: BTreeMap<String, ProjectionArtifactEvidence>,
    published_generations: BTreeSet<String>,
    quarantined: BTreeMap<String, ProjectionArtifactEvidence>,
    max_fence_epoch: i64,
    bad_snapshot_evidence: bool,
    bad_batch_receipt: bool,
    fail_after_publish_once: bool,
    active_inspect_failure: Option<FakeInspectFailure>,
    generation_inspect_failure: Option<(String, FakeInspectFailure)>,
    corrupt_publication_marker: Option<String>,
    publication_validation_failure: Option<String>,
    active_contents_validation_failure: bool,
    label_mutation_during_apply: Option<ApplyLabelMutation>,
    label_mutation_during_publish: Option<ApplyLabelMutation>,
    repair_calls: usize,
}

struct ApplyLabelMutation {
    path: PathBuf,
    board: String,
    label_ref: String,
    description: String,
}

#[derive(Clone, Copy)]
enum FakeInspectFailure {
    BindingMismatch,
    TransientStorage,
}

impl FakeProjectionStore {
    fn for_store(store_name: &str) -> Self {
        Self {
            descriptor: ProjectionStoreDescriptor {
                store_name: store_name.to_owned(),
                provider: "fake".to_owned(),
                provider_fingerprint: "fake-v1".to_owned(),
                corpus: fake_corpus_for_store(store_name),
            },
            state: Mutex::new(FakeProjectionStoreState::default()),
        }
    }

    fn with_provider(provider: &str, provider_fingerprint: &str) -> Self {
        Self {
            descriptor: ProjectionStoreDescriptor {
                store_name: STORE.to_owned(),
                provider: provider.to_owned(),
                provider_fingerprint: provider_fingerprint.to_owned(),
                corpus: None,
            },
            state: Mutex::new(FakeProjectionStoreState::default()),
        }
    }

    fn with_corpus(
        store_name: &str,
        provider: &str,
        provider_fingerprint: &str,
        corpus: Option<ProjectionCorpusMetadata>,
    ) -> Self {
        Self {
            descriptor: ProjectionStoreDescriptor {
                store_name: store_name.to_owned(),
                provider: provider.to_owned(),
                provider_fingerprint: provider_fingerprint.to_owned(),
                corpus,
            },
            state: Mutex::new(FakeProjectionStoreState::default()),
        }
    }

    fn with_bad_snapshot_evidence() -> Self {
        let store = Self::default();
        store.state.lock().expect("fake lock").bad_snapshot_evidence = true;
        store
    }

    fn with_bad_batch_receipt() -> Self {
        let store = Self::default();
        store.state.lock().expect("fake lock").bad_batch_receipt = true;
        store
    }

    fn reject_next_batch_receipt(&self) {
        self.state.lock().expect("fake lock").bad_batch_receipt = true;
    }

    fn mutate_label_semantics_during_next_batch(
        &self,
        path: &Path,
        board: &str,
        label_ref: &str,
        description: &str,
    ) {
        self.state
            .lock()
            .expect("fake lock")
            .label_mutation_during_apply = Some(ApplyLabelMutation {
            path: path.to_path_buf(),
            board: board.to_owned(),
            label_ref: label_ref.to_owned(),
            description: description.to_owned(),
        });
    }

    fn mutate_label_semantics_during_next_publish(
        &self,
        path: &Path,
        board: &str,
        label_ref: &str,
        description: &str,
    ) {
        self.state
            .lock()
            .expect("fake lock")
            .label_mutation_during_publish = Some(ApplyLabelMutation {
            path: path.to_path_buf(),
            board: board.to_owned(),
            label_ref: label_ref.to_owned(),
            description: description.to_owned(),
        });
    }

    fn fail_after_publish_once(&self) {
        self.state
            .lock()
            .expect("fake lock")
            .fail_after_publish_once = true;
    }

    fn corrupt_generation_schema_version(&self, generation: &str) {
        self.state
            .lock()
            .expect("fake lock")
            .generations
            .get_mut(generation)
            .expect("prepared fake generation")
            .manifest
            .schema_version += 1;
    }

    fn corrupt_active_generation_schema_version(&self, generation: &str) {
        let mut state = self.state.lock().expect("fake lock");
        state
            .generations
            .get_mut(generation)
            .expect("published fake generation")
            .manifest
            .schema_version += 1;
        state
            .active
            .as_mut()
            .filter(|active| active.manifest.generation == generation)
            .expect("active fake generation")
            .manifest
            .schema_version += 1;
    }

    fn fail_active_inspection(&self, failure: FakeInspectFailure) {
        self.state.lock().expect("fake lock").active_inspect_failure = Some(failure);
    }

    fn fail_generation_inspection(&self, generation: &str, failure: FakeInspectFailure) {
        self.state
            .lock()
            .expect("fake lock")
            .generation_inspect_failure = Some((generation.to_owned(), failure));
    }

    fn clear_inspection_failures(&self) {
        let mut state = self.state.lock().expect("fake lock");
        state.active_inspect_failure = None;
        state.generation_inspect_failure = None;
    }

    fn install_unexpected_active(&self, generation: &str) -> ProjectionArtifactEvidence {
        let mut state = self.state.lock().expect("fake lock");
        let mut evidence = state
            .prepared
            .clone()
            .expect("prepared fake generation for unexpected active");
        evidence.manifest.generation = generation.to_owned();
        evidence.fingerprint = format!("sha256:{generation}:{}", evidence.manifest.snapshot_cursor);
        evidence.manifest.fingerprint = Some(evidence.fingerprint.clone());
        state
            .generations
            .insert(generation.to_owned(), evidence.clone());
        state.published_generations.insert(generation.to_owned());
        state.active = Some(evidence.clone());
        evidence
    }

    fn corrupt_publication_marker(&self, generation: &str) {
        let mut state = self.state.lock().expect("fake lock");
        state.corrupt_publication_marker = Some(generation.to_owned());
        state.published_generations.remove(generation);
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.manifest.generation == generation)
        {
            let fallback = state
                .published_generations
                .iter()
                .filter_map(|generation| state.generations.get(generation))
                .max_by_key(|evidence| evidence.manifest.fence_epoch)
                .cloned();
            state.active = fallback;
        }
    }

    fn fail_publication_validation(&self, generation: &str) {
        self.state
            .lock()
            .expect("fake lock")
            .publication_validation_failure = Some(generation.to_owned());
    }

    fn fail_active_contents_validation(&self) {
        self.state
            .lock()
            .expect("fake lock")
            .active_contents_validation_failure = true;
    }

    fn repair_calls(&self) -> usize {
        self.state.lock().expect("fake lock").repair_calls
    }

    fn quarantined_generation(&self, generation: &str) -> Option<ProjectionArtifactEvidence> {
        self.state
            .lock()
            .expect("fake lock")
            .quarantined
            .get(generation)
            .cloned()
    }
}

impl ProjectionStoreBackend for FakeProjectionStore {
    fn descriptor(&self) -> kanban_core::Result<ProjectionStoreDescriptor> {
        Ok(self.descriptor.clone())
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> kanban_core::Result<ProjectionArtifactEvidence> {
        let mut state = self.state.lock().expect("fake lock");
        let manifest = &snapshot.manifest;
        if manifest.fence_epoch < state.max_fence_epoch {
            return Err(KanbanError::Conflict("stale store fence".to_owned()));
        }
        let (count, digest) = fake_snapshot_coverage(snapshot);
        if count != manifest.canonical_item_count || digest != manifest.canonical_digest {
            return Err(KanbanError::Conflict(
                "canonical snapshot records do not match manifest".to_owned(),
            ));
        }
        state.max_fence_epoch = manifest.fence_epoch;
        let fingerprint = format!(
            "sha256:{}:{}",
            manifest.generation, manifest.snapshot_cursor
        );
        let mut evidence_manifest = manifest.clone();
        if state.bad_snapshot_evidence {
            evidence_manifest.database_instance_id = "db_wrong".to_owned();
        }
        evidence_manifest.fingerprint = Some(fingerprint.clone());
        let evidence = ProjectionArtifactEvidence {
            manifest: evidence_manifest,
            fingerprint,
        };
        state.prepared = Some(evidence.clone());
        state
            .generations
            .insert(evidence.manifest.generation.clone(), evidence.clone());
        Ok(evidence)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> kanban_core::Result<ProjectionBatchReceipt> {
        let mut state = self.state.lock().expect("fake lock");
        if batch.fence_epoch < state.max_fence_epoch {
            return Err(KanbanError::Conflict("stale store fence".to_owned()));
        }
        let generation_known = state
            .prepared
            .as_ref()
            .is_some_and(|value| value.manifest.generation == batch.target_generation)
            || state
                .active
                .as_ref()
                .is_some_and(|value| value.manifest.generation == batch.target_generation);
        if !generation_known {
            return Err(KanbanError::Conflict(
                "unknown target generation".to_owned(),
            ));
        }
        state.max_fence_epoch = batch.fence_epoch;
        let bad_batch_receipt = state.bad_batch_receipt;
        let label_mutation = state.label_mutation_during_apply.take();
        drop(state);
        if let Some(mutation) = label_mutation {
            upsert_label_semantics(
                &mutation.path,
                &mutation.board,
                UpsertLabelSemantics {
                    label_ref: mutation.label_ref,
                    description: Some(mutation.description),
                    ..UpsertLabelSemantics::default()
                },
            )?;
        }
        Ok(ProjectionBatchReceipt {
            store_name: batch.store_name.clone(),
            database_instance_id: batch.database_instance_id.clone(),
            protocol_version: batch.protocol_version,
            schema_version: batch.schema_version,
            provider: batch.provider.clone(),
            provider_fingerprint: batch.provider_fingerprint.clone(),
            target_generation: batch.target_generation.clone(),
            lease_token: batch.lease_token.clone(),
            fence_epoch: batch.fence_epoch,
            claim_token: if bad_batch_receipt {
                "pclaim_wrong".to_owned()
            } else {
                batch.claim_token.clone()
            },
            applied_item_count: batch.items.len(),
        })
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> kanban_core::Result<ProjectionPublishReceipt> {
        let mut state = self.state.lock().expect("fake lock");
        if state
            .corrupt_publication_marker
            .as_deref()
            .is_some_and(|generation| generation != prepared.manifest.generation)
        {
            return Err(KanbanError::Storage(
                "publish candidate does not own the corrupt marker".to_owned(),
            ));
        }
        if state.active.as_ref() != expected_active {
            return Err(KanbanError::Conflict(
                "active generation CAS mismatch".to_owned(),
            ));
        }
        if state.prepared.as_ref() != Some(prepared) {
            return Err(KanbanError::Conflict(
                "prepared generation readback mismatch".to_owned(),
            ));
        }
        if prepared.manifest.fence_epoch < state.max_fence_epoch {
            return Err(KanbanError::Conflict("stale store fence".to_owned()));
        }
        if state.corrupt_publication_marker.is_some() {
            state.corrupt_publication_marker = None;
        }
        state.max_fence_epoch = prepared.manifest.fence_epoch;
        let retained_previous = state.active.clone();
        state
            .generations
            .insert(prepared.manifest.generation.clone(), prepared.clone());
        state
            .published_generations
            .insert(prepared.manifest.generation.clone());
        state.active = Some(prepared.clone());
        if state.fail_after_publish_once {
            state.fail_after_publish_once = false;
            return Err(KanbanError::Storage(
                "simulated crash after pointer swap".to_owned(),
            ));
        }
        let label_mutation = state.label_mutation_during_publish.take();
        drop(state);
        if let Some(mutation) = label_mutation {
            upsert_label_semantics(
                &mutation.path,
                &mutation.board,
                UpsertLabelSemantics {
                    label_ref: mutation.label_ref,
                    description: Some(mutation.description),
                    ..UpsertLabelSemantics::default()
                },
            )?;
        }
        Ok(ProjectionPublishReceipt {
            active: prepared.clone(),
            retained_previous,
        })
    }

    fn inspect_active(&self) -> kanban_core::Result<Option<ProjectionArtifactEvidence>> {
        let state = self.state.lock().expect("fake lock");
        if state.corrupt_publication_marker.is_some() {
            return Err(KanbanError::Storage(
                "corrupt publication marker".to_owned(),
            ));
        }
        if state.active.is_some() {
            match state.active_inspect_failure {
                Some(FakeInspectFailure::BindingMismatch) => {
                    return Err(KanbanError::Conflict(
                        "physical evidence binding mismatch".to_owned(),
                    ));
                }
                Some(FakeInspectFailure::TransientStorage) => {
                    return Err(KanbanError::Storage(
                        "transient physical inspection failure".to_owned(),
                    ));
                }
                None => {}
            }
        }
        Ok(state.active.clone())
    }

    fn inspect_generation(
        &self,
        generation: &str,
    ) -> kanban_core::Result<Option<ProjectionArtifactEvidence>> {
        let state = self.state.lock().expect("fake lock");
        if let Some((failed_generation, failure)) = &state.generation_inspect_failure
            && failed_generation == generation
        {
            return match failure {
                FakeInspectFailure::BindingMismatch => Err(KanbanError::Conflict(
                    "physical generation evidence binding mismatch".to_owned(),
                )),
                FakeInspectFailure::TransientStorage => Err(KanbanError::Storage(
                    "transient generation inspection failure".to_owned(),
                )),
            };
        }
        Ok(state.generations.get(generation).cloned())
    }

    fn validate_generation_publication(
        &self,
        expected: &ProjectionArtifactEvidence,
    ) -> kanban_core::Result<()> {
        let state = self.state.lock().expect("fake lock");
        if state.corrupt_publication_marker.as_deref()
            == Some(expected.manifest.generation.as_str())
        {
            return Err(KanbanError::Storage(
                "corrupt publication marker".to_owned(),
            ));
        }
        if state.publication_validation_failure.as_deref()
            == Some(expected.manifest.generation.as_str())
        {
            return Err(KanbanError::Storage(
                "corrupt auxiliary publication state".to_owned(),
            ));
        }
        match state.generations.get(&expected.manifest.generation) {
            Some(actual) if actual == expected => Ok(()),
            Some(_) => Err(KanbanError::Storage(
                "fake generation evidence mismatch".to_owned(),
            )),
            None => Err(KanbanError::Storage(
                "fake generation is missing".to_owned(),
            )),
        }
    }

    fn repair_generation_publication(
        &self,
        expected: &ProjectionArtifactEvidence,
    ) -> kanban_core::Result<()> {
        let mut state = self.state.lock().expect("fake lock");
        state.repair_calls += 1;
        if state
            .corrupt_publication_marker
            .as_deref()
            .is_some_and(|generation| generation != expected.manifest.generation)
        {
            return Err(KanbanError::Storage(
                "publication repair candidate does not own the corrupt marker".to_owned(),
            ));
        }
        if state.generations.get(&expected.manifest.generation) != Some(expected) {
            return Err(KanbanError::Storage(
                "publication repair candidate evidence mismatch".to_owned(),
            ));
        }
        state.corrupt_publication_marker = None;
        state
            .published_generations
            .insert(expected.manifest.generation.clone());
        state.active = Some(expected.clone());
        Ok(())
    }

    fn validate_active_contents(
        &self,
        _active: &ProjectionArtifactEvidence,
    ) -> kanban_core::Result<()> {
        if self
            .state
            .lock()
            .expect("fake lock")
            .active_contents_validation_failure
        {
            return Err(KanbanError::Storage(
                "active contents include a newer canonical mutation".to_owned(),
            ));
        }
        Ok(())
    }

    fn quarantine_generation(&self, generation: &str) -> kanban_core::Result<()> {
        let mut state = self.state.lock().expect("fake lock");
        let evidence = state
            .generations
            .remove(generation)
            .or_else(|| {
                state
                    .active
                    .as_ref()
                    .filter(|active| active.manifest.generation == generation)
                    .cloned()
            })
            .or_else(|| {
                state
                    .prepared
                    .as_ref()
                    .filter(|prepared| prepared.manifest.generation == generation)
                    .cloned()
            });
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.manifest.generation == generation)
        {
            state.active = None;
        }
        if state
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.manifest.generation == generation)
        {
            state.prepared = None;
        }
        if let Some(evidence) = evidence {
            state.published_generations.remove(generation);
            state.quarantined.insert(generation.to_owned(), evidence);
        }
        if state.active.is_none() {
            let fallback = state
                .published_generations
                .iter()
                .filter_map(|generation| state.generations.get(generation))
                .max_by_key(|evidence| evidence.manifest.fence_epoch)
                .cloned();
            state.active = fallback;
        }
        Ok(())
    }
}

fn fake_corpus_for_store(store_name: &str) -> Option<ProjectionCorpusMetadata> {
    let corpus_schema = match store_name {
        "lancedb_chunks" => "task-chunks-v2",
        "lancedb_label_atoms" => "label-atoms-v2",
        _ => return None,
    };
    Some(ProjectionCorpusMetadata {
        corpus_schema: corpus_schema.to_owned(),
        corpus_fingerprint: format!("{corpus_schema}:fake-v1"),
        embedding_model: "fake-embedding-v1".to_owned(),
        embedding_dimensions: 3,
    })
}

fn fake_snapshot_coverage(snapshot: &ProjectionSnapshot) -> (i64, String) {
    let mut hash = 0xcbf29ce484222325_u64;
    for record in &snapshot.records {
        assert_eq!(
            record.content_hash,
            fake_hash(record.payload_json.as_bytes())
        );
        for bytes in [
            record.board_id.as_bytes(),
            record.identity.as_bytes(),
            record.payload_json.as_bytes(),
            record.content_hash.as_bytes(),
        ] {
            for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    (snapshot.records.len() as i64, format!("fnv64:{hash:016x}"))
}

fn fake_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}

#[test]
fn init_seeds_stable_database_identity_and_explicit_bootstrap_health() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_identity")?;
    init_database(&temp.path, "first")?;
    let first = projection_status(&temp.path)?;
    init_database(&temp.path, "second")?;
    let second = projection_status(&temp.path)?;

    assert_eq!(first.database_instance_id, second.database_instance_id);
    assert!(first.database_instance_id.starts_with("db_"));
    assert_eq!(first.protocol_version, 2);
    assert_eq!(first.stores.len(), 4);
    assert!(first.stores.iter().all(|store| {
        store.control_plane == "legacy"
            && store.active_generation.is_none()
            && store.lifecycle_status == "bootstrap_required"
            && store.fallback_reason.as_deref() == Some("generation_rebuild_required")
    }));
    Ok(())
}

#[test]
fn generation_descriptors_fail_closed_on_store_corpus_contract() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_corpus_descriptor_contract")?;
    init_database(&temp.path, "tester")?;

    let chunks_without_corpus =
        FakeProjectionStore::with_corpus("lancedb_chunks", "fake", "fake-v1", None);
    let chunks_lease =
        acquire_projection_lease(&temp.path, "lancedb_chunks", "chunks-owner", 10_000)?;
    let error = result_err(begin_projection_generation(
        &temp.path,
        "lancedb_chunks",
        "chunks-owner",
        &chunks_lease.lease_token,
        &chunks_without_corpus,
    ))?;
    assert!(error.to_string().contains("corpus binding"));

    let tantivy_with_corpus = FakeProjectionStore::with_corpus(
        STORE,
        "fake",
        "fake-v1",
        fake_corpus_for_store("lancedb_chunks"),
    );
    let tantivy_lease = acquire_projection_lease(&temp.path, STORE, "tantivy-owner", 10_000)?;
    let error = result_err(begin_projection_generation(
        &temp.path,
        STORE,
        "tantivy-owner",
        &tantivy_lease.lease_token,
        &tantivy_with_corpus,
    ))?;
    assert!(error.to_string().contains("unexpected corpus binding"));
    Ok(())
}

#[test]
fn lance_store_corpus_bindings_are_independent_and_durable() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_independent_corpus_bindings")?;
    init_database(&temp.path, "tester")?;
    let chunks = FakeProjectionStore::for_store("lancedb_chunks");
    let labels = FakeProjectionStore::for_store("lancedb_label_atoms");
    let chunks_lease =
        acquire_projection_lease(&temp.path, "lancedb_chunks", "chunks-owner", 20_000)?;
    let labels_lease =
        acquire_projection_lease(&temp.path, "lancedb_label_atoms", "labels-owner", 20_000)?;

    let chunks_manifest = begin_projection_generation(
        &temp.path,
        "lancedb_chunks",
        "chunks-owner",
        &chunks_lease.lease_token,
        &chunks,
    )?;
    let labels_manifest = begin_projection_generation(
        &temp.path,
        "lancedb_label_atoms",
        "labels-owner",
        &labels_lease.lease_token,
        &labels,
    )?;

    assert_eq!(
        chunks_manifest.corpus,
        fake_corpus_for_store("lancedb_chunks")
    );
    assert_eq!(
        labels_manifest.corpus,
        fake_corpus_for_store("lancedb_label_atoms")
    );
    assert_ne!(chunks_manifest.corpus, labels_manifest.corpus);
    let status = projection_status(&temp.path)?;
    let chunks_status = status
        .stores
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .expect("chunks projection state");
    let labels_status = status
        .stores
        .iter()
        .find(|store| store.store_name == "lancedb_label_atoms")
        .expect("label atom projection state");
    assert_eq!(chunks_status.building_corpus, chunks_manifest.corpus);
    assert_eq!(labels_status.building_corpus, labels_manifest.corpus);
    Ok(())
}

#[test]
fn lance_corpus_binding_rotates_with_active_and_previous_generations() -> anyhow::Result<()> {
    const CHUNKS: &str = "lancedb_chunks";
    let temp = TempDb::new("projection_v2_corpus_generation_rotation")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::for_store(CHUNKS);
    let lease = acquire_projection_lease(&temp.path, CHUNKS, "owner", 20_000)?;

    let first =
        begin_projection_generation(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;

    let second =
        begin_projection_generation(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;

    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == CHUNKS)
        .expect("chunks projection state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(second.generation.as_str())
    );
    assert_eq!(
        store.previous_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(store.active_corpus, second.corpus);
    assert_eq!(store.previous_corpus, first.corpus);
    assert_eq!(store.building_corpus, None);
    Ok(())
}

#[test]
fn lance_prepare_batch_and_publish_revalidate_the_corpus_binding() -> anyhow::Result<()> {
    const CHUNKS: &str = "lancedb_chunks";
    let temp = TempDb::new("projection_v2_corpus_operation_binding")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::for_store(CHUNKS);
    let wrong_backend = FakeProjectionStore::with_corpus(
        CHUNKS,
        "fake",
        "fake-v1",
        Some(ProjectionCorpusMetadata {
            corpus_schema: "task-chunks-v2".to_owned(),
            corpus_fingerprint: "task-chunks-v2:fake-v2".to_owned(),
            embedding_model: "fake-embedding-v2".to_owned(),
            embedding_dimensions: 4,
        }),
    );
    let lease = acquire_projection_lease(&temp.path, CHUNKS, "owner", 20_000)?;
    begin_projection_generation(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;

    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        CHUNKS,
        "owner",
        &lease.lease_token,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("corpus binding"));

    prepare_projection_snapshot_with(&temp.path, CHUNKS, "owner", &lease.lease_token, &backend)?;
    let error = result_err(run_projection_batch_with(
        &temp.path,
        CHUNKS,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("corpus binding"));
    let error = result_err(publish_projection_generation_with(
        &temp.path,
        CHUNKS,
        "owner",
        &lease.lease_token,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("corpus binding"));
    Ok(())
}

#[test]
fn same_owner_cannot_replace_an_unexpired_lease_token() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_same_owner_fence")?;
    init_database(&temp.path, "tester")?;
    let first = acquire_projection_lease(&temp.path, STORE, "serve", 10_000)?;
    let error = result_err(acquire_projection_lease(&temp.path, STORE, "serve", 10_000))?;
    assert!(error.to_string().contains("projection lease"));
    let conn = connect_file(&temp.path)?;
    let (token, epoch): (String, i64) = conn.query_row(
        "SELECT lease_token,fence_epoch FROM projection_store_state WHERE store_name=?1",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(token, first.lease_token);
    assert_eq!(epoch, first.fence_epoch);
    Ok(())
}

#[test]
fn projection_lease_debug_redacts_capability_token() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_lease_debug_redaction")?;
    init_database(&temp.path, "tester")?;
    let lease = acquire_projection_lease(&temp.path, STORE, "debug-owner", 10_000)?;

    let rendered = format!("{lease:?}");

    assert!(!rendered.contains(&lease.lease_token));
    assert!(rendered.contains("[REDACTED]"));
    Ok(())
}

#[test]
fn delivery_requires_resolvable_and_consistent_board_scope() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_board_scope")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let unresolved = conn.execute(
        "INSERT INTO index_outbox(\
           source_event_id,target,entity_uri,action,payload_json,status,attempts,created_at,updated_at\
         ) VALUES(NULL,'tantivy','kb://task/missing','upsert','{}','pending',0,1,1)",
        [],
    );
    assert!(unresolved.is_err());

    let (board_a, board_b): (String, String) = {
        let board_a: String =
            conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
                row.get(0)
            })?;
        conn.execute(
            "INSERT INTO boards(id,slug,name,created_at,updated_at) \
             VALUES('b_second','second','Second',1,1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO entities(\
               uri,kind,source_table,source_id,board_id,created_at,updated_at\
             ) VALUES('kb://task/foreign','task','fixture','foreign','b_second',1,1)",
            [],
        )?;
        (board_a, "b_second".to_owned())
    };
    conn.execute(
        "INSERT INTO tasks(\
           id,board_id,seq,title,status,priority,created_by,created_at,updated_at,metadata_json\
         ) VALUES('t_scope',?1,99,'Scope','todo',1,'tester',1,1,'{}')",
        [&board_a],
    )?;
    conn.execute(
        "INSERT INTO task_events(event_id,board_id,task_id,actor,kind,payload_json,created_at) \
         VALUES('e_scope',?1,'t_scope','tester','task.updated','{}',1)",
        [&board_a],
    )?;
    let event_id = conn.last_insert_rowid();
    let mismatch = conn.execute(
        "INSERT INTO index_outbox(\
           source_event_id,target,entity_uri,action,payload_json,status,attempts,created_at,updated_at\
         ) VALUES(?1,'tantivy','kb://task/foreign','upsert','{}','pending',0,1,1)",
        [event_id],
    );
    assert!(
        mismatch.is_err(),
        "boards {board_a} and {board_b} must fail"
    );
    Ok(())
}

#[test]
fn untrusted_snapshot_evidence_does_not_advance_delivery_or_checkpoint() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_snapshot_evidence")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let backend = FakeProjectionStore::with_bad_snapshot_evidence();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("artifact evidence"));
    let conn = connect_file(&temp.path)?;
    let (status, checkpoint): (String, i64) = conn.query_row(
        "SELECT d.status,s.checkpoint_cursor \
         FROM projection_deliveries d JOIN projection_store_state s USING(store_name) \
         WHERE d.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(status, "pending");
    assert_eq!(checkpoint, 0);
    Ok(())
}

#[test]
fn snapshot_coverage_change_cannot_bulk_acknowledge_canonical_work() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_snapshot_coverage_change")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    connect_file(&temp.path)?.execute(
        "UPDATE projection_deliveries SET payload_json='{\"changed\":true}'
         WHERE store_name=?1 AND cursor=10",
        [STORE],
    )?;

    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("snapshot coverage changed"));
    let conn = connect_file(&temp.path)?;
    let (status, checkpoint): (String, i64) = conn.query_row(
        "SELECT d.status,s.checkpoint_cursor
         FROM projection_deliveries d JOIN projection_store_state s USING(store_name)
         WHERE d.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(status, "pending");
    assert_eq!(checkpoint, 0);
    Ok(())
}

#[test]
fn canonical_change_after_begin_cannot_be_hidden_by_snapshot_evidence() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_canonical_coverage_change")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("changed after snapshot boundary"),
    )?;

    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(
        error
            .to_string()
            .contains("canonical snapshot coverage changed")
    );
    let status = projection_status(&temp.path)?;
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(store.lifecycle_status, "error");
    assert_eq!(
        store.fallback_reason.as_deref(),
        Some("derived_store_error")
    );
    Ok(())
}

#[test]
fn generation_begin_rejects_running_claim_and_atomically_blocks_legacy_completion()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_begin_control_plane")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    force_running_delivery(&temp.path, &lease, "gen_existing", 10)?;
    let error = result_err(begin_projection_generation(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("are running"));
    release_projection_lease(&temp.path, STORE, "owner", &lease.lease_token)?;

    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE index_outbox SET status='done',updated_at=2 WHERE id=10",
        [],
    )?;
    let (control_plane, delivery_status): (String, String) = conn.query_row(
        "SELECT s.control_plane,d.status
         FROM projection_store_state s
         JOIN projection_deliveries d USING(store_name)
         WHERE s.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(control_plane, "v2");
    assert_eq!(delivery_status, "pending");
    drop(conn);
    abort_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    Ok(())
}

#[test]
fn physical_writer_lock_serializes_legacy_and_v2_transition() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_physical_writer_lock")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    let guard = DerivedStoreWriteGuard::acquire(&temp.path, STORE)?;
    let error = result_err(begin_projection_generation(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("active physical writer"));
    drop(guard);
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    Ok(())
}

#[test]
fn abort_restores_pending_delivery_and_exact_checkpoint() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_abort_checkpoint")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let conn = connect_file(&temp.path)?;
    let checkpoint: i64 = conn.query_row(
        "SELECT checkpoint_cursor FROM projection_store_state WHERE store_name=?1",
        [STORE],
        |row| row.get(0),
    )?;
    assert_eq!(checkpoint, 10);
    drop(conn);

    abort_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let conn = connect_file(&temp.path)?;
    let (status, checkpoint): (String, i64) = conn.query_row(
        "SELECT d.status,s.checkpoint_cursor
         FROM projection_deliveries d
         JOIN projection_store_state s USING(store_name)
         WHERE d.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(status, "pending");
    assert_eq!(checkpoint, 0);
    drop(conn);
    assert_eq!(backend.inspect_generation(&generation.generation)?, None);
    assert!(
        backend
            .quarantined_generation(&generation.generation)
            .is_some(),
        "aborted physical evidence must remain in quarantine"
    );
    assert!(
        !doctor_database(&temp.path)?
            .consistency_issues
            .iter()
            .any(|issue| issue.code == "projection_checkpoint_discontinuous")
    );
    Ok(())
}

#[test]
fn abort_quarantines_mismatched_published_generation_before_sqlite_reset() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_abort_mismatched_published")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let prepared =
        prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.publish_generation(None, &prepared)?;
    backend.corrupt_active_generation_schema_version(&generation.generation);

    abort_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    assert_eq!(backend.inspect_active()?, None);
    assert_eq!(backend.inspect_generation(&generation.generation)?, None);
    assert!(
        backend
            .quarantined_generation(&generation.generation)
            .is_some(),
        "mismatched published evidence must remain in quarantine"
    );
    let status = projection_status(&temp.path)?;
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == STORE)
        .expect("Tantivy status");
    assert!(store.building_generation.is_none());
    Ok(())
}

#[test]
fn abort_quarantines_binding_mismatch_inspection_before_sqlite_reset() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_abort_binding_mismatch")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let prepared =
        prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.publish_generation(None, &prepared)?;
    backend.fail_active_inspection(FakeInspectFailure::BindingMismatch);

    abort_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    assert_eq!(backend.inspect_active()?, None);
    assert_eq!(backend.inspect_generation(&generation.generation)?, None);
    assert!(
        backend
            .quarantined_generation(&generation.generation)
            .is_some(),
        "binding-mismatched physical evidence must remain in quarantine"
    );
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("projection state");
    assert!(store.building_generation.is_none());
    Ok(())
}

#[test]
fn abort_keeps_exact_generation_and_sqlite_state_on_transient_inspect_failure() -> anyhow::Result<()>
{
    let temp = TempDb::new("projection_v2_abort_transient_inspect")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let prepared =
        prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.publish_generation(None, &prepared)?;
    backend.fail_active_inspection(FakeInspectFailure::TransientStorage);

    let error = result_err(abort_projection_generation(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("transient physical inspection"));
    assert!(
        backend
            .inspect_generation(&generation.generation)?
            .is_some()
    );
    assert_eq!(backend.quarantined_generation(&generation.generation), None);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("projection state");
    assert_eq!(
        store.building_generation.as_deref(),
        Some(generation.generation.as_str())
    );
    Ok(())
}

#[test]
fn database_replace_cannot_cross_an_active_projection_writer() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_replace_writer_barrier")?;
    init_database(&temp.path, "tester")?;
    let guard = DerivedStoreWriteGuard::acquire(&temp.path, STORE)?;
    let error = result_err(begin_database_replace(&temp.path))?;
    assert!(error.to_string().contains("active physical writer"));
    drop(guard);
    let replace = begin_database_replace(&temp.path)?;
    drop(replace);
    Ok(())
}

#[test]
fn snapshot_refuses_to_erase_a_running_claim() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_snapshot_running_claim")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 10_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    force_running_delivery(&temp.path, &lease, &generation.generation, 10)?;

    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("running delivery"));
    let conn = connect_file(&temp.path)?;
    let status: String = conn.query_row(
        "SELECT status FROM projection_deliveries WHERE store_name=?1 AND cursor=10",
        [STORE],
        |row| row.get(0),
    )?;
    assert_eq!(status, "running");
    Ok(())
}

#[test]
fn claim_ttl_cannot_outlive_lease_and_release_recovers_claim() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_claim_lease_bound")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 2_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    seed_delivery(&temp.path, 10)?;
    let error = result_err(run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        10_000,
        10,
        &backend,
    ))?;
    assert!(error.to_string().contains("cannot exceed"));

    force_running_delivery(&temp.path, &lease, &generation.generation, 10)?;
    release_projection_lease(&temp.path, STORE, "owner", &lease.lease_token)?;
    let conn = connect_file(&temp.path)?;
    let status: String = conn.query_row(
        "SELECT status FROM projection_deliveries WHERE store_name=?1 AND cursor=10",
        [STORE],
        |row| row.get(0),
    )?;
    assert_eq!(status, "pending");
    Ok(())
}

#[test]
fn batch_receipt_is_generation_and_fence_bound() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_batch_receipt")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::with_bad_batch_receipt();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    seed_delivery(&temp.path, 10)?;
    let error = result_err(run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &backend,
    ))?;
    assert!(error.to_string().contains("receipt mismatch"));
    let report = doctor_database(&temp.path)?;
    assert!(!report.ok);
    assert!(report.consistency_issues.iter().any(|issue| {
        issue.code == "projection_delivery_failed" || issue.code == "projection_store_error"
    }));
    release_projection_lease(&temp.path, STORE, "owner", &lease.lease_token)?;
    Ok(())
}

#[test]
fn every_physical_operation_revalidates_the_generation_provider() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_provider_binding")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let wrong_backend = FakeProjectionStore::with_provider("wrong", "wrong-v2");
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let error = result_err(prepare_projection_snapshot_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("provider binding"));
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    seed_delivery(&temp.path, 10)?;
    let error = result_err(run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("provider binding"));
    run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &backend,
    )?;

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &wrong_backend,
    ))?;
    assert!(error.to_string().contains("provider binding"));
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    Ok(())
}

#[test]
fn target_evidence_mismatch_is_rejected_before_delivery_claim() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_target_evidence")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.corrupt_generation_schema_version(&generation.generation);
    seed_delivery(&temp.path, 10)?;

    let error = result_err(run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &backend,
    ))?;
    assert!(error.to_string().contains("evidence does not match SQLite"));

    let conn = connect_file(&temp.path)?;
    let (status, attempts, claim_token, checkpoint): (String, i64, Option<String>, i64) = conn
        .query_row(
            "SELECT d.status,d.attempts,d.claim_token,s.checkpoint_cursor
             FROM projection_deliveries d
             JOIN projection_store_state s USING(store_name)
             WHERE d.store_name=?1 AND d.cursor=10",
            [STORE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
    assert_eq!(status, "pending");
    assert_eq!(attempts, 0);
    assert_eq!(claim_token, None);
    assert_eq!(checkpoint, 0);
    Ok(())
}

#[test]
fn building_generation_ack_does_not_clean_legacy_health_before_publish() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_building_legacy_health")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    connect_file(&temp.path)?.execute(
        "UPDATE derived_store_state SET dirty=1,last_event_id=0 WHERE store_name=?1",
        [STORE],
    )?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    seed_delivery(&temp.path, 20)?;
    run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &backend,
    )?;

    let conn = connect_file(&temp.path)?;
    let (outbox_status, dirty, last_event_id): (String, i64, i64) = conn.query_row(
        "SELECT o.status,s.dirty,s.last_event_id
         FROM index_outbox o
         JOIN derived_store_state s ON s.store_name=?1
         WHERE o.id=20",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(outbox_status, "pending");
    assert_eq!(dirty, 1);
    assert_eq!(last_event_id, 0);
    drop(conn);

    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let conn = connect_file(&temp.path)?;
    let (outbox_status, dirty): (String, i64) = conn.query_row(
        "SELECT o.status,s.dirty
         FROM index_outbox o
         JOIN derived_store_state s ON s.store_name=?1
         WHERE o.id=20",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(outbox_status, "done");
    assert_eq!(dirty, 0);
    Ok(())
}

#[test]
fn publish_accepts_a_post_snapshot_mutation_after_its_delivery_is_applied() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_publish_post_snapshot_mutation")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let generation =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("created after snapshot preparation"),
    )?;
    run_projection_batch_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        1_000,
        10,
        &backend,
    )?;
    backend.fail_active_contents_validation();

    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(generation.generation.as_str())
    );
    assert_eq!(store.building_generation, None);
    Ok(())
}

#[test]
fn publish_recovers_the_exact_prepared_marker_through_expected_active_cas() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_publish_repairs_prepared_marker")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let prepared =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.corrupt_publication_marker(&prepared.generation);

    let active = publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    assert_eq!(active.manifest.generation, prepared.generation);
    assert_eq!(backend.repair_calls(), 0);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(prepared.generation.as_str())
    );
    assert_eq!(store.building_generation, None);
    Ok(())
}

#[test]
fn publish_fails_closed_when_a_different_generations_marker_is_corrupt() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_publish_rejects_other_marker")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.corrupt_publication_marker(&first.generation);

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(
        error
            .to_string()
            .contains("does not own the corrupt marker")
    );
    assert_eq!(backend.repair_calls(), 0);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(
        store.building_generation.as_deref(),
        Some(second.generation.as_str())
    );
    Ok(())
}

#[test]
fn transient_active_inspection_never_calls_candidate_repair_or_confirms_sqlite()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_transient_inspect_no_candidate_repair")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.fail_active_inspection(FakeInspectFailure::TransientStorage);

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("transient physical inspection"));
    assert_eq!(backend.repair_calls(), 0);
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn publish_rejects_unexpected_x_when_the_prepared_b_marker_is_corrupt() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_publish_expected_predecessor_cas")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.install_unexpected_active("gen_unexpected_publish_x");
    backend.corrupt_publication_marker(&second.generation);

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("active generation CAS mismatch"));
    assert_eq!(backend.repair_calls(), 0);
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn reconcile_rejects_unexpected_x_when_the_prepared_b_marker_is_corrupt() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_reconcile_expected_predecessor_cas")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.install_unexpected_active("gen_unexpected_reconcile_x");
    backend.corrupt_publication_marker(&second.generation);

    let error = result_err(reconcile_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("active generation CAS mismatch"));
    assert_eq!(backend.repair_calls(), 0);
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn recovery_rejects_unexpected_x_when_the_prepared_b_marker_is_corrupt() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_expected_predecessor_cas")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.install_unexpected_active("gen_unexpected_recovery_x");
    backend.corrupt_publication_marker(&second.generation);

    let error = result_err(recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("active generation CAS mismatch"));
    assert_eq!(backend.repair_calls(), 0);
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn recovery_fails_closed_without_quarantining_an_unexpected_valid_x() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_rejects_valid_unexpected_x")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let unexpected = backend.install_unexpected_active("gen_unexpected_valid_recovery_x");

    let error = result_err(recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(
        error
            .to_string()
            .contains("unexpected physical predecessor")
    );
    assert_eq!(backend.repair_calls(), 0);
    assert_eq!(
        backend.quarantined_generation(&unexpected.manifest.generation),
        None,
        "unexpected valid physical generations require operator attribution"
    );
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn publish_keeps_previous_and_reconciles_a_corrupt_marker_after_pointer_swap() -> anyhow::Result<()>
{
    let temp = TempDb::new("projection_v2_publish_reconcile")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    seed_delivery(&temp.path, 10)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.fail_after_publish_once();
    let crash = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(crash.to_string().contains("pointer swap"));
    let abort = result_err(abort_projection_generation(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(abort.to_string().contains("physically active"));
    assert_eq!(
        backend.quarantined_generation(&second.generation),
        None,
        "exact published evidence must be reconciled, never quarantined"
    );
    connect_file(&temp.path)?.execute(
        "UPDATE projection_store_state SET lease_expires_at=0 WHERE store_name=?1",
        [STORE],
    )?;
    let takeover = acquire_projection_lease(&temp.path, STORE, "recovery-owner", 20_000)?;
    backend.corrupt_publication_marker(&second.generation);
    reconcile_projection_generation_with(
        &temp.path,
        STORE,
        "recovery-owner",
        &takeover.lease_token,
        &backend,
    )?;
    assert_eq!(backend.repair_calls(), 0);

    let status = projection_status(&temp.path)?;
    let store = status
        .stores
        .iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(second.generation.as_str())
    );
    assert_eq!(
        store.previous_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert!(
        backend.inspect_generation(&first.generation)?.is_some(),
        "previous physical generation must remain readable"
    );
    assert_eq!(store.control_plane, "v2");
    Ok(())
}

#[test]
fn recovery_recovers_the_exact_prepared_marker_through_expected_active_cas() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_repairs_prepared_marker")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.corrupt_publication_marker(&second.generation);

    let recovered = recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    assert_eq!(recovered.manifest.generation, second.generation);
    assert_eq!(backend.repair_calls(), 0);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(second.generation.as_str())
    );
    assert_eq!(
        store.previous_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(store.building_generation, None);
    Ok(())
}

#[test]
fn recovery_repairs_corrupt_logical_a_before_publishing_prepared_b() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_repairs_logical_a_first")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.corrupt_publication_marker(&first.generation);

    let recovered = recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    assert_eq!(recovered.manifest.generation, second.generation);
    assert_eq!(backend.repair_calls(), 1);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(second.generation.as_str())
    );
    assert_eq!(
        store.previous_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(store.building_generation, None);
    Ok(())
}

#[test]
fn recovery_fails_closed_on_a_corrupt_third_generation_before_publishing_b() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_rejects_corrupt_third")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let unexpected = backend.install_unexpected_active("gen_corrupt_recovery_x");
    backend.corrupt_publication_marker(&unexpected.manifest.generation);

    let error = result_err(recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(
        error
            .to_string()
            .contains("does not own the corrupt marker")
    );
    assert_eq!(backend.repair_calls(), 0);
    assert_sqlite_active_and_building(&temp.path, &first.generation, &second.generation)?;
    Ok(())
}

#[test]
fn corrupt_candidate_publication_state_cannot_be_confirmed_after_pointer_swap() -> anyhow::Result<()>
{
    let temp = TempDb::new("projection_v2_rejects_corrupt_candidate_publication")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.fail_after_publish_once();
    let crash = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(crash.to_string().contains("pointer swap"));
    backend.fail_publication_validation(&second.generation);

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("auxiliary publication state"));
    assert_eq!(backend.repair_calls(), 0);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(
        store.building_generation.as_deref(),
        Some(second.generation.as_str())
    );
    Ok(())
}

#[test]
fn corrupt_previous_publication_state_cannot_be_confirmed_after_pointer_swap() -> anyhow::Result<()>
{
    let temp = TempDb::new("projection_v2_rejects_corrupt_previous_publication")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.fail_after_publish_once();
    let crash = result_err(publish_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(crash.to_string().contains("pointer swap"));
    backend.fail_publication_validation(&first.generation);

    let error = result_err(reconcile_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(error.to_string().contains("auxiliary publication state"));
    assert_eq!(backend.repair_calls(), 0);
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(
        store.building_generation.as_deref(),
        Some(second.generation.as_str())
    );
    Ok(())
}

#[test]
fn recovery_preserves_logical_active_on_transient_generation_inspection_failure()
-> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_recovery_transient_inspect")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    let first =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;

    seed_delivery(&temp.path, 10)?;
    let second =
        begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    backend.fail_generation_inspection(&first.generation, FakeInspectFailure::TransientStorage);

    let error = result_err(recover_projection_generation_with(
        &temp.path,
        STORE,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;

    assert!(
        error
            .to_string()
            .contains("transient generation inspection")
    );
    backend.clear_inspection_failures();
    assert!(backend.inspect_generation(&first.generation)?.is_some());
    assert!(backend.inspect_generation(&second.generation)?.is_some());
    assert_eq!(backend.quarantined_generation(&first.generation), None);
    assert_eq!(
        backend
            .inspect_active()?
            .map(|evidence| evidence.manifest.generation),
        Some(first.generation.clone())
    );
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("projection state");
    assert_eq!(
        store.active_generation.as_deref(),
        Some(first.generation.as_str())
    );
    assert_eq!(
        store.building_generation.as_deref(),
        Some(second.generation.as_str())
    );
    Ok(())
}

#[test]
fn label_atoms_enter_v2_after_mutation_delivery_migration() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atoms_enabled")?;
    init_database(&temp.path, "tester")?;
    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 10_000)?;
    let generation = begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    let store = projection_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == LABEL_ATOMS)
        .expect("label atom projection state");
    assert_eq!(store.control_plane, "v2");
    assert_eq!(
        store.building_generation.as_deref(),
        Some(generation.generation.as_str())
    );
    abort_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    Ok(())
}

#[test]
fn label_atom_v2_batch_ack_cleans_only_the_claimed_drained_boards() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_batch_ack_board_isolation")?;
    init_database(&temp.path, "tester")?;
    for index in 1..9 {
        create_board(
            &temp.path,
            "tester",
            CreateBoard {
                slug: format!("label-board-{index}"),
                name: format!("Label board {index}"),
                description: None,
            },
        )?;
    }
    let conn = connect_file(&temp.path)?;
    let boards = {
        let mut statement = conn.prepare("SELECT id FROM boards ORDER BY slug LIMIT 9")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(boards.len(), 9);
    for (index, board_id) in boards.iter().enumerate() {
        conn.execute(
            "INSERT INTO label_atom_index_boards(
               store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
             ) VALUES(?1,?2,0,NULL,NULL,?3)",
            params![LABEL_ATOMS, board_id, 100 + index as i64],
        )?;
    }
    drop(conn);

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    let conn = connect_file(&temp.path)?;
    for (index, board_id) in boards.iter().enumerate() {
        conn.execute(
            "UPDATE label_atom_index_boards
             SET dirty=1,last_error='preexisting provider diagnostic',updated_at=?1
             WHERE store_name=?2 AND board_id=?3",
            params![200 + index as i64, LABEL_ATOMS, board_id],
        )?;
    }
    drop(conn);

    let batch = run_projection_batch_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        1_000,
        4,
        &backend,
    )?;
    assert_eq!(batch.items.len(), 4);
    let claimed_boards = batch
        .items
        .iter()
        .map(|item| item.board_id.as_str())
        .collect::<BTreeSet<_>>();

    let conn = connect_file(&temp.path)?;
    for board_id in &boards {
        let (dirty, last_rebuild_at, last_error, updated_at): (
            bool,
            Option<i64>,
            Option<String>,
            i64,
        ) = conn.query_row(
            "SELECT dirty,last_rebuild_at,last_error,updated_at
             FROM label_atom_index_boards
             WHERE store_name=?1 AND board_id=?2",
            params![LABEL_ATOMS, board_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        if claimed_boards.contains(board_id.as_str()) {
            assert!(!dirty, "claimed and drained board must become clean");
            assert!(
                last_rebuild_at.is_some(),
                "compatibility generation must advance on successful batch ack"
            );
            assert_eq!(last_error, None);
            assert_eq!(updated_at, last_rebuild_at.expect("compat generation"));
        } else {
            assert!(dirty, "unclaimed board must remain dirty");
            assert_eq!(last_rebuild_at, None);
            assert_eq!(
                last_error.as_deref(),
                Some("preexisting provider diagnostic")
            );
        }
    }
    Ok(())
}

#[test]
fn label_atom_v2_batch_ack_does_not_clear_a_concurrent_canonical_mutation() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_batch_ack_concurrent_mutation")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("initial semantics".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "UPDATE label_atom_index_boards
         SET dirty=0,last_rebuild_at=101,last_error=NULL,updated_at=101
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
    )?;
    drop(conn);
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("mutation claimed by this batch".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    backend.mutate_label_semantics_during_next_batch(
        &temp.path,
        "default",
        "backend",
        "newer mutation created while provider applies the batch",
    );

    let batch = run_projection_batch_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        1_000,
        1,
        &backend,
    )?;
    assert_eq!(batch.items.len(), 1);

    let conn = connect_file(&temp.path)?;
    let board_state: (bool, Option<i64>, Option<String>) = conn.query_row(
        "SELECT dirty,last_rebuild_at,last_error
         FROM label_atom_index_boards
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(board_state, (true, Some(101), None));
    let incomplete: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2 AND status!='done'",
        params![LABEL_ATOMS, board_id],
        |row| row.get(0),
    )?;
    assert_eq!(incomplete, 1, "new canonical work must remain pending");
    Ok(())
}

#[test]
fn label_atom_v2_batch_ack_failure_preserves_board_dirty_error_state() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_batch_ack_failure_state")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES(?1,?2,0,101,NULL,101)",
        params![LABEL_ATOMS, board_id],
    )?;
    drop(conn);

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    connect_file(&temp.path)?.execute(
        "UPDATE label_atom_index_boards
         SET dirty=1,last_error='provider unavailable',updated_at=202
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
    )?;
    backend.reject_next_batch_receipt();

    let error = result_err(run_projection_batch_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        1_000,
        1,
        &backend,
    ))?;
    assert!(error.to_string().contains("receipt mismatch"));

    let conn = connect_file(&temp.path)?;
    let board_state: (bool, Option<i64>, Option<String>) = conn.query_row(
        "SELECT dirty,last_rebuild_at,last_error
         FROM label_atom_index_boards
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(
        board_state,
        (true, Some(101), Some("provider unavailable".to_owned()))
    );
    let failed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2 AND status='failed'",
        params![LABEL_ATOMS, board_id],
        |row| row.get(0),
    )?;
    assert_eq!(failed, 1);
    Ok(())
}

#[test]
fn label_atom_v2_snapshot_publish_cleans_nine_drained_boards() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_snapshot_publish_nine_boards")?;
    init_database(&temp.path, "tester")?;
    for index in 1..9 {
        create_board(
            &temp.path,
            "tester",
            CreateBoard {
                slug: format!("snapshot-label-board-{index}"),
                name: format!("Snapshot label board {index}"),
                description: None,
            },
        )?;
    }
    let conn = connect_file(&temp.path)?;
    let boards = {
        let mut statement = conn.prepare("SELECT id FROM boards ORDER BY slug LIMIT 9")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    assert_eq!(boards.len(), 9);
    for (index, board_id) in boards.iter().enumerate() {
        conn.execute(
            "INSERT INTO label_atom_index_boards(
               store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
             ) VALUES(?1,?2,1,NULL,'snapshot pending',?3)",
            params![LABEL_ATOMS, board_id, 300 + index as i64],
        )?;
    }
    drop(conn);

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    let prepared = prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    let conn = connect_file(&temp.path)?;
    let dirty_before_publish: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_atom_index_boards
         WHERE store_name=?1 AND dirty=1 AND last_rebuild_at IS NULL
           AND last_error='snapshot pending'",
        [LABEL_ATOMS],
        |row| row.get(0),
    )?;
    let snapshot_done: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND published_generation=?2 AND status='done'",
        params![LABEL_ATOMS, prepared.manifest.generation],
        |row| row.get(0),
    )?;
    assert_eq!(
        dirty_before_publish, 9,
        "prepare coverage must not claim an unpublished generation is ready"
    );
    assert_eq!(snapshot_done, 9);
    drop(conn);

    publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;

    let conn = connect_file(&temp.path)?;
    for board_id in boards {
        let state: (bool, Option<i64>, Option<String>, i64) = conn.query_row(
            "SELECT dirty,last_rebuild_at,last_error,updated_at
             FROM label_atom_index_boards
             WHERE store_name=?1 AND board_id=?2",
            params![LABEL_ATOMS, board_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        assert!(!state.0);
        assert!(state.1.is_some());
        assert_eq!(state.2, None);
        assert_eq!(state.3, state.1.expect("compat generation"));
    }
    Ok(())
}

#[test]
fn label_atom_v2_snapshot_publish_failure_keeps_board_dirty_error_state() -> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_snapshot_publish_failure")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(
           store_name,board_id,dirty,last_rebuild_at,last_error,updated_at
         ) VALUES(?1,?2,1,NULL,'snapshot provider failed',401)",
        params![LABEL_ATOMS, board_id],
    )?;
    drop(conn);

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    backend.fail_after_publish_once();
    let error = result_err(publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("pointer swap"));

    let state: (bool, Option<i64>, Option<String>) = connect_file(&temp.path)?.query_row(
        "SELECT dirty,last_rebuild_at,last_error
         FROM label_atom_index_boards
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(
        state,
        (true, None, Some("snapshot provider failed".to_owned()))
    );
    Ok(())
}

#[test]
fn label_atom_v2_snapshot_confirm_rejects_and_preserves_a_concurrent_canonical_mutation()
-> anyhow::Result<()> {
    const LABEL_ATOMS: &str = "lancedb_label_atoms";
    let temp = TempDb::new("projection_v2_label_atom_snapshot_concurrent_mutation")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("snapshot semantics".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    drop(conn);

    let backend = FakeProjectionStore::for_store(LABEL_ATOMS);
    let lease = acquire_projection_lease(&temp.path, LABEL_ATOMS, "owner", 20_000)?;
    begin_projection_generation(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    prepare_projection_snapshot_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    )?;
    backend.mutate_label_semantics_during_next_publish(
        &temp.path,
        "default",
        "backend",
        "newer mutation committed after physical publish but before SQLite confirm",
    );

    let error = result_err(publish_projection_generation_with(
        &temp.path,
        LABEL_ATOMS,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("catch-up is incomplete"));

    let conn = connect_file(&temp.path)?;
    let state: (bool, Option<i64>) = conn.query_row(
        "SELECT dirty,last_rebuild_at
         FROM label_atom_index_boards
         WHERE store_name=?1 AND board_id=?2",
        params![LABEL_ATOMS, board_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(state, (true, None));
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projection_deliveries
         WHERE store_name=?1 AND board_id=?2 AND status='pending'",
        params![LABEL_ATOMS, board_id],
        |row| row.get(0),
    )?;
    assert_eq!(pending, 1);
    Ok(())
}

#[test]
fn oxigraph_snapshot_and_doctor_reject_cross_board_relations() -> anyhow::Result<()> {
    const OXIGRAPH: &str = "oxigraph_relations";
    let temp = TempDb::new("projection_v2_relation_board_scope")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let first_board: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO boards(id,slug,name,created_at,updated_at)
         VALUES('b_projection_second','projection-second','Second',1,1)",
        [],
    )?;
    conn.execute(
        "INSERT INTO entities(uri,kind,source_table,source_id,board_id,created_at,updated_at)
         VALUES('kb://projection/subject','fixture','fixture','subject',?1,1,1)",
        [&first_board],
    )?;
    conn.execute(
        "INSERT INTO entities(uri,kind,source_table,source_id,board_id,created_at,updated_at)
         VALUES('kb://projection/object','fixture','fixture','object','b_projection_second',1,1)",
        [],
    )?;
    conn.execute(
        "INSERT INTO entity_relations(
           subject_uri,predicate,object_uri,graph_uri,authoritative_store,
           metadata_json,created_at,updated_at
         ) VALUES(
           'kb://projection/subject','related_to','kb://projection/object',
           'kb://graph/indexed','sqlite','{}',1,1
         )",
        [],
    )?;
    drop(conn);

    let report = doctor_database(&temp.path)?;
    assert!(report.consistency_issues.iter().any(|issue| {
        issue.code == "projection_relation_board_mismatch" && issue.severity == "error"
    }));

    let backend = FakeProjectionStore::for_store(OXIGRAPH);
    let lease = acquire_projection_lease(&temp.path, OXIGRAPH, "owner", 10_000)?;
    let error = result_err(begin_projection_generation(
        &temp.path,
        OXIGRAPH,
        "owner",
        &lease.lease_token,
        &backend,
    ))?;
    assert!(error.to_string().contains("cross-board relation"));
    Ok(())
}

#[test]
fn legacy_done_is_not_v2_coverage_and_v2_publish_bridges_old_health() -> anyhow::Result<()> {
    let temp = TempDb::new("projection_v2_legacy_bridge")?;
    init_database(&temp.path, "tester")?;
    seed_delivery(&temp.path, 10)?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE index_outbox SET status='done',updated_at=2 WHERE id=10",
        [],
    )?;
    let (delivery_status, checkpoint): (String, i64) = conn.query_row(
        "SELECT d.status,s.checkpoint_cursor \
         FROM projection_deliveries d JOIN projection_store_state s USING(store_name) \
         WHERE d.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(delivery_status, "legacy_done");
    assert_eq!(checkpoint, 0);
    drop(conn);
    init_database(&temp.path, "tester")?;
    let status = projection_status(&temp.path)?;
    let legacy = status
        .stores
        .iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(legacy.legacy_checkpoint_cursor, 10);
    assert_eq!(legacy.checkpoint_cursor, 0);

    let backend = FakeProjectionStore::default();
    let lease = acquire_projection_lease(&temp.path, STORE, "owner", 20_000)?;
    begin_projection_generation(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    prepare_projection_snapshot_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    publish_projection_generation_with(&temp.path, STORE, "owner", &lease.lease_token, &backend)?;
    let conn = connect_file(&temp.path)?;
    let (dirty, delivery_status, checkpoint): (i64, String, i64) = conn.query_row(
        "SELECT legacy.dirty,d.status,s.checkpoint_cursor \
         FROM derived_store_state legacy \
         JOIN projection_store_state s USING(store_name) \
         JOIN projection_deliveries d USING(store_name) \
         WHERE s.store_name=?1 AND d.cursor=10",
        [STORE],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(dirty, 0);
    assert_eq!(delivery_status, "done");
    assert_eq!(checkpoint, 10);
    Ok(())
}

#[test]
fn doctor_requires_projection_tables_and_checks_claim_fencing() -> anyhow::Result<()> {
    let missing = TempDb::new("projection_v2_doctor_missing")?;
    init_database(&missing.path, "tester")?;
    connect_file(&missing.path)?
        .execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE projection_deliveries;")?;
    let report = doctor_database(&missing.path)?;
    assert!(!report.ok);

    let corrupt = TempDb::new("projection_v2_doctor_claim")?;
    init_database(&corrupt.path, "tester")?;
    seed_delivery(&corrupt.path, 10)?;
    let lease = acquire_projection_lease(&corrupt.path, STORE, "owner", 10_000)?;
    let backend = FakeProjectionStore::default();
    let generation =
        begin_projection_generation(&corrupt.path, STORE, "owner", &lease.lease_token, &backend)?;
    force_running_delivery(&corrupt.path, &lease, &generation.generation, 10)?;
    let conn = connect_file(&corrupt.path)?;
    conn.execute(
        "UPDATE projection_deliveries SET claim_fence_epoch=claim_fence_epoch+1 \
         WHERE store_name=?1 AND cursor=10",
        [STORE],
    )?;
    drop(conn);
    let report = doctor_database(&corrupt.path)?;
    assert!(!report.ok);
    assert!(report.consistency_issues.iter().any(|issue| {
        issue.code == "projection_claim_fence_mismatch" && issue.severity == "error"
    }));

    let conn = connect_file(&corrupt.path)?;
    conn.execute(
        "UPDATE projection_store_state \
         SET lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL \
         WHERE store_name=?1",
        [STORE],
    )?;
    drop(conn);
    let report = doctor_database(&corrupt.path)?;
    assert!(report.consistency_issues.iter().any(|issue| {
        issue.code == "projection_claim_fence_mismatch" && issue.severity == "error"
    }));

    let conn = connect_file(&corrupt.path)?;
    conn.execute(
        "UPDATE projection_deliveries \
         SET claim_expires_at=0 \
         WHERE store_name=?1 AND cursor=10",
        [STORE],
    )?;
    drop(conn);
    let report = doctor_database(&corrupt.path)?;
    assert!(
        report
            .consistency_issues
            .iter()
            .any(|issue| issue.code == "projection_claim_expired" && issue.severity == "error")
    );

    let conn = connect_file(&corrupt.path)?;
    conn.execute(
        "UPDATE projection_store_state SET checkpoint_cursor=999 WHERE store_name=?1",
        [STORE],
    )?;
    drop(conn);
    let report = doctor_database(&corrupt.path)?;
    assert!(report.consistency_issues.iter().any(|issue| {
        issue.code == "projection_checkpoint_discontinuous" && issue.severity == "error"
    }));
    Ok(())
}

fn seed_delivery(path: &Path, outbox_id: i64) -> anyhow::Result<()> {
    let conn = connect_file(path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO index_outbox(\
           id,source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,\
           created_at,updated_at\
         ) VALUES(?1,NULL,'tantivy',?2,'upsert','{}','pending',0,NULL,1,1)",
        params![outbox_id, format!("kb://board/{board_id}")],
    )?;
    Ok(())
}

fn assert_sqlite_active_and_building(
    path: &Path,
    active_generation: &str,
    building_generation: &str,
) -> anyhow::Result<()> {
    let store = projection_status(path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == STORE)
        .expect("tantivy state");
    assert_eq!(store.active_generation.as_deref(), Some(active_generation));
    assert_eq!(
        store.building_generation.as_deref(),
        Some(building_generation)
    );
    Ok(())
}

fn force_running_delivery(
    path: &Path,
    lease: &kanban_sqlite::api::ProjectionLease,
    generation: &str,
    cursor: i64,
) -> anyhow::Result<()> {
    let conn = connect_file(path)?;
    conn.execute(
        "UPDATE projection_deliveries \
         SET status='running',claim_owner='owner',claim_token='pclaim_fixture',\
             claim_lease_token=?1,claim_fence_epoch=?2,claim_generation=?3,\
             claim_expires_at=?4 \
         WHERE store_name=?5 AND cursor=?6",
        params![
            lease.lease_token,
            lease.fence_epoch,
            generation,
            lease.lease_expires_at,
            STORE,
            cursor
        ],
    )?;
    Ok(())
}
