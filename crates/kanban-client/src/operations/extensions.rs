//! 对象和文件调用复用共享 channel、actor metadata 与整次 unary 预算。
use crate::{ClientError, KanbanClient, transport::UNARY_TIMEOUT};
use kanban_protocol::rpc::{extensions as w, v1};

macro_rules! calls {
    ($client:ident; $( $method:ident($input:ty) -> $output:ty; )*) => {
        impl KanbanClient {
            $(pub async fn $method(&self, input: $input) -> Result<$output, ClientError> {
                tokio::time::timeout(UNARY_TIMEOUT, async {
                    let mut client = $client::new(self.channel().await?)
                        .max_decoding_message_size(8 * 1024 * 1024)
                        .max_encoding_message_size(2 * 1024 * 1024);
                    client.$method(self.unary_request(input)).await
                        .map(tonic::Response::into_inner).map_err(ClientError::status)
                }).await.map_err(|_| ClientError::unary_timeout())?
            })*
        }
    };
}
use w::file_service_client::FileServiceClient;
use w::object_service_client::ObjectServiceClient;
// 生成 client 的类型在模块内引用；宏不创建独立连接池。
calls! {
    ObjectServiceClient;
    execute_object(w::ObjectCommandInput) -> w::ObjectDocument;
    get_object(w::ObjectIdentityInput) -> w::ObjectDocument;
    list_objects(w::ObjectListInput) -> w::ObjectDocument;
    get_object_catalog(v1::Empty) -> w::ObjectDocument;
    get_object_overview(w::ObjectIdentityInput) -> w::ObjectDocument;
    get_workflow_closure(w::ObjectIdentityInput) -> w::ObjectDocument;
    get_object_references(w::ObjectReferencesInput) -> w::ObjectDocument;
    get_object_history(w::ObjectHistoryInput) -> w::ObjectDocument;
    get_object_snapshots(w::ObjectSnapshotsInput) -> w::ObjectDocument;
    diagnose_objects(w::ObjectBoardInput) -> w::ObjectDocument;
}
calls! {
    FileServiceClient;
    begin_file_upload(w::BeginFileUploadInput) -> w::BeginFileUploadOutput;
    write_file_chunk(w::WriteFileChunkInput) -> w::FileOffsetOutput;
    finish_file_upload(w::FileUploadIdentity) -> w::FileInfo;
    cancel_file_upload(w::FileUploadIdentity) -> w::CancelFileUploadOutput;
    list_object_files(w::FileOwnerInput) -> w::FileListOutput;
    download_file(w::FileIdentityInput) -> tonic::Streaming<w::FileDownloadFrame>;
}
