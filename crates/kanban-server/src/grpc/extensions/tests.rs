use super::*;
use kanban_protocol::rpc::extensions::{self as w, file_service_server::FileService};
use kanban_service::object_model::files::FileUploadSpec;
use sha2::{Digest, Sha256};
use std::time::Duration;

#[tokio::test]
async fn stalled_file_download_releases_permits_on_deadline_and_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("files.db"), "file-proof")
        .await
        .unwrap();
    let service = state.application();
    let board = service.get_board("default").await.unwrap().id;
    let receipt = service.object_execute(serde_json::from_value(serde_json::json!({
        "board_id":board,"actor":"file-proof","request_id":"owner",
        "mutation":{"operation":"create","object":{"type_key":"note","title":"慢读取","body":null,"properties":{}}}
    })).unwrap()).await.unwrap();
    let owner = receipt.ids[0].clone();
    let bytes = vec![37; 3 * 1024 * 1024];
    let spec = FileUploadSpec {
        board_id: board.clone(),
        owner_id: owner.clone(),
        file_id: "a_stalled".into(),
        filename: "stalled.bin".into(),
        content_type: None,
        size_bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        actor: "file-proof".into(),
    };
    let mut upload = service.file_begin_upload(&spec).await.unwrap();
    for chunk in bytes.chunks(64 * 1024) {
        upload = upload.write_chunk(chunk).unwrap();
    }
    let upload = upload.seal().unwrap();
    service.file_commit_upload(&spec, upload).await.unwrap();
    let (_, runtime) = mount(&state);
    let downloads = Arc::new(Semaphore::new(4));
    let files = files::Files {
        state,
        runtime: runtime.clone(),
        downloads: downloads.clone(),
        chunk_workers: Arc::new(Semaphore::new(8)),
    };
    let input = w::FileIdentityInput {
        board_id: board,
        owner_id: owner,
        file_id: spec.file_id,
    };
    let mut request = tonic::Request::new(input.clone());
    request.set_timeout(Duration::from_millis(150));
    let stalled = files.download_file(request).await.unwrap();
    // 保持 response 存活但从不读取；两个有界块填满后 producer 会阻塞。
    tokio::time::timeout(Duration::from_secs(3), async {
        while downloads.available_permits() != 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(runtime.streams.available_permits(), 32);
    drop(stalled);
    let stalled = files
        .download_file(tonic::Request::new(input))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), runtime.stop())
        .await
        .unwrap();
    assert_eq!(downloads.available_permits(), 4);
    assert!(runtime.check().is_err());
    drop(stalled);
}
