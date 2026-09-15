use std::sync::Arc;

use tokio::sync::OnceCell;
use tonic::transport::Endpoint;

use crate::{
    KanbanClient,
    error::ClientError,
    transport::{CONNECT_TIMEOUT, normalize_localhost_url},
};

impl std::fmt::Debug for KanbanClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KanbanClient")
            .field("base_url", &self.base_url)
            .field("actor", &self.actor)
            .finish_non_exhaustive()
    }
}

impl KanbanClient {
    pub fn new(base_url: impl Into<String>, actor: impl Into<String>) -> Result<Self, ClientError> {
        let base_url = normalize_localhost_url(base_url.into())?;
        let actor = actor.into();
        if actor.trim().is_empty() || actor.len() > 8192 {
            return Err(ClientError::InvalidInput(
                "actor 必须为非空文本且不超过 8192 字节".to_owned(),
            ));
        }
        let endpoint = Endpoint::from_shared(base_url.clone())
            .map_err(|_| ClientError::InvalidServerUrl(base_url.clone()))?
            .connect_timeout(CONNECT_TIMEOUT);
        Ok(Self {
            base_url,
            actor,
            endpoint,
            channel: Arc::new(OnceCell::new()),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_loopback_base_urls() {
        for value in [
            "http://127.0.0.1:8721",
            "http://localhost:8721/",
            "http://[::1]:8721",
        ] {
            assert!(KanbanClient::new(value, "test").is_ok(), "{value}");
        }
        for value in [
            "",
            "http://example.com",
            "http://192.168.1.10:8721",
            "https://127.0.0.1:8721",
            "http://127.0.0.1:8721@evil.example",
            "http://localhost:8721/api",
        ] {
            assert!(KanbanClient::new(value, "test").is_err(), "{value}");
        }
    }

    #[test]
    fn constructor_is_runtime_independent_and_clones_share_lazy_channel() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "异步作者").unwrap();
        assert!(client.channel.get().is_none());
        assert!(Arc::ptr_eq(&client.channel, &client.clone().channel));
        assert!(KanbanClient::new(crate::DEFAULT_SERVER_URL, " ").is_err());
        assert!(KanbanClient::new(crate::DEFAULT_SERVER_URL, "x".repeat(8193)).is_err());
    }
}
