use std::time::Duration;

use crate::{KanbanClient, error::ClientError, transport::normalize_localhost_url};

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
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout_read(Duration::from_secs(30))
            .timeout_write(Duration::from_secs(30))
            .build();
        Ok(Self {
            base_url,
            actor: actor.into(),
            agent,
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
}
