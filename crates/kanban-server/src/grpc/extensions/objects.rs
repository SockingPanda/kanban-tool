//! 对象 RPC 与 QueryService 读取共享同一转换和 application query。
use super::{ExtensionRuntime, status};
use crate::AppState;
use kanban_protocol::rpc::{
    self, extensions as w,
    v1::{query_definition::Query, query_result::Result as Projection},
};
use kanban_service::object_model as model;
use serde::Serialize;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub(super) struct Objects {
    pub state: AppState,
    pub runtime: ExtensionRuntime,
}
fn document<T: Serialize>(value: T) -> Result<w::ObjectDocument, Status> {
    let json = serde_json::to_value(value).map_err(status::io)?;
    if serde_json::to_vec(&json).map_err(status::io)?.len() > 4 * 1024 * 1024 {
        return Err(status::exhausted("object.response_too_large"));
    }
    Ok(w::ObjectDocument {
        data: Some(rpc::encode_json(json).map_err(status::io)?),
    })
}
fn required_json<T: serde::de::DeserializeOwned>(
    value: Option<rpc::v1::JsonValue>,
) -> Result<T, Status> {
    let value = value.ok_or_else(|| status::invalid("object.missing_value"))?;
    serde_json::from_value(rpc::decode_json(value).map_err(super::super::context::invalid_request)?)
        .map_err(super::super::context::invalid_request)
}
fn optional_json<T: serde::de::DeserializeOwned>(
    value: Option<rpc::v1::JsonValue>,
) -> Result<Option<T>, Status> {
    value.map(|v| required_json(Some(v))).transpose()
}
fn some(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
fn limit(value: u32, default: u32) -> u32 {
    if value == 0 { default } else { value }
}

macro_rules! reads {
    ($( $method:ident: $variant:ident($input:ty) ($state:ident, $i:pat) => $call:expr; )*) => {
        pub(crate) async fn read_query(state: &AppState, query: Query) -> Result<Projection, Status> {
            match query {
                $(Query::$variant(input) => {
                    let $state = state; let $i = input;
                    Ok(Projection::$variant(document($call.await.map_err(status::object)?)?))
                })*
                Query::ListObjectFiles(input) => Ok(Projection::ListObjectFiles(super::files::list(state, input).await?)),
                _ => Err(status::invalid("object.query_unsupported")),
            }
        }
        #[tonic::async_trait]
        impl w::object_service_server::ObjectService for Objects {
            async fn execute_object(&self, request: Request<w::ObjectCommandInput>) -> Result<Response<w::ObjectDocument>, Status> {
                self.runtime.check()?;
                let actor = status::actor(&self.state, request.metadata(), &request.get_ref().actor)?;
                let input = request.into_inner();
                document(self.state.application().object_execute(model::ObjectCommand {
                    board_id: input.board_id, actor, request_id: input.request_id,
                    mutation: required_json(input.mutation)?,
                }).await.map_err(status::object)?).map(Response::new)
            }
            $(async fn $method(&self, request: Request<$input>) -> Result<Response<w::ObjectDocument>, Status> {
                self.runtime.check()?;
                let $state = &self.state; let $i = request.into_inner();
                document($call.await.map_err(status::object)?).map(Response::new)
            })*
        }
    };
}
reads! {
    get_object: GetObject(w::ObjectIdentityInput)(state, i) => state.application().object_get(&i.board_id, &i.object_id);
    list_objects: ListObjects(w::ObjectListInput)(state, i) => state.application().object_list(model::ObjectQuery {
        board_id: i.board_id, type_key: some(i.type_key), text: some(i.text), filters: optional_json(i.filters)?.unwrap_or_default(),
        include_archived: i.include_archived, limit: limit(i.limit, 100), after: optional_json(i.after)?,
    });
    get_object_catalog: GetObjectCatalog(rpc::v1::Empty)(state, _) => state.application().object_catalog();
    get_object_overview: GetObjectOverview(w::ObjectIdentityInput)(state, i) => state.application().object_overview(&i.board_id, &i.object_id);
    get_workflow_closure: GetWorkflowClosure(w::ObjectIdentityInput)(state, i) => state.application().object_workflow_closure(&i.board_id, &i.object_id);
    get_object_references: GetObjectReferences(w::ObjectReferencesInput)(state, i) => state.application().object_references(required_json(i.query)?);
    get_object_history: GetObjectHistory(w::ObjectHistoryInput)(state, i) => state.application().object_history(&i.board_id, &i.object_id, i.after, limit(i.limit, 100));
    get_object_snapshots: GetObjectSnapshots(w::ObjectSnapshotsInput)(state, i) => state.application().object_snapshots(&i.board_id, &i.object_id, some(i.before), limit(i.limit, 50));
    diagnose_objects: DiagnoseObjects(w::ObjectBoardInput)(state, i) => state.application().object_diagnostics(&i.board_id);
}
