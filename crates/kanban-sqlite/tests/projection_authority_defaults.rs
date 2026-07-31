use std::sync::atomic::{AtomicUsize, Ordering};

use kanban_core::Result;
use kanban_sqlite::api::provider::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionDestructiveAuthority, ProjectionGenerationBinding,
    ProjectionGenerationRole, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor,
};

#[derive(Default)]
struct LegacyOnlyMutatingBackend {
    prepare_calls: AtomicUsize,
    apply_calls: AtomicUsize,
    publish_calls: AtomicUsize,
    repair_calls: AtomicUsize,
}

impl ProjectionStoreBackend for LegacyOnlyMutatingBackend {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: "tantivy_tasks".to_owned(),
            provider: "legacy-only".to_owned(),
            provider_fingerprint: "legacy-only-v1".to_owned(),
            corpus: None,
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        self.prepare_calls.fetch_add(1, Ordering::SeqCst);
        let mut manifest = snapshot.manifest.clone();
        manifest.fingerprint = Some("legacy-fingerprint".to_owned());
        Ok(ProjectionArtifactEvidence {
            manifest,
            fingerprint: "legacy-fingerprint".to_owned(),
        })
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        self.apply_calls.fetch_add(1, Ordering::SeqCst);
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
            claim_token: batch.claim_token.clone(),
            applied_item_count: batch.items.len(),
        })
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt> {
        self.publish_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ProjectionPublishReceipt {
            active: prepared.clone(),
            retained_previous: expected_active.cloned(),
        })
    }

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
        Ok(None)
    }

    fn inspect_generation(&self, _generation: &str) -> Result<Option<ProjectionArtifactEvidence>> {
        Ok(None)
    }

    fn repair_generation_publication(&self, _expected: &ProjectionArtifactEvidence) -> Result<()> {
        self.repair_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn authority_mutating_defaults_never_delegate_to_legacy_mutators() {
    let backend = LegacyOnlyMutatingBackend::default();
    let manifest = ProjectionArtifactManifest {
        store_name: "tantivy_tasks".to_owned(),
        database_instance_id: "db_test".to_owned(),
        protocol_version: 2,
        schema_version: 1,
        generation: "gen_authority".to_owned(),
        fence_epoch: 7,
        snapshot_cursor: 11,
        provider: "legacy-only".to_owned(),
        provider_fingerprint: "legacy-only-v1".to_owned(),
        corpus: None,
        canonical_item_count: 0,
        canonical_digest: "fnv64:canonical".to_owned(),
        delivery_item_count: 0,
        delivery_digest: "fnv64:delivery".to_owned(),
        fingerprint: Some("legacy-fingerprint".to_owned()),
    };
    let evidence = ProjectionArtifactEvidence {
        manifest: manifest.clone(),
        fingerprint: "legacy-fingerprint".to_owned(),
    };
    let snapshot = ProjectionSnapshot {
        manifest: manifest.clone(),
        records: Vec::new(),
    };
    let batch = ProjectionBatch {
        store_name: manifest.store_name.clone(),
        database_instance_id: manifest.database_instance_id.clone(),
        protocol_version: manifest.protocol_version,
        schema_version: manifest.schema_version,
        provider: manifest.provider.clone(),
        provider_fingerprint: manifest.provider_fingerprint.clone(),
        corpus: None,
        owner: "owner".to_owned(),
        lease_token: "lease-token".to_owned(),
        fence_epoch: 9,
        target_generation: manifest.generation.clone(),
        claim_token: "claim-token".to_owned(),
        claim_expires_at: i64::MAX,
        items: Vec::new(),
    };
    let authority = ProjectionDestructiveAuthority {
        owner: "owner".to_owned(),
        lease_token: "lease-token".to_owned(),
        fence_epoch: 9,
        lease_expires_at: i64::MAX,
        role: ProjectionGenerationRole::Building,
        generation: manifest.generation.clone(),
        expected_manifest: Some(manifest.clone()),
        expected_binding: ProjectionGenerationBinding {
            generation: manifest.generation.clone(),
            fingerprint: manifest.fingerprint.clone(),
            fence_epoch: manifest.fence_epoch,
            snapshot_cursor: Some(manifest.snapshot_cursor),
            provider: manifest.provider.clone(),
            provider_fingerprint: manifest.provider_fingerprint.clone(),
            canonical_count: manifest.canonical_item_count,
            canonical_digest: manifest.canonical_digest.clone(),
            delivery_count: manifest.delivery_item_count,
            delivery_digest: manifest.delivery_digest.clone(),
            corpus: None,
        },
        building_phase: Some("prepared".to_owned()),
    };

    let prepare_error = backend
        .prepare_snapshot_with_authority(&snapshot, &authority)
        .expect_err("legacy prepare must not be reachable through the authority seam");
    let apply_error = backend
        .apply_batch_with_authority(&batch, &authority)
        .expect_err("legacy batch apply must not be reachable through the authority seam");
    let publish_error = backend
        .publish_generation_with_authority(None, &evidence, &authority)
        .expect_err("legacy publish must not be reachable through the authority seam");
    let repair_error = backend
        .repair_generation_publication_with_authority(&evidence, &authority)
        .expect_err("legacy repair must not be reachable through the authority seam");

    for error in [prepare_error, apply_error, publish_error, repair_error] {
        assert!(
            error.to_string().contains("authority-bearing"),
            "unexpected fail-closed error: {error}"
        );
    }
    assert_eq!(backend.prepare_calls.load(Ordering::SeqCst), 0);
    assert_eq!(backend.apply_calls.load(Ordering::SeqCst), 0);
    assert_eq!(backend.publish_calls.load(Ordering::SeqCst), 0);
    assert_eq!(backend.repair_calls.load(Ordering::SeqCst), 0);
}
