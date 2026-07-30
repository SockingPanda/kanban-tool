use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use kanban_contract::{
    ProjectionArtifactEvidence as WireProjectionArtifactEvidence,
    ProjectionArtifactManifest as WireProjectionArtifactManifest,
    ProjectionBatch as WireProjectionBatch, ProjectionBatchReceipt as WireProjectionBatchReceipt,
    ProjectionDelivery as WireProjectionDelivery,
    ProjectionDeliveryAction as WireProjectionDeliveryAction,
    ProjectionPublishReceipt as WireProjectionPublishReceipt,
    ProjectionSnapshot as WireProjectionSnapshot,
    ProjectionSnapshotRecord as WireProjectionSnapshotRecord,
    ProjectionStoreDescriptor as WireProjectionStoreDescriptor, VECTOR_PROJECTION_PROTOCOL_VERSION,
    VectorProjectionApplyBatchRequest, VectorProjectionCleanupProtection,
    VectorProjectionCleanupRequest, VectorProjectionCleanupResponse,
    VectorProjectionDescriptorRequest, VectorProjectionGenerationMutationRequest,
    VectorProjectionHelperDescriptor, VectorProjectionHelperErrorKind,
    VectorProjectionHelperOperation, VectorProjectionHelperRequest, VectorProjectionHelperResponse,
    VectorProjectionInspectActiveRequest, VectorProjectionInspectGenerationRequest,
    VectorProjectionInventoryRequest, VectorProjectionInventoryResponse,
    VectorProjectionMutationAck, VectorProjectionMutationContext,
    VectorProjectionPrepareSnapshotRequest, VectorProjectionPublishRequest,
    VectorProjectionRepairPublicationRequest, VectorProjectionValidateActiveRequest,
    VectorProjectionValidateGenerationRequest, VectorProjectionValidationResponse,
};
use kanban_core::{KanbanError, Result, new_typed_id};
use kanban_indexer::{
    DERIVED_STORE_SCHEMA_VERSION, LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE,
};
use kanban_vector::{
    LABEL_ATOMS_CORPUS_SCHEMA, SubprocessVectorProjectionClient, TASK_CHUNKS_CORPUS_SCHEMA,
    VectorError, corpus_provider_fingerprint, embedding_provider_fingerprint,
    validate_projection_request_against_descriptor,
};

use super::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor,
};

const VECTOR_PROJECTION_HELPER: &str = "kanban-vector-lancedb";
const VECTOR_PROJECTION_HELPER_ENV: &str = "KANBAN_VECTOR_HELPER";

const REQUIRED_OPERATIONS: [VectorProjectionHelperOperation; 13] = [
    VectorProjectionHelperOperation::Descriptor,
    VectorProjectionHelperOperation::PrepareSnapshot,
    VectorProjectionHelperOperation::ApplyBatch,
    VectorProjectionHelperOperation::Publish,
    VectorProjectionHelperOperation::InspectActive,
    VectorProjectionHelperOperation::InspectGeneration,
    VectorProjectionHelperOperation::ValidateGenerationPublication,
    VectorProjectionHelperOperation::ValidateActiveContents,
    VectorProjectionHelperOperation::RepairPublication,
    VectorProjectionHelperOperation::Quarantine,
    VectorProjectionHelperOperation::Abort,
    VectorProjectionHelperOperation::Inventory,
    VectorProjectionHelperOperation::Cleanup,
];

trait VectorProjectionTransport: Send + Sync {
    fn execute(
        &self,
        request: &VectorProjectionHelperRequest,
    ) -> std::result::Result<VectorProjectionHelperResponse, VectorError>;
}

impl VectorProjectionTransport for SubprocessVectorProjectionClient {
    fn execute(
        &self,
        request: &VectorProjectionHelperRequest,
    ) -> std::result::Result<VectorProjectionHelperResponse, VectorError> {
        SubprocessVectorProjectionClient::execute(self, request)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LanceDbProjectionFailureClass {
    Provider,
    Backend,
    Delivery,
}

/// SQLite-owned adapter for one of the two LanceDB Projection v2 corpora.
///
/// The helper descriptor is pinned for the lifetime of this value. Every
/// subsequent request is validated against that descriptor before the opaque
/// subprocess transport sees it.
#[derive(Clone)]
pub(crate) struct LanceDbProjectionStore {
    db_path: PathBuf,
    transport: Arc<dyn VectorProjectionTransport>,
    helper_descriptor: VectorProjectionHelperDescriptor,
    store_descriptor: WireProjectionStoreDescriptor,
    generation_digests: Arc<Mutex<BTreeMap<String, String>>>,
}

impl fmt::Debug for LanceDbProjectionStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LanceDbProjectionStore")
            .field("db_path", &self.db_path)
            .field("build_identity", &self.helper_descriptor.build_identity)
            .field("store_name", &self.store_descriptor.store_name)
            .field("schema_version", &self.store_descriptor.schema_version)
            .field("provider", &self.store_descriptor.provider)
            .field(
                "provider_fingerprint",
                &self.store_descriptor.provider_fingerprint,
            )
            .finish_non_exhaustive()
    }
}

impl LanceDbProjectionStore {
    pub(crate) fn connect_resolved(db_path: impl AsRef<Path>, store_name: &str) -> Result<Self> {
        Self::connect(
            resolve_vector_projection_helper(),
            db_path,
            None::<PathBuf>,
            store_name,
        )
    }

    pub(crate) fn connect(
        helper_path: impl Into<PathBuf>,
        db_path: impl AsRef<Path>,
        vector_config_path: Option<impl Into<PathBuf>>,
        store_name: &str,
    ) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let client = SubprocessVectorProjectionClient::new(
            helper_path,
            &db_path,
            vector_config_path.map(Into::into),
        );
        Self::connect_transport(Arc::new(client), db_path, store_name)
    }

    fn connect_transport(
        transport: Arc<dyn VectorProjectionTransport>,
        db_path: PathBuf,
        store_name: &str,
    ) -> Result<Self> {
        let request_id = next_request_id();
        let request =
            VectorProjectionHelperRequest::Descriptor(VectorProjectionDescriptorRequest {
                request_id: request_id.clone(),
            });
        let response = transport
            .execute(&request)
            .map_err(|error| projection_transport_error("descriptor", error))?;
        let VectorProjectionHelperResponse::Descriptor(helper_descriptor) = response else {
            return Err(KanbanError::Storage(
                "LanceDB projection helper returned the wrong descriptor response operation"
                    .to_owned(),
            ));
        };
        if helper_descriptor.request_id != request_id {
            return Err(KanbanError::Storage(
                "LanceDB projection helper descriptor correlation does not match the request"
                    .to_owned(),
            ));
        }
        validate_projection_request_against_descriptor(&request, &helper_descriptor)
            .map_err(|error| projection_transport_error("descriptor validation", error))?;
        let store_descriptor = validate_helper_descriptor(store_name, &helper_descriptor)?.clone();
        Ok(Self {
            db_path,
            transport,
            helper_descriptor,
            store_descriptor,
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    #[cfg(test)]
    pub(crate) fn wire_descriptor(&self) -> &WireProjectionStoreDescriptor {
        &self.store_descriptor
    }

    pub(crate) fn execute_checked(
        &self,
        action: &str,
        request: VectorProjectionHelperRequest,
    ) -> Result<VectorProjectionHelperResponse> {
        validate_projection_request_against_descriptor(&request, &self.helper_descriptor)
            .map_err(|error| projection_transport_error(action, error))?;
        self.transport
            .execute(&request)
            .map_err(|error| projection_transport_error(action, error))
    }

    pub(crate) fn prepare_wire_snapshot(
        &self,
        snapshot: &WireProjectionSnapshot,
    ) -> Result<WireProjectionArtifactEvidence> {
        let metadata = snapshot.manifest.corpus.clone().ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "LanceDB projection snapshot for {} has no corpus binding",
                snapshot.manifest.store_name
            ))
        })?;
        let context = mutation_context(
            &snapshot.manifest.store_name,
            &snapshot.manifest.generation,
            &snapshot.manifest.delivery_digest,
        )?;
        self.remember_generation_digest(
            &snapshot.manifest.generation,
            &snapshot.manifest.delivery_digest,
        );
        let response = self.execute_checked(
            "prepare snapshot",
            VectorProjectionHelperRequest::PrepareSnapshot(
                VectorProjectionPrepareSnapshotRequest {
                    context: context.clone(),
                    snapshot: snapshot.clone(),
                    metadata,
                },
            ),
        )?;
        let VectorProjectionHelperResponse::PrepareSnapshot(response) = response else {
            return wrong_operation("prepare snapshot");
        };
        require_ack("prepare snapshot", &context, &response.ack)?;
        let mut prepared_manifest = response.evidence.manifest.clone();
        let physical_fingerprint = response.evidence.fingerprint.as_str();
        if physical_fingerprint.trim().is_empty()
            || prepared_manifest.fingerprint.as_deref() != Some(physical_fingerprint)
        {
            return Err(KanbanError::Storage(
                "LanceDB projection prepare snapshot evidence does not match the request"
                    .to_owned(),
            ));
        }
        prepared_manifest.fingerprint = None;
        if prepared_manifest != snapshot.manifest {
            return Err(KanbanError::Storage(
                "LanceDB projection prepare snapshot evidence does not match the request"
                    .to_owned(),
            ));
        }
        self.require_evidence_binding("prepare snapshot", &response.evidence)?;
        Ok(response.evidence)
    }

    /// Apply a wire batch without accepting capability tokens from stdout.
    ///
    /// The returned in-process receipt reconstructs both tokens exclusively
    /// from the original request.
    pub(crate) fn apply_wire_batch(
        &self,
        batch: &WireProjectionBatch,
        delivery_digest: &str,
    ) -> Result<WireProjectionBatchReceipt> {
        let context =
            mutation_context(&batch.store_name, &batch.target_generation, delivery_digest)?;
        self.remember_generation_digest(&batch.target_generation, delivery_digest);
        let response = self.execute_checked(
            "apply batch",
            VectorProjectionHelperRequest::ApplyBatch(VectorProjectionApplyBatchRequest {
                context: context.clone(),
                batch: batch.clone(),
            }),
        )?;
        let VectorProjectionHelperResponse::ApplyBatch(response) = response else {
            return wrong_operation("apply batch");
        };
        require_ack("apply batch", &context, &response.ack)?;
        let receipt = response.receipt;
        if receipt.store_name != batch.store_name
            || receipt.database_instance_id != batch.database_instance_id
            || receipt.protocol_version != batch.protocol_version
            || receipt.schema_version != batch.schema_version
            || receipt.provider != batch.provider
            || receipt.provider_fingerprint != batch.provider_fingerprint
            || receipt.target_generation != batch.target_generation
            || receipt.fence_epoch != batch.fence_epoch
            || receipt.applied_item_count != batch.items.len()
        {
            return Err(KanbanError::Storage(
                "LanceDB projection apply batch receipt does not match the request".to_owned(),
            ));
        }
        Ok(WireProjectionBatchReceipt {
            store_name: receipt.store_name,
            database_instance_id: receipt.database_instance_id,
            protocol_version: receipt.protocol_version,
            schema_version: receipt.schema_version,
            provider: receipt.provider,
            provider_fingerprint: receipt.provider_fingerprint,
            target_generation: receipt.target_generation,
            lease_token: batch.lease_token.clone(),
            fence_epoch: receipt.fence_epoch,
            claim_token: batch.claim_token.clone(),
            applied_item_count: receipt.applied_item_count,
        })
    }

    pub(crate) fn publish_wire_generation(
        &self,
        expected_active: Option<&WireProjectionArtifactEvidence>,
        prepared: &WireProjectionArtifactEvidence,
    ) -> Result<WireProjectionPublishReceipt> {
        self.require_evidence_binding("publish generation", prepared)?;
        if let Some(expected_active) = expected_active {
            self.require_evidence_binding("publish previous generation", expected_active)?;
        }
        let context = mutation_context(
            &prepared.manifest.store_name,
            &prepared.manifest.generation,
            &prepared.manifest.delivery_digest,
        )?;
        self.remember_generation_digest(
            &prepared.manifest.generation,
            &prepared.manifest.delivery_digest,
        );
        let response = self.execute_checked(
            "publish generation",
            VectorProjectionHelperRequest::Publish(Box::new(VectorProjectionPublishRequest {
                context: context.clone(),
                expected_active: expected_active.cloned(),
                prepared: prepared.clone(),
            })),
        )?;
        let VectorProjectionHelperResponse::Publish(response) = response else {
            return wrong_operation("publish generation");
        };
        require_ack("publish generation", &context, &response.ack)?;
        if response.receipt.active != *prepared
            || response.receipt.retained_previous.as_ref() != expected_active
        {
            return Err(KanbanError::Storage(
                "LanceDB projection publish receipt does not match the requested generation"
                    .to_owned(),
            ));
        }
        Ok(response.receipt)
    }

    pub(crate) fn inspect_wire_active(&self) -> Result<Option<WireProjectionArtifactEvidence>> {
        let request_id = next_request_id();
        let response = self.execute_checked(
            "inspect active",
            VectorProjectionHelperRequest::InspectActive(VectorProjectionInspectActiveRequest {
                request_id: request_id.clone(),
                projection_store: self.store_descriptor.store_name.clone(),
            }),
        )?;
        let VectorProjectionHelperResponse::InspectActive(response) = response else {
            return wrong_operation("inspect active");
        };
        if response.request_id != request_id
            || response.projection_store != self.store_descriptor.store_name
        {
            return Err(KanbanError::Storage(
                "LanceDB projection inspect active correlation mismatch".to_owned(),
            ));
        }
        if let Some(active) = &response.active {
            self.remember_generation_digest(
                &active.manifest.generation,
                &active.manifest.delivery_digest,
            );
            self.require_evidence_binding("inspect active", active)?;
        }
        Ok(response.active)
    }

    pub(crate) fn inspect_wire_generation(
        &self,
        generation_id: &str,
    ) -> Result<Option<WireProjectionArtifactEvidence>> {
        let request_id = next_request_id();
        let response = self.execute_checked(
            "inspect generation",
            VectorProjectionHelperRequest::InspectGeneration(
                VectorProjectionInspectGenerationRequest {
                    request_id: request_id.clone(),
                    projection_store: self.store_descriptor.store_name.clone(),
                    generation_id: generation_id.to_owned(),
                },
            ),
        )?;
        let VectorProjectionHelperResponse::InspectGeneration(response) = response else {
            return wrong_operation("inspect generation");
        };
        if response.request_id != request_id
            || response.projection_store != self.store_descriptor.store_name
            || response.generation_id != generation_id
        {
            return Err(KanbanError::Storage(
                "LanceDB projection inspect generation correlation mismatch".to_owned(),
            ));
        }
        if let Some(evidence) = &response.evidence {
            self.remember_generation_digest(
                &evidence.manifest.generation,
                &evidence.manifest.delivery_digest,
            );
            if evidence.manifest.generation != generation_id {
                return Err(KanbanError::Storage(
                    "LanceDB projection inspect generation returned another generation".to_owned(),
                ));
            }
            self.require_evidence_binding("inspect generation", evidence)?;
        }
        Ok(response.evidence)
    }

    pub(crate) fn validate_wire_generation_publication(
        &self,
        expected: &WireProjectionArtifactEvidence,
    ) -> Result<()> {
        self.require_evidence_binding("validate generation", expected)?;
        let request_id = next_request_id();
        let response = self.execute_checked(
            "validate generation",
            VectorProjectionHelperRequest::ValidateGenerationPublication(
                VectorProjectionValidateGenerationRequest {
                    request_id: request_id.clone(),
                    projection_store: self.store_descriptor.store_name.clone(),
                    expected: expected.clone(),
                },
            ),
        )?;
        let VectorProjectionHelperResponse::ValidateGenerationPublication(response) = response
        else {
            return wrong_operation("validate generation");
        };
        require_validation(
            "generation publication",
            &request_id,
            &self.store_descriptor.store_name,
            response,
        )
    }

    pub(crate) fn validate_wire_active_contents(
        &self,
        active: &WireProjectionArtifactEvidence,
    ) -> Result<()> {
        self.require_evidence_binding("validate active", active)?;
        let request_id = next_request_id();
        let response = self.execute_checked(
            "validate active",
            VectorProjectionHelperRequest::ValidateActiveContents(
                VectorProjectionValidateActiveRequest {
                    request_id: request_id.clone(),
                    projection_store: self.store_descriptor.store_name.clone(),
                    active: active.clone(),
                },
            ),
        )?;
        let VectorProjectionHelperResponse::ValidateActiveContents(response) = response else {
            return wrong_operation("validate active");
        };
        require_validation(
            "active contents",
            &request_id,
            &self.store_descriptor.store_name,
            response,
        )
    }

    pub(crate) fn repair_wire_publication(
        &self,
        expected: &WireProjectionArtifactEvidence,
    ) -> Result<()> {
        self.require_evidence_binding("repair publication", expected)?;
        let context = mutation_context(
            &expected.manifest.store_name,
            &expected.manifest.generation,
            &expected.manifest.delivery_digest,
        )?;
        let response = self.execute_checked(
            "repair publication",
            VectorProjectionHelperRequest::RepairPublication(
                VectorProjectionRepairPublicationRequest {
                    context: context.clone(),
                    expected: expected.clone(),
                },
            ),
        )?;
        let VectorProjectionHelperResponse::RepairPublication(ack) = response else {
            return wrong_operation("repair publication");
        };
        require_ack("repair publication", &context, &ack)
    }

    pub(crate) fn quarantine_wire_generation(
        &self,
        generation_id: &str,
        delivery_digest: &str,
    ) -> Result<()> {
        self.mutate_wire_generation(
            "quarantine generation",
            generation_id,
            delivery_digest,
            VectorProjectionHelperRequest::Quarantine,
            |response| match response {
                VectorProjectionHelperResponse::Quarantine(ack) => Some(ack),
                _ => None,
            },
        )
    }

    pub(crate) fn abort_wire_generation(
        &self,
        generation_id: &str,
        delivery_digest: &str,
    ) -> Result<()> {
        self.mutate_wire_generation(
            "abort generation",
            generation_id,
            delivery_digest,
            VectorProjectionHelperRequest::Abort,
            |response| match response {
                VectorProjectionHelperResponse::Abort(ack) => Some(ack),
                _ => None,
            },
        )
    }

    #[allow(dead_code)]
    pub(crate) fn inventory_wire_generations(&self) -> Result<VectorProjectionInventoryResponse> {
        let request_id = next_request_id();
        let response = self.execute_checked(
            "inventory generations",
            VectorProjectionHelperRequest::Inventory(VectorProjectionInventoryRequest {
                request_id: request_id.clone(),
                projection_store: self.store_descriptor.store_name.clone(),
            }),
        )?;
        let VectorProjectionHelperResponse::Inventory(response) = response else {
            return wrong_operation("inventory generations");
        };
        if response.request_id != request_id
            || response.projection_store != self.store_descriptor.store_name
        {
            return Err(KanbanError::Storage(
                "LanceDB projection inventory correlation mismatch".to_owned(),
            ));
        }
        let mut generations = BTreeSet::new();
        for generation in &response.generations {
            if generation.generation_id.trim().is_empty()
                || !generations.insert(generation.generation_id.as_str())
            {
                return Err(KanbanError::Storage(
                    "LanceDB projection inventory contains an empty or duplicate generation"
                        .to_owned(),
                ));
            }
            if let Some(evidence) = &generation.evidence {
                self.remember_generation_digest(
                    &evidence.manifest.generation,
                    &evidence.manifest.delivery_digest,
                );
                if evidence.manifest.generation != generation.generation_id {
                    return Err(KanbanError::Storage(
                        "LanceDB projection inventory evidence generation mismatch".to_owned(),
                    ));
                }
                self.require_evidence_binding("inventory generations", evidence)?;
            }
        }
        Ok(response)
    }

    #[allow(dead_code)]
    pub(crate) fn cleanup_wire_generations(
        &self,
        context_generation: &str,
        delivery_digest: &str,
        dry_run: bool,
        protection: VectorProjectionCleanupProtection,
    ) -> Result<VectorProjectionCleanupResponse> {
        let context = mutation_context(
            &self.store_descriptor.store_name,
            context_generation,
            delivery_digest,
        )?;
        let response = self.execute_checked(
            "cleanup generations",
            VectorProjectionHelperRequest::Cleanup(VectorProjectionCleanupRequest {
                context: context.clone(),
                dry_run,
                protection,
            }),
        )?;
        let VectorProjectionHelperResponse::Cleanup(response) = response else {
            return wrong_operation("cleanup generations");
        };
        require_ack("cleanup generations", &context, &response.ack)?;
        if response.dry_run != dry_run || (dry_run && !response.removed_generations.is_empty()) {
            return Err(KanbanError::Storage(
                "LanceDB projection cleanup receipt does not match dry-run semantics".to_owned(),
            ));
        }
        Ok(response)
    }

    fn mutate_wire_generation(
        &self,
        action: &str,
        generation_id: &str,
        delivery_digest: &str,
        request: impl FnOnce(VectorProjectionGenerationMutationRequest) -> VectorProjectionHelperRequest,
        response_ack: impl FnOnce(VectorProjectionHelperResponse) -> Option<VectorProjectionMutationAck>,
    ) -> Result<()> {
        let context = mutation_context(
            &self.store_descriptor.store_name,
            generation_id,
            delivery_digest,
        )?;
        let response = self.execute_checked(
            action,
            request(VectorProjectionGenerationMutationRequest {
                context: context.clone(),
            }),
        )?;
        let Some(ack) = response_ack(response) else {
            return wrong_operation(action);
        };
        require_ack(action, &context, &ack)
    }

    fn require_evidence_binding(
        &self,
        action: &str,
        evidence: &WireProjectionArtifactEvidence,
    ) -> Result<()> {
        let manifest = &evidence.manifest;
        if evidence.fingerprint.trim().is_empty()
            || manifest
                .fingerprint
                .as_ref()
                .is_some_and(|fingerprint| fingerprint != &evidence.fingerprint)
            || manifest.store_name != self.store_descriptor.store_name
            || !manifest.database_instance_id.starts_with("db_")
            || manifest.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || manifest.schema_version != self.store_descriptor.schema_version
            || manifest.generation.trim().is_empty()
            || manifest.fence_epoch < 0
            || manifest.snapshot_cursor < 0
            || manifest.provider != self.store_descriptor.provider
            || manifest.provider_fingerprint != self.store_descriptor.provider_fingerprint
            || manifest.corpus != self.store_descriptor.corpus
            || manifest.canonical_item_count < 0
            || manifest.canonical_digest.trim().is_empty()
            || manifest.delivery_item_count < 0
            || manifest.delivery_digest.trim().is_empty()
        {
            return Err(KanbanError::Conflict(format!(
                "LanceDB projection {action} evidence does not match the pinned helper descriptor"
            )));
        }
        Ok(())
    }

    fn remember_generation_digest(&self, generation_id: &str, delivery_digest: &str) {
        if generation_id.trim().is_empty() || delivery_digest.trim().is_empty() {
            return;
        }
        if let Ok(mut digests) = self.generation_digests.lock() {
            digests.insert(generation_id.to_owned(), delivery_digest.to_owned());
        }
    }

    fn generation_delivery_digest(&self, generation_id: &str) -> Result<String> {
        if let Some(digest) = self
            .generation_digests
            .lock()
            .map_err(|_| {
                KanbanError::Storage(
                    "LanceDB projection generation digest mutex is poisoned".to_owned(),
                )
            })?
            .get(generation_id)
            .cloned()
        {
            return Ok(digest);
        }
        let conn = crate::db::connect_file(&self.db_path)?;
        let digest = conn
            .query_row(
                "SELECT CASE
                   WHEN building_generation=?2 THEN building_delivery_digest
                   WHEN active_generation=?2 THEN active_delivery_digest
                   WHEN previous_generation=?2 THEN previous_delivery_digest
                   ELSE NULL
                 END
                 FROM projection_store_state
                 WHERE store_name=?1",
                rusqlite::params![self.store_descriptor.store_name, generation_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(super::storage)?
            .filter(|digest| !digest.trim().is_empty())
            .ok_or_else(|| {
                KanbanError::Conflict(format!(
                    "SQLite has no delivery digest for LanceDB projection generation {generation_id}"
                ))
            })?;
        self.remember_generation_digest(generation_id, &digest);
        Ok(digest)
    }

    fn wire_snapshot(&self, snapshot: &ProjectionSnapshot) -> Result<WireProjectionSnapshot> {
        if snapshot.manifest.fingerprint.is_some() {
            return Err(KanbanError::Conflict(
                "LanceDB projection snapshot manifest already has a physical fingerprint"
                    .to_owned(),
            ));
        }
        let records = snapshot
            .records
            .iter()
            .map(|record| {
                if record.board_id.trim().is_empty()
                    || !record.identity.starts_with("kb://")
                    || record.content_hash.trim().is_empty()
                    || serde_json::from_str::<serde_json::Value>(&record.payload_json).is_err()
                {
                    return Err(KanbanError::Conflict(
                        "LanceDB projection snapshot contains an invalid canonical record"
                            .to_owned(),
                    ));
                }
                Ok(WireProjectionSnapshotRecord {
                    board_id: record.board_id.clone(),
                    identity: record.identity.clone(),
                    payload_json: record.payload_json.clone(),
                    content_hash: record.content_hash.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(WireProjectionSnapshot {
            manifest: self.wire_manifest("snapshot", &snapshot.manifest)?,
            records,
        })
    }

    fn wire_evidence(
        &self,
        action: &str,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<WireProjectionArtifactEvidence> {
        if evidence.fingerprint.trim().is_empty()
            || evidence.manifest.fingerprint.as_deref() != Some(evidence.fingerprint.as_str())
        {
            return Err(KanbanError::Conflict(format!(
                "LanceDB projection {action} local evidence fingerprint is incomplete"
            )));
        }
        Ok(WireProjectionArtifactEvidence {
            manifest: self.wire_manifest(action, &evidence.manifest)?,
            fingerprint: evidence.fingerprint.clone(),
        })
    }

    fn local_evidence(
        &self,
        action: &str,
        evidence: WireProjectionArtifactEvidence,
    ) -> Result<ProjectionArtifactEvidence> {
        self.require_evidence_binding(action, &evidence)?;
        let fingerprint = evidence.fingerprint;
        let manifest = evidence.manifest;
        self.remember_generation_digest(&manifest.generation, &manifest.delivery_digest);
        Ok(ProjectionArtifactEvidence {
            manifest: ProjectionArtifactManifest {
                store_name: manifest.store_name,
                database_instance_id: manifest.database_instance_id,
                protocol_version: manifest.protocol_version,
                schema_version: manifest.schema_version,
                generation: manifest.generation,
                fence_epoch: manifest.fence_epoch,
                snapshot_cursor: manifest.snapshot_cursor,
                provider: manifest.provider,
                provider_fingerprint: manifest.provider_fingerprint,
                canonical_item_count: manifest.canonical_item_count,
                canonical_digest: manifest.canonical_digest,
                delivery_item_count: manifest.delivery_item_count,
                delivery_digest: manifest.delivery_digest,
                fingerprint: Some(fingerprint.clone()),
            },
            fingerprint,
        })
    }

    fn wire_manifest(
        &self,
        action: &str,
        manifest: &ProjectionArtifactManifest,
    ) -> Result<WireProjectionArtifactManifest> {
        if manifest.store_name != self.store_descriptor.store_name
            || !manifest.database_instance_id.starts_with("db_")
            || manifest.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || manifest.schema_version != self.store_descriptor.schema_version
            || manifest.generation.trim().is_empty()
            || manifest.fence_epoch < 0
            || manifest.snapshot_cursor < 0
            || manifest.provider != self.store_descriptor.provider
            || manifest.provider_fingerprint != self.store_descriptor.provider_fingerprint
            || manifest.canonical_item_count < 0
            || manifest.canonical_digest.trim().is_empty()
            || manifest.delivery_item_count < 0
            || manifest.delivery_digest.trim().is_empty()
        {
            return Err(KanbanError::Conflict(format!(
                "LanceDB projection {action} local manifest does not match the pinned helper descriptor"
            )));
        }
        Ok(WireProjectionArtifactManifest {
            store_name: manifest.store_name.clone(),
            database_instance_id: manifest.database_instance_id.clone(),
            protocol_version: manifest.protocol_version,
            schema_version: manifest.schema_version,
            generation: manifest.generation.clone(),
            fence_epoch: manifest.fence_epoch,
            snapshot_cursor: manifest.snapshot_cursor,
            provider: manifest.provider.clone(),
            provider_fingerprint: manifest.provider_fingerprint.clone(),
            corpus: self.store_descriptor.corpus.clone(),
            canonical_item_count: manifest.canonical_item_count,
            canonical_digest: manifest.canonical_digest.clone(),
            delivery_item_count: manifest.delivery_item_count,
            delivery_digest: manifest.delivery_digest.clone(),
            // The helper persists the immutable pre-publication manifest and
            // carries the physical fingerprint in the enclosing evidence.
            fingerprint: None,
        })
    }

    fn wire_batch(&self, batch: &ProjectionBatch) -> Result<WireProjectionBatch> {
        if batch.store_name != self.store_descriptor.store_name
            || !batch.database_instance_id.starts_with("db_")
            || batch.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || batch.schema_version != self.store_descriptor.schema_version
            || batch.provider != self.store_descriptor.provider
            || batch.provider_fingerprint != self.store_descriptor.provider_fingerprint
            || batch.owner.trim().is_empty()
            || batch.lease_token.trim().is_empty()
            || batch.fence_epoch < 0
            || batch.target_generation.trim().is_empty()
            || batch.claim_token.trim().is_empty()
            || batch.claim_expires_at <= 0
        {
            return Err(KanbanError::Conflict(
                "LanceDB projection local batch binding is invalid".to_owned(),
            ));
        }
        let items = batch
            .items
            .iter()
            .map(|item| {
                let action = match item.action.as_str() {
                    "upsert" => WireProjectionDeliveryAction::Upsert,
                    "delete" => WireProjectionDeliveryAction::Delete,
                    "rebuild" => WireProjectionDeliveryAction::Rebuild,
                    _ => {
                        return Err(KanbanError::Conflict(format!(
                            "LanceDB projection delivery {} has an unsupported action",
                            item.id
                        )));
                    }
                };
                if item.id <= 0
                    || item.outbox_id <= 0
                    || item.store_name != batch.store_name
                    || item.board_id.trim().is_empty()
                    || item.cursor <= 0
                    || !item.entity_uri.starts_with("kb://")
                    || item.attempts < 0
                    || serde_json::from_str::<serde_json::Value>(&item.payload_json).is_err()
                {
                    return Err(KanbanError::Conflict(format!(
                        "LanceDB projection delivery {} is invalid",
                        item.id
                    )));
                }
                Ok(WireProjectionDelivery {
                    id: item.id,
                    outbox_id: item.outbox_id,
                    store_name: item.store_name.clone(),
                    generation_id: batch.target_generation.clone(),
                    board_id: item.board_id.clone(),
                    source_event_id: item.source_event_id,
                    cursor: item.cursor,
                    action,
                    entity_uri: item.entity_uri.clone(),
                    payload_json: item.payload_json.clone(),
                    attempts: item.attempts,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(WireProjectionBatch {
            store_name: batch.store_name.clone(),
            database_instance_id: batch.database_instance_id.clone(),
            protocol_version: batch.protocol_version,
            schema_version: batch.schema_version,
            provider: batch.provider.clone(),
            provider_fingerprint: batch.provider_fingerprint.clone(),
            owner: batch.owner.clone(),
            lease_token: batch.lease_token.clone(),
            fence_epoch: batch.fence_epoch,
            target_generation: batch.target_generation.clone(),
            claim_token: batch.claim_token.clone(),
            claim_expires_at: batch.claim_expires_at,
            items,
        })
    }
}

impl ProjectionStoreBackend for LanceDbProjectionStore {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: self.store_descriptor.store_name.clone(),
            provider: self.store_descriptor.provider.clone(),
            provider_fingerprint: self.store_descriptor.provider_fingerprint.clone(),
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        let snapshot = self.wire_snapshot(snapshot)?;
        let evidence = self.prepare_wire_snapshot(&snapshot)?;
        self.local_evidence("prepare snapshot", evidence)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        let delivery_digest = self.generation_delivery_digest(&batch.target_generation)?;
        let wire_batch = self.wire_batch(batch)?;
        let receipt = self.apply_wire_batch(&wire_batch, &delivery_digest)?;
        Ok(ProjectionBatchReceipt {
            store_name: receipt.store_name,
            database_instance_id: receipt.database_instance_id,
            protocol_version: receipt.protocol_version,
            schema_version: receipt.schema_version,
            provider: receipt.provider,
            provider_fingerprint: receipt.provider_fingerprint,
            target_generation: receipt.target_generation,
            lease_token: receipt.lease_token,
            fence_epoch: receipt.fence_epoch,
            claim_token: receipt.claim_token,
            applied_item_count: receipt.applied_item_count,
        })
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt> {
        let expected_active = expected_active
            .map(|evidence| self.wire_evidence("publish previous", evidence))
            .transpose()?;
        let prepared = self.wire_evidence("publish prepared", prepared)?;
        let receipt = self.publish_wire_generation(expected_active.as_ref(), &prepared)?;
        Ok(ProjectionPublishReceipt {
            active: self.local_evidence("publish active", receipt.active)?,
            retained_previous: receipt
                .retained_previous
                .map(|evidence| self.local_evidence("publish retained previous", evidence))
                .transpose()?,
        })
    }

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
        self.inspect_wire_active()?
            .map(|evidence| self.local_evidence("inspect active", evidence))
            .transpose()
    }

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>> {
        self.inspect_wire_generation(generation)?
            .map(|evidence| self.local_evidence("inspect generation", evidence))
            .transpose()
    }

    fn validate_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let expected = self.wire_evidence("validate generation", expected)?;
        self.validate_wire_generation_publication(&expected)
    }

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let expected = self.wire_evidence("repair publication", expected)?;
        self.repair_wire_publication(&expected)
    }

    fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
        let active = self.wire_evidence("validate active", active)?;
        self.validate_wire_active_contents(&active)
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        let delivery_digest = self.generation_delivery_digest(generation)?;
        self.quarantine_wire_generation(generation, &delivery_digest)
    }

    fn abort_generation(&self, generation: &str) -> Result<()> {
        let delivery_digest = self.generation_delivery_digest(generation)?;
        self.abort_wire_generation(generation, &delivery_digest)
    }
}

fn mutation_context(
    projection_store: &str,
    generation_id: &str,
    delivery_digest: &str,
) -> Result<VectorProjectionMutationContext> {
    if projection_store.trim().is_empty()
        || generation_id.trim().is_empty()
        || delivery_digest.trim().is_empty()
    {
        return Err(KanbanError::InvalidInput(
            "LanceDB projection mutation correlation fields cannot be empty".to_owned(),
        ));
    }
    Ok(VectorProjectionMutationContext {
        request_id: next_request_id(),
        projection_store: projection_store.to_owned(),
        generation_id: generation_id.to_owned(),
        delivery_digest: delivery_digest.to_owned(),
    })
}

fn require_ack(
    action: &str,
    expected: &VectorProjectionMutationContext,
    actual: &VectorProjectionMutationAck,
) -> Result<()> {
    if actual.request_id != expected.request_id
        || actual.projection_store != expected.projection_store
        || actual.generation_id != expected.generation_id
        || actual.delivery_digest != expected.delivery_digest
    {
        return Err(KanbanError::Storage(format!(
            "LanceDB projection {action} acknowledgement correlation mismatch"
        )));
    }
    Ok(())
}

fn require_validation(
    action: &str,
    request_id: &str,
    projection_store: &str,
    response: VectorProjectionValidationResponse,
) -> Result<()> {
    if response.request_id != request_id || response.projection_store != projection_store {
        return Err(KanbanError::Storage(format!(
            "LanceDB projection {action} validation correlation mismatch"
        )));
    }
    if !response.valid {
        return Err(KanbanError::Storage(format!(
            "LanceDB projection {action} validation failed"
        )));
    }
    Ok(())
}

fn wrong_operation<T>(action: &str) -> Result<T> {
    Err(KanbanError::Storage(format!(
        "LanceDB projection helper returned the wrong {action} response operation"
    )))
}

pub(crate) fn resolve_vector_projection_helper() -> PathBuf {
    if let Ok(value) = env::var(VECTOR_PROJECTION_HELPER_ENV) {
        let value = value.trim();
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }

    let installed = PathBuf::from("/usr/lib/kanban").join(VECTOR_PROJECTION_HELPER);
    if installed.exists() {
        return installed;
    }

    if let Ok(current_exe) = env::current_exe()
        && let Some(directory) = current_exe.parent()
    {
        let sibling = directory.join(VECTOR_PROJECTION_HELPER);
        if sibling.exists() {
            return sibling;
        }
    }

    ["KANBAN_CARGO_TARGET_ROOT", "CARGO_TARGET_DIR"]
        .into_iter()
        .filter_map(|key| {
            let value = env::var(key).ok()?;
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            let candidate = PathBuf::from(value)
                .join("release")
                .join(VECTOR_PROJECTION_HELPER);
            candidate.is_file().then_some(candidate)
        })
        .next()
        .unwrap_or_else(|| PathBuf::from(VECTOR_PROJECTION_HELPER))
}

pub(crate) fn lancedb_failure_class(error: &KanbanError) -> LanceDbProjectionFailureClass {
    match error {
        KanbanError::InvalidInput(_) => LanceDbProjectionFailureClass::Provider,
        KanbanError::Conflict(_) => LanceDbProjectionFailureClass::Delivery,
        KanbanError::Storage(_) => LanceDbProjectionFailureClass::Backend,
        _ => LanceDbProjectionFailureClass::Backend,
    }
}

fn validate_helper_descriptor<'descriptor>(
    store_name: &str,
    descriptor: &'descriptor VectorProjectionHelperDescriptor,
) -> Result<&'descriptor WireProjectionStoreDescriptor> {
    if descriptor.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
        || descriptor.build_identity.trim().is_empty()
        || REQUIRED_OPERATIONS
            .iter()
            .any(|operation| !descriptor.supported_operations.contains(operation))
    {
        return Err(KanbanError::Storage(
            "LanceDB projection helper does not advertise the complete Projection v2 protocol"
                .to_owned(),
        ));
    }
    let store = descriptor
        .supported_stores
        .iter()
        .find(|candidate| candidate.store_name == store_name)
        .ok_or_else(|| {
            KanbanError::Storage(format!(
                "LanceDB projection helper does not advertise store {store_name}"
            ))
        })?;
    let corpus_schema = match store_name {
        LANCEDB_CHUNKS_STORE => TASK_CHUNKS_CORPUS_SCHEMA,
        LANCEDB_LABEL_ATOMS_STORE => LABEL_ATOMS_CORPUS_SCHEMA,
        _ => {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported LanceDB projection store: {store_name}"
            )));
        }
    };
    let corpus = store.corpus.as_ref().ok_or_else(|| {
        KanbanError::Storage(format!(
            "LanceDB projection helper store {store_name} has no corpus binding"
        ))
    })?;
    let provider_fingerprint = embedding_provider_fingerprint(
        &store.provider,
        &corpus.embedding_model,
        corpus.embedding_dimensions,
    );
    let corpus_fingerprint =
        corpus_provider_fingerprint(corpus_schema, &store.provider_fingerprint);
    if store.schema_version != DERIVED_STORE_SCHEMA_VERSION
        || store.provider.trim().is_empty()
        || store.provider_fingerprint != provider_fingerprint
        || corpus.corpus_schema != corpus_schema
        || corpus.corpus_fingerprint != corpus_fingerprint
        || corpus.embedding_model.trim().is_empty()
        || corpus.embedding_dimensions == 0
    {
        return Err(KanbanError::Storage(format!(
            "LanceDB projection helper store {store_name} has an incompatible schema/provider/corpus binding"
        )));
    }
    Ok(store)
}

fn next_request_id() -> String {
    new_typed_id("vpreq")
}

fn projection_transport_error(action: &str, error: VectorError) -> KanbanError {
    let message = format!("LanceDB projection {action} failed: {error}");
    match &error {
        VectorError::MissingEmbeddingProvider
        | VectorError::DimensionMismatch { .. }
        | VectorError::EmbeddingModelMismatch { .. }
        | VectorError::Provider { .. } => KanbanError::InvalidInput(message),
        VectorError::ProjectionHelper(error)
            if error.kind == VectorProjectionHelperErrorKind::Provider =>
        {
            KanbanError::InvalidInput(message)
        }
        VectorError::ProjectionHelper(error)
            if error.kind == VectorProjectionHelperErrorKind::Delivery =>
        {
            KanbanError::Conflict(message)
        }
        VectorError::Chunk(_) => KanbanError::Conflict(message),
        VectorError::ProjectionHelper(_) | VectorError::Disabled | VectorError::Store(_) => {
            KanbanError::Storage(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use kanban_contract::{
        ProjectionBatch, ProjectionCorpusMetadata, ProjectionDelivery, ProjectionDeliveryAction,
        ProjectionStoreDescriptor, VectorProjectionApplyBatchResponse,
        VectorProjectionBatchApplicationReceipt, VectorProjectionHelperDescriptor,
        VectorProjectionHelperRequest, VectorProjectionHelperResponse,
        VectorProjectionInspectActiveResponse, VectorProjectionMutationAck,
        VectorProjectionPrepareSnapshotResponse,
    };
    use kanban_vector::{corpus_provider_fingerprint, embedding_provider_fingerprint};

    use super::*;

    struct ScriptedTransport {
        response: Mutex<Option<VectorProjectionHelperResponse>>,
        requests: Mutex<Vec<VectorProjectionHelperRequest>>,
    }

    impl ScriptedTransport {
        fn new(response: VectorProjectionHelperResponse) -> Self {
            Self {
                response: Mutex::new(Some(response)),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl VectorProjectionTransport for ScriptedTransport {
        fn execute(
            &self,
            request: &VectorProjectionHelperRequest,
        ) -> std::result::Result<VectorProjectionHelperResponse, VectorError> {
            self.requests.lock().unwrap().push(request.clone());
            let mut response = self
                .response
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| VectorError::Store("unexpected request".to_owned()))?;
            if let (
                VectorProjectionHelperRequest::Descriptor(request),
                VectorProjectionHelperResponse::Descriptor(response),
            ) = (request, &mut response)
                && response.request_id.is_empty()
            {
                response.request_id.clone_from(&request.request_id);
            }
            Ok(response)
        }
    }

    type TransportHandler = dyn FnMut(
            &VectorProjectionHelperRequest,
        ) -> std::result::Result<VectorProjectionHelperResponse, VectorError>
        + Send;

    struct FunctionTransport {
        handler: Mutex<Box<TransportHandler>>,
    }

    impl FunctionTransport {
        fn new(
            handler: impl FnMut(
                &VectorProjectionHelperRequest,
            )
                -> std::result::Result<VectorProjectionHelperResponse, VectorError>
            + Send
            + 'static,
        ) -> Self {
            Self {
                handler: Mutex::new(Box::new(handler)),
            }
        }
    }

    impl VectorProjectionTransport for FunctionTransport {
        fn execute(
            &self,
            request: &VectorProjectionHelperRequest,
        ) -> std::result::Result<VectorProjectionHelperResponse, VectorError> {
            (self.handler.lock().unwrap())(request)
        }
    }

    #[test]
    fn descriptor_pins_independent_corpus_fingerprints() {
        for (store_name, corpus_schema) in [
            (LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA),
            (LANCEDB_LABEL_ATOMS_STORE, LABEL_ATOMS_CORPUS_SCHEMA),
        ] {
            let response = descriptor_response(store_name, corpus_schema);
            let transport = Arc::new(ScriptedTransport::new(response));
            let backend = LanceDbProjectionStore::connect_transport(
                transport.clone(),
                PathBuf::from("descriptor-fixture.db"),
                store_name,
            )
            .unwrap();

            assert_eq!(backend.wire_descriptor().store_name, store_name);
            assert_eq!(
                backend
                    .wire_descriptor()
                    .corpus
                    .as_ref()
                    .unwrap()
                    .corpus_schema,
                corpus_schema
            );
            assert_eq!(transport.requests.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn descriptor_rejects_model_dimension_fingerprint_mismatch() {
        let VectorProjectionHelperResponse::Descriptor(mut descriptor) =
            descriptor_response(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA)
        else {
            unreachable!()
        };
        descriptor.supported_stores[0]
            .corpus
            .as_mut()
            .unwrap()
            .embedding_dimensions += 1;
        let error = LanceDbProjectionStore::connect_transport(
            Arc::new(ScriptedTransport::new(
                VectorProjectionHelperResponse::Descriptor(descriptor),
            )),
            PathBuf::from("descriptor-fixture.db"),
            LANCEDB_CHUNKS_STORE,
        )
        .unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)));
        assert!(error.to_string().contains("incompatible"));
    }

    #[test]
    fn descriptor_rejects_wrong_request_correlation() {
        let VectorProjectionHelperResponse::Descriptor(mut descriptor) =
            descriptor_response(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA)
        else {
            unreachable!()
        };
        descriptor.request_id = "vpreq_wrong".to_owned();
        let error = LanceDbProjectionStore::connect_transport(
            Arc::new(ScriptedTransport::new(
                VectorProjectionHelperResponse::Descriptor(descriptor),
            )),
            PathBuf::from("descriptor-fixture.db"),
            LANCEDB_CHUNKS_STORE,
        )
        .unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)));
        assert!(error.to_string().contains("correlation"));
    }

    #[test]
    fn chunk_and_label_atom_corpora_have_distinct_fingerprints() {
        let chunks = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let labels = helper_descriptor(LANCEDB_LABEL_ATOMS_STORE, LABEL_ATOMS_CORPUS_SCHEMA);

        assert_ne!(
            chunks.supported_stores[0]
                .corpus
                .as_ref()
                .unwrap()
                .corpus_fingerprint,
            labels.supported_stores[0]
                .corpus
                .as_ref()
                .unwrap()
                .corpus_fingerprint
        );
    }

    #[test]
    fn descriptor_requires_the_full_runtime_operation_set() {
        let VectorProjectionHelperResponse::Descriptor(mut descriptor) =
            descriptor_response(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA)
        else {
            unreachable!()
        };
        descriptor
            .supported_operations
            .retain(|operation| *operation != VectorProjectionHelperOperation::Abort);
        let error = LanceDbProjectionStore::connect_transport(
            Arc::new(ScriptedTransport::new(
                VectorProjectionHelperResponse::Descriptor(descriptor),
            )),
            PathBuf::from("descriptor-fixture.db"),
            LANCEDB_CHUNKS_STORE,
        )
        .unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)));
    }

    #[test]
    fn incompatible_inspection_caches_delivery_digest_for_lossless_quarantine() {
        let descriptor = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let store_descriptor = descriptor.supported_stores[0].clone();
        let mut corpus = store_descriptor.corpus.clone().unwrap();
        corpus.corpus_schema = "task-chunks-v1".to_owned();
        let evidence = WireProjectionArtifactEvidence {
            manifest: WireProjectionArtifactManifest {
                store_name: LANCEDB_CHUNKS_STORE.to_owned(),
                database_instance_id: "db_fixture".to_owned(),
                protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
                schema_version: DERIVED_STORE_SCHEMA_VERSION,
                generation: "gen_incompatible".to_owned(),
                fence_epoch: 7,
                snapshot_cursor: 11,
                provider: store_descriptor.provider.clone(),
                provider_fingerprint: store_descriptor.provider_fingerprint.clone(),
                corpus: Some(corpus),
                canonical_item_count: 3,
                canonical_digest: "canonical:fixture".to_owned(),
                delivery_item_count: 4,
                delivery_digest: "delivery:incompatible".to_owned(),
                fingerprint: Some("physical:incompatible".to_owned()),
            },
            fingerprint: "physical:incompatible".to_owned(),
        };
        let transport = Arc::new(FunctionTransport::new(move |request| {
            let VectorProjectionHelperRequest::InspectActive(request) = request else {
                return Err(VectorError::Store("unexpected request".to_owned()));
            };
            Ok(VectorProjectionHelperResponse::InspectActive(
                VectorProjectionInspectActiveResponse {
                    request_id: request.request_id.clone(),
                    projection_store: request.projection_store.clone(),
                    active: Some(evidence.clone()),
                },
            ))
        }));
        let backend = LanceDbProjectionStore {
            db_path: PathBuf::from("missing-fixture.db"),
            transport,
            helper_descriptor: descriptor,
            store_descriptor,
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        };

        let error = backend.inspect_wire_active().unwrap_err();

        assert!(matches!(error, KanbanError::Conflict(_)));
        assert_eq!(
            backend
                .generation_delivery_digest("gen_incompatible")
                .unwrap(),
            "delivery:incompatible"
        );
    }

    #[test]
    fn apply_receipt_reconstructs_capabilities_from_stdin_request_only() {
        let descriptor = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let store_descriptor = descriptor.supported_stores[0].clone();
        let transport = Arc::new(FunctionTransport::new(|request| {
            let VectorProjectionHelperRequest::ApplyBatch(request) = request else {
                return Err(VectorError::Store("unexpected request".to_owned()));
            };
            assert_eq!(request.batch.lease_token, "lease-secret");
            assert_eq!(request.batch.claim_token, "claim-secret");
            Ok(VectorProjectionHelperResponse::ApplyBatch(
                VectorProjectionApplyBatchResponse {
                    ack: VectorProjectionMutationAck {
                        request_id: request.context.request_id.clone(),
                        projection_store: request.context.projection_store.clone(),
                        generation_id: request.context.generation_id.clone(),
                        delivery_digest: request.context.delivery_digest.clone(),
                    },
                    receipt: VectorProjectionBatchApplicationReceipt {
                        store_name: request.batch.store_name.clone(),
                        database_instance_id: request.batch.database_instance_id.clone(),
                        protocol_version: request.batch.protocol_version,
                        schema_version: request.batch.schema_version,
                        provider: request.batch.provider.clone(),
                        provider_fingerprint: request.batch.provider_fingerprint.clone(),
                        target_generation: request.batch.target_generation.clone(),
                        fence_epoch: request.batch.fence_epoch,
                        applied_item_count: request.batch.items.len(),
                    },
                },
            ))
        }));
        let backend = LanceDbProjectionStore {
            db_path: PathBuf::from("apply-fixture.db"),
            transport,
            helper_descriptor: descriptor,
            store_descriptor: store_descriptor.clone(),
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let batch = ProjectionBatch {
            store_name: LANCEDB_CHUNKS_STORE.to_owned(),
            database_instance_id: "db_fixture".to_owned(),
            protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
            schema_version: DERIVED_STORE_SCHEMA_VERSION,
            provider: store_descriptor.provider.clone(),
            provider_fingerprint: store_descriptor.provider_fingerprint.clone(),
            owner: "owner".to_owned(),
            lease_token: "lease-secret".to_owned(),
            fence_epoch: 7,
            target_generation: "gen_fixture".to_owned(),
            claim_token: "claim-secret".to_owned(),
            claim_expires_at: 10_000,
            items: vec![ProjectionDelivery {
                id: 1,
                outbox_id: 2,
                store_name: LANCEDB_CHUNKS_STORE.to_owned(),
                generation_id: "gen_fixture".to_owned(),
                board_id: "board_fixture".to_owned(),
                source_event_id: Some(3),
                cursor: 2,
                action: ProjectionDeliveryAction::Upsert,
                entity_uri: "kb://task/task_fixture".to_owned(),
                payload_json: "{}".to_owned(),
                attempts: 1,
            }],
        };

        let receipt = backend
            .apply_wire_batch(&batch, "delivery:fixture")
            .unwrap();

        assert_eq!(receipt.lease_token, "lease-secret");
        assert_eq!(receipt.claim_token, "claim-secret");
        let debug = format!("{receipt:?}");
        assert!(!debug.contains("lease-secret"));
        assert!(!debug.contains("claim-secret"));
    }

    #[test]
    fn prepare_accepts_physical_fingerprint_added_to_evidence_manifest() {
        let descriptor = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let store_descriptor = descriptor.supported_stores[0].clone();
        let transport = Arc::new(FunctionTransport::new(|request| {
            let VectorProjectionHelperRequest::PrepareSnapshot(request) = request else {
                return Err(VectorError::Store("unexpected request".to_owned()));
            };
            let mut manifest = request.snapshot.manifest.clone();
            manifest.fingerprint = Some("physical:fixture".to_owned());
            Ok(VectorProjectionHelperResponse::PrepareSnapshot(
                VectorProjectionPrepareSnapshotResponse {
                    ack: VectorProjectionMutationAck {
                        request_id: request.context.request_id.clone(),
                        projection_store: request.context.projection_store.clone(),
                        generation_id: request.context.generation_id.clone(),
                        delivery_digest: request.context.delivery_digest.clone(),
                    },
                    evidence: WireProjectionArtifactEvidence {
                        manifest,
                        fingerprint: "physical:fixture".to_owned(),
                    },
                },
            ))
        }));
        let backend = LanceDbProjectionStore {
            db_path: PathBuf::from("prepare-fixture.db"),
            transport,
            helper_descriptor: descriptor,
            store_descriptor: store_descriptor.clone(),
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let snapshot = WireProjectionSnapshot {
            manifest: WireProjectionArtifactManifest {
                store_name: LANCEDB_CHUNKS_STORE.to_owned(),
                database_instance_id: "db_fixture".to_owned(),
                protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
                schema_version: DERIVED_STORE_SCHEMA_VERSION,
                generation: "gen_fixture".to_owned(),
                fence_epoch: 2,
                snapshot_cursor: 3,
                provider: store_descriptor.provider,
                provider_fingerprint: store_descriptor.provider_fingerprint,
                corpus: store_descriptor.corpus,
                canonical_item_count: 0,
                canonical_digest: "canonical:fixture".to_owned(),
                delivery_item_count: 0,
                delivery_digest: "delivery:fixture".to_owned(),
                fingerprint: None,
            },
            records: Vec::new(),
        };

        let evidence = backend.prepare_wire_snapshot(&snapshot).unwrap();

        assert_eq!(evidence.fingerprint, "physical:fixture");
        assert_eq!(
            evidence.manifest.fingerprint.as_deref(),
            Some("physical:fixture")
        );
    }

    #[test]
    fn local_evidence_round_trip_injects_corpus_without_persisting_wire_details() {
        let descriptor = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let store_descriptor = descriptor.supported_stores[0].clone();
        let backend = LanceDbProjectionStore {
            db_path: PathBuf::from("evidence-fixture.db"),
            transport: Arc::new(FunctionTransport::new(|_| {
                Err(VectorError::Store("transport is not used".to_owned()))
            })),
            helper_descriptor: descriptor,
            store_descriptor,
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let evidence = ProjectionArtifactEvidence {
            manifest: ProjectionArtifactManifest {
                store_name: LANCEDB_CHUNKS_STORE.to_owned(),
                database_instance_id: "db_fixture".to_owned(),
                protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
                schema_version: DERIVED_STORE_SCHEMA_VERSION,
                generation: "gen_fixture".to_owned(),
                fence_epoch: 1,
                snapshot_cursor: 2,
                provider: backend.store_descriptor.provider.clone(),
                provider_fingerprint: backend.store_descriptor.provider_fingerprint.clone(),
                canonical_item_count: 3,
                canonical_digest: "canonical:fixture".to_owned(),
                delivery_item_count: 4,
                delivery_digest: "delivery:fixture".to_owned(),
                fingerprint: Some("physical:fixture".to_owned()),
            },
            fingerprint: "physical:fixture".to_owned(),
        };

        let wire = backend.wire_evidence("fixture", &evidence).unwrap();
        assert_eq!(
            wire.manifest.corpus.as_ref().unwrap().corpus_schema,
            TASK_CHUNKS_CORPUS_SCHEMA
        );
        assert_eq!(wire.manifest.fingerprint, None);

        let round_trip = backend.local_evidence("fixture", wire).unwrap();
        assert_eq!(round_trip, evidence);
    }

    #[test]
    fn local_batch_conversion_binds_each_delivery_to_the_target_generation() {
        let descriptor = helper_descriptor(LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA);
        let store_descriptor = descriptor.supported_stores[0].clone();
        let backend = LanceDbProjectionStore {
            db_path: PathBuf::from("batch-conversion-fixture.db"),
            transport: Arc::new(FunctionTransport::new(|_| {
                Err(VectorError::Store("transport is not used".to_owned()))
            })),
            helper_descriptor: descriptor,
            store_descriptor: store_descriptor.clone(),
            generation_digests: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let mut batch = super::ProjectionBatch {
            store_name: LANCEDB_CHUNKS_STORE.to_owned(),
            database_instance_id: "db_fixture".to_owned(),
            protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
            schema_version: DERIVED_STORE_SCHEMA_VERSION,
            provider: store_descriptor.provider,
            provider_fingerprint: store_descriptor.provider_fingerprint,
            owner: "owner".to_owned(),
            lease_token: "lease-secret".to_owned(),
            fence_epoch: 4,
            target_generation: "gen_fixture".to_owned(),
            claim_token: "claim-secret".to_owned(),
            claim_expires_at: 100,
            items: vec![super::super::ProjectionDelivery {
                id: 1,
                outbox_id: 2,
                store_name: LANCEDB_CHUNKS_STORE.to_owned(),
                board_id: "board_fixture".to_owned(),
                source_event_id: None,
                cursor: 2,
                action: "delete".to_owned(),
                entity_uri: "kb://task/task_fixture".to_owned(),
                payload_json: "{}".to_owned(),
                attempts: 1,
            }],
        };

        let wire = backend.wire_batch(&batch).unwrap();
        assert_eq!(wire.items[0].generation_id, "gen_fixture");
        assert_eq!(wire.items[0].action, ProjectionDeliveryAction::Delete);
        let batch_debug = format!("{batch:?}");
        assert!(!batch_debug.contains("lease-secret"));
        assert!(!batch_debug.contains("claim-secret"));

        let receipt = super::ProjectionBatchReceipt {
            store_name: batch.store_name.clone(),
            database_instance_id: batch.database_instance_id.clone(),
            protocol_version: batch.protocol_version,
            schema_version: batch.schema_version,
            provider: batch.provider.clone(),
            provider_fingerprint: batch.provider_fingerprint.clone(),
            target_generation: batch.target_generation.clone(),
            lease_token: batch.lease_token.clone(),
            fence_epoch: batch.fence_epoch,
            claim_token: batch.claim_token.clone(),
            applied_item_count: 1,
        };
        let receipt_debug = format!("{receipt:?}");
        assert!(!receipt_debug.contains("lease-secret"));
        assert!(!receipt_debug.contains("claim-secret"));

        batch.items[0].action = "unknown".to_owned();
        assert!(matches!(
            backend.wire_batch(&batch),
            Err(KanbanError::Conflict(_))
        ));
    }

    fn descriptor_response(
        store_name: &str,
        corpus_schema: &str,
    ) -> VectorProjectionHelperResponse {
        VectorProjectionHelperResponse::Descriptor(helper_descriptor(store_name, corpus_schema))
    }

    fn helper_descriptor(
        store_name: &str,
        corpus_schema: &str,
    ) -> VectorProjectionHelperDescriptor {
        let provider = "ollama";
        let model = "embedding-fixture";
        let dimensions = 8;
        let provider_fingerprint = embedding_provider_fingerprint(provider, model, dimensions);
        let corpus_fingerprint = corpus_provider_fingerprint(corpus_schema, &provider_fingerprint);
        VectorProjectionHelperDescriptor {
            request_id: String::new(),
            protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
            build_identity: "fixture@1".to_owned(),
            supported_stores: vec![ProjectionStoreDescriptor {
                store_name: store_name.to_owned(),
                schema_version: DERIVED_STORE_SCHEMA_VERSION,
                provider: provider.to_owned(),
                provider_fingerprint,
                corpus: Some(ProjectionCorpusMetadata {
                    corpus_schema: corpus_schema.to_owned(),
                    corpus_fingerprint,
                    embedding_model: model.to_owned(),
                    embedding_dimensions: dimensions,
                }),
            }],
            supported_operations: REQUIRED_OPERATIONS.to_vec(),
        }
    }
}
