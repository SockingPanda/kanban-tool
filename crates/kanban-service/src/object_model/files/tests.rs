use std::{io::Read, sync::Arc};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::{DeleteAttachmentCommand, object_model::ObjectErrorCode, test_support::create_input};

async fn fixture() -> (tempfile::TempDir, KanbanService) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("attachments");
    std::fs::create_dir(&root).unwrap();
    let service = KanbanService::open_with_roots(dir.path().join("state.db"), None, Arc::new(root))
        .await
        .unwrap();
    for id in ["t_one", "t_two"] {
        service
            .store
            .create_task("default", create_input(id, None, id))
            .await
            .unwrap();
    }
    (dir, service)
}

fn spec(id: &str, bytes: &[u8]) -> FileUploadSpec {
    FileUploadSpec {
        board_id: "b_default".into(),
        owner_id: "t_one".into(),
        file_id: id.into(),
        filename: "证据.bin".into(),
        content_type: Some("application/octet-stream".into()),
        size_bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        actor: "file-test".into(),
    }
}

async fn upload(service: &KanbanService, spec: &FileUploadSpec, bytes: &[u8]) -> FileInfo {
    let staging = service.file_begin_upload(spec).await.unwrap();
    let bytes = bytes.to_vec();
    let staging =
        tokio::task::spawn_blocking(move || staging.write_chunk(&bytes).unwrap().seal().unwrap())
            .await
            .unwrap();
    service.file_commit_upload(spec, staging).await.unwrap()
}

#[tokio::test]
async fn shared_blob_and_two_owners_survive_task_unlink_and_replay() {
    let (_dir, service) = fixture().await;
    let bytes = b"same immutable content";
    let original = spec("a_one", bytes);
    upload(&service, &original, bytes).await;
    upload(&service, &spec("a_two", bytes), bytes).await;
    assert!(
        service
            .file_upload_replay(&original)
            .await
            .unwrap()
            .is_some()
    );
    let mut changed = original.clone();
    changed.actor = "other-agent".into();
    assert_eq!(
        service.file_upload_replay(&changed).await.unwrap_err().code,
        ObjectErrorCode::Conflict
    );
    let file = service.object_get("b_default", "a_one").await.unwrap();
    let owner = service.object_get("b_default", "t_two").await.unwrap();
    let catalog = service.object_catalog().await.unwrap();
    service.object_execute(serde_json::from_value(json!({
        "board_id":"b_default","actor":"file-test","request_id":"second-owner",
        "mutation":{"operation":"change_relations","change":{
            "expected_catalog_version":catalog.version,"expected":[{"id":file.id,"expected":file.version},{"id":owner.id,"expected":owner.version}],
            "remove":[],"add":[{"relation_key":"file.attachment","source_id":"a_one","target_id":"t_two"}]
        }}
    })).unwrap()).await.unwrap();
    assert!(
        service
            .delete_attachment(DeleteAttachmentCommand {
                task_id: "t_one".into(),
                attachment_id: "a_one".into(),
                actor: "file-test".into()
            })
            .await
            .unwrap()
    );
    assert_eq!(
        service.file_list("b_default", "t_two").await.unwrap().len(),
        1
    );
    assert!(service.file_upload_replay(&original).await.is_err());
    let mut downloaded = service
        .file_download("b_default", "t_two", "a_one")
        .await
        .unwrap();
    let mut observed = Vec::new();
    downloaded.file.read_to_end(&mut observed).unwrap();
    assert_eq!(observed, bytes);
    let c = service.store.connection().await.unwrap();
    assert_eq!(
        super::super::store::count(&c, "SELECT COUNT(*) FROM file_blobs", vec![])
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        super::super::store::count(&c, "SELECT COUNT(*) FROM task_attachments", vec![])
            .await
            .unwrap(),
        0
    );
    assert!(
        service
            .file_download("other-board", "t_two", "a_one")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn empty_file_limits_and_cancelled_staging_preserve_capacity() {
    let (dir, service) = fixture().await;
    let empty = spec("a_empty", b"");
    upload(&service, &empty, b"").await;
    assert_eq!(
        service
            .file_download("b_default", "t_one", "a_empty")
            .await
            .unwrap()
            .info
            .size_bytes,
        0
    );
    let mut oversized = spec("a_large", b"");
    oversized.size_bytes = crate::MAX_ATTACHMENT_BYTES + 1;
    assert!(service.file_begin_upload(&oversized).await.is_err());
    let mut staging = Vec::new();
    for _ in 0..4 {
        staging.push(
            service
                .file_begin_upload(&spec("a_pending", b""))
                .await
                .unwrap(),
        );
    }
    assert!(
        service
            .file_begin_upload(&spec("a_busy", b""))
            .await
            .is_err()
    );
    drop(staging.pop());
    staging.push(
        service
            .file_begin_upload(&spec("a_recovered", b""))
            .await
            .unwrap(),
    );
    drop(staging);
    assert_eq!(
        std::fs::read_dir(dir.path().join("attachments/.incoming"))
            .unwrap()
            .count(),
        0
    );
}
