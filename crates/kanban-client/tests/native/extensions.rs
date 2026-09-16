use crate::common::{Host, task_request};
use kanban_protocol::rpc::{self, extensions as w};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn document(value: w::ObjectDocument) -> Value {
    rpc::decode_json(value.data.unwrap()).unwrap()
}

#[tokio::test]
async fn native_objects_and_chunked_files_preserve_actor_cas_replay_and_limits() {
    let host = Host::start().await;
    let client = &host.client;
    let task = client
        .create_task("default", task_request("t_extension"))
        .await
        .unwrap();
    let command = w::ObjectCommandInput {
        board_id: task.board_id.clone(), actor: String::new(), request_id: "object-native".into(),
        mutation: Some(rpc::encode_json(json!({"operation":"create","object":{"type_key":"module","title":"原生模块","body":null,"properties":{}}})).unwrap()),
    };
    let first = document(client.execute_object(command.clone()).await.unwrap());
    assert_eq!(
        document(client.execute_object(command).await.unwrap())["replayed"],
        true
    );
    let id = first["ids"][0].as_str().unwrap().to_owned();
    let identity = w::ObjectIdentityInput {
        board_id: task.board_id.clone(),
        object_id: id.clone(),
    };
    let original = document(client.get_object(identity.clone()).await.unwrap());
    let patch = w::ObjectCommandInput {
        board_id:task.board_id.clone(),actor:String::new(),request_id:"object-patch".into(),
        mutation:Some(rpc::encode_json(json!({"operation":"patch","patch":{"target":{"id":id,"expected":original["version"]},"title":"更新模块","edits":[]}})).unwrap()),
    };
    client.execute_object(patch.clone()).await.unwrap();
    let mut stale = patch;
    stale.request_id = "stale-patch".into();
    assert_eq!(
        client.execute_object(stale).await.unwrap_err().code(),
        "conflict"
    );
    assert_eq!(
        document(client.get_object(identity).await.unwrap())["title"],
        "更新模块"
    );
    let bytes = b"native chunk receipt";
    let begin = w::BeginFileUploadInput {
        board_id: task.board_id.clone(),
        owner_id: task.id.clone(),
        file_id: "a_native_chunk".into(),
        filename: "原始.bin".into(),
        content_type: "application/octet-stream".into(),
        size_bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        actor: String::new(),
    };
    let session = client.begin_file_upload(begin.clone()).await.unwrap();
    let chunk = w::WriteFileChunkInput {
        upload_id: session.upload_id.clone(),
        offset: 0,
        data: bytes.to_vec(),
    };
    assert_eq!(
        client.write_file_chunk(chunk.clone()).await.unwrap().offset,
        bytes.len() as u64
    );
    assert_eq!(
        client.write_file_chunk(chunk).await.unwrap().offset,
        bytes.len() as u64
    );
    let wrong = w::WriteFileChunkInput {
        upload_id: session.upload_id.clone(),
        offset: 0,
        data: b"different bytes".to_vec(),
    };
    assert_eq!(
        client.write_file_chunk(wrong).await.unwrap_err().code(),
        "conflict"
    );
    let info = client
        .finish_file_upload(w::FileUploadIdentity {
            upload_id: session.upload_id,
        })
        .await
        .unwrap();
    assert_eq!(info.created_by, "原生作者");
    assert_eq!(
        client
            .begin_file_upload(begin.clone())
            .await
            .unwrap()
            .completed,
        Some(info.clone())
    );
    let mut oversized = begin.clone();
    oversized.file_id = "a_oversized".into();
    oversized.size_bytes = 256 * 1024 * 1024 + 1;
    assert_eq!(
        client
            .begin_file_upload(oversized)
            .await
            .unwrap_err()
            .code(),
        "invalid_input"
    );
    let mut download = client
        .download_file(w::FileIdentityInput {
            board_id: task.board_id.clone(),
            owner_id: task.id.clone(),
            file_id: info.id.clone(),
        })
        .await
        .unwrap();
    let mut observed = Vec::new();
    let mut completed = false;
    while let Some(frame) = download.message().await.unwrap() {
        match frame.frame.unwrap() {
            w::file_download_frame::Frame::Header(header) => assert_eq!(header, info),
            w::file_download_frame::Frame::Chunk(chunk) => {
                assert_eq!(chunk.offset, observed.len() as u64);
                observed.extend(chunk.data);
            }
            w::file_download_frame::Frame::Complete(complete) => {
                assert_eq!(complete.sha256, begin.sha256);
                completed = true;
            }
        }
    }
    assert!(completed);
    assert_eq!(observed, bytes);
    let mut pending = begin;
    pending.file_id = "a_cancelled".into();
    let pending = client.begin_file_upload(pending).await.unwrap();
    assert!(
        client
            .cancel_file_upload(w::FileUploadIdentity {
                upload_id: pending.upload_id
            })
            .await
            .unwrap()
            .removed
    );
    let files = client
        .list_object_files(w::FileOwnerInput {
            board_id: task.board_id,
            owner_id: task.id,
            limit: 100,
            after_id: String::new(),
        })
        .await
        .unwrap();
    assert_eq!(files.items.len(), 1);
    host.finish().await;
}
