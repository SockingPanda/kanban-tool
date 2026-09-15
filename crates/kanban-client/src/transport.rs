use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use kanban_protocol::rpc::{MAX_MESSAGE_BYTES, v1::kanban_service_client::KanbanServiceClient};
use tonic::{Request, metadata::MetadataValue, transport::Channel};

use crate::{ClientError, KanbanClient};

pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const UNARY_TIMEOUT: Duration = Duration::from_secs(30);

// 每个调用点固定 request 类型与生成 client 方法；没有动态方法名或 JSON dispatcher。
macro_rules! rpc {
    ($client:expr, $method:ident, $request:ident, $path:expr, $query:expr, $input:expr) => {{
        let message = kanban_protocol::rpc::v1::$request::from_parts($path, $query, $input)
            .map_err($crate::ClientError::request_codec)?;
        let client = $client;
        tokio::time::timeout($crate::transport::UNARY_TIMEOUT, async {
            let mut rpc_client = client.rpc_client().await?;
            let response = rpc_client
                .$method(client.unary_request(message))
                .await
                .map_err($crate::ClientError::status)?
                .into_inner();
            response
                .try_into()
                .map_err($crate::ClientError::response_codec)
        })
        .await
        .map_err(|_| $crate::ClientError::unary_timeout())?
    }};
}

pub(crate) use rpc;

impl KanbanClient {
    pub(crate) async fn channel(&self) -> Result<Channel, ClientError> {
        self.channel
            .get_or_try_init(|| async {
                self.endpoint.connect().await.map_err(|error| {
                    ClientError::ServerUnavailable(format!("{}: {error}", self.base_url))
                })
            })
            .await
            .cloned()
    }

    pub(crate) async fn rpc_client(&self) -> Result<KanbanServiceClient<Channel>, ClientError> {
        Ok(KanbanServiceClient::new(self.channel().await?)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_MESSAGE_BYTES))
    }

    pub(crate) fn request<T>(&self, message: T) -> Request<T> {
        let mut request = Request::new(message);
        request.metadata_mut().insert_bin(
            "x-kb-actor-bin",
            MetadataValue::from_bytes(self.actor.as_bytes()),
        );
        request
    }

    pub(crate) fn unary_request<T>(&self, message: T) -> Request<T> {
        let mut request = self.request(message);
        request.set_timeout(UNARY_TIMEOUT);
        request
    }
}

pub(crate) fn normalize_localhost_url(value: String) -> Result<String, ClientError> {
    let value = value.trim().trim_end_matches('/').to_owned();
    let Some(authority) = value.strip_prefix("http://") else {
        return Err(ClientError::InvalidServerUrl(value));
    };
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@'])
        || !is_loopback_authority(authority)
    {
        return Err(ClientError::InvalidServerUrl(value));
    }
    Ok(value)
}

fn is_loopback_authority(authority: &str) -> bool {
    if matches!(authority, "localhost" | "[::1]") {
        return true;
    }
    if let Some(port) = authority.strip_prefix("localhost:") {
        return port.parse::<u16>().is_ok();
    }
    authority.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || authority
            .parse::<SocketAddr>()
            .is_ok_and(|addr| addr.ip().is_loopback())
}
