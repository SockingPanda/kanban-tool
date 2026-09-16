//! 测试通过实际正式 RPC 路由发送 Protobuf，不另建业务 adapter。
use std::{collections::BTreeMap, fmt::Debug};

use axum::{
    body::Body,
    http::{Request, Response, StatusCode, Version},
};
use http_body_util::BodyExt;
use prost::Message;
use tonic::{Code, Status};

pub(crate) fn rpc_request(
    method: &str,
    message: impl Message,
    metadata: &BTreeMap<String, String>,
) -> Request<Body> {
    let bytes = message.encode_to_vec();
    let mut framed = vec![0];
    framed.extend(u32::try_from(bytes.len()).unwrap().to_be_bytes());
    framed.extend(bytes);
    // 内存 Request builder 默认 HTTP/1.1；这里模拟与真实 tonic channel 一致的原生 H2 请求。
    let mut request = Request::builder()
        .version(Version::HTTP_2)
        .method("POST")
        .uri(format!("/kanban.v1.KanbanService/{method}"))
        .header("content-type", "application/grpc");
    for (name, value) in metadata {
        // Content-Type 是 transport framing，既有 fixture 的 locale/actor metadata 保留原值。
        if !name.eq_ignore_ascii_case("content-type") {
            request = request.header(name, value);
        }
    }
    request.body(Body::from(framed)).unwrap()
}

pub(crate) async fn wire_response<M: Message + Default>(
    response: Response<Body>,
) -> Result<M, Status> {
    assert_eq!(response.status(), StatusCode::OK);
    if let Some(status) = Status::from_header_map(response.headers())
        && status.code() != Code::Ok
    {
        return Err(status);
    }
    let collected = response.into_body().collect().await.unwrap();
    let trailers = collected
        .trailers()
        .expect("正式 unary 必须以 grpc-status 结束");
    assert!(trailers.contains_key("grpc-status"));
    if let Some(status) = Status::from_header_map(trailers)
        && status.code() != Code::Ok
    {
        return Err(status);
    }
    let bytes = collected.to_bytes();
    assert!(bytes.len() >= 5);
    assert_eq!(bytes[0], 0);
    let length = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    assert_eq!(
        length + 5,
        bytes.len(),
        "unary 必须恰好交付一个完整 Protobuf 消息"
    );
    Ok(M::decode(&bytes[5..]).unwrap())
}

pub(crate) async fn decode_response<M, T>(response: Response<Body>) -> T
where
    M: Message + Default + TryInto<T>,
    M::Error: Debug,
{
    wire_response::<M>(response)
        .await
        .unwrap()
        .try_into()
        .unwrap()
}

pub(crate) async fn decode_error(response: Response<Body>) -> kanban_protocol::ErrorEnvelope {
    let status = wire_response::<kanban_protocol::rpc::v1::GetHealthResponse>(response)
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::InvalidArgument);
    kanban_protocol::ErrorEnvelope {
        error: kanban_protocol::rpc::decode_status(&status).unwrap(),
    }
}

pub(crate) fn parts<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}
