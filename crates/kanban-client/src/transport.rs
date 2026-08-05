use std::io::Read;
use std::net::{IpAddr, SocketAddr};

use kanban_protocol::ErrorEnvelope;
use serde::{Serialize, de::DeserializeOwned};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub(crate) fn get<T>(&self, path: &str) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let request = self
            .agent
            .get(&format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.call())
    }

    pub(crate) fn post<B, T>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let body = serde_json::to_value(body)
            .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
        let request = self
            .agent
            .post(&format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.send_json(body))
    }

    pub(crate) fn put<B, T>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let body = serde_json::to_value(body)
            .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
        let request = self
            .agent
            .request("PUT", &format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.send_json(body))
    }

    pub(crate) fn delete<T>(&self, path: &str) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let request = self
            .agent
            .request("DELETE", &format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.call())
    }

    pub(crate) fn patch<B, T>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        let body = serde_json::to_value(body)
            .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
        let request = self
            .agent
            .request("PATCH", &format!("{}{path}", self.base_url))
            .set("Accept", "application/json")
            .set("X-KB-Actor", &self.actor);
        decode_response(request.send_json(body))
    }

    pub(crate) fn get_text(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<(Option<String>, String), ClientError> {
        let request = self
            .agent
            .get(&format!("{}{path}", self.base_url))
            .set("Accept", accept)
            .set("X-KB-Actor", &self.actor);
        decode_text_response(request.call())
    }

    pub(crate) fn get_bytes(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<(Option<String>, Option<String>, Option<String>, Vec<u8>), ClientError> {
        let request = self
            .agent
            .get(&format!("{}{path}", self.base_url))
            .set("Accept", accept)
            .set("X-KB-Actor", &self.actor);
        decode_bytes_response(request.call())
    }
}

fn decode_response<T>(response: Result<ureq::Response, ureq::Error>) -> Result<T, ClientError>
where
    T: DeserializeOwned,
{
    match response {
        Ok(response) => response
            .into_json::<T>()
            .map_err(|error| ClientError::InvalidResponse(error.to_string())),
        Err(ureq::Error::Status(status, response)) => {
            let envelope = response.into_json::<ErrorEnvelope>().map_err(|error| {
                ClientError::InvalidResponse(format!(
                    "HTTP {status} did not contain the error envelope: {error}"
                ))
            })?;
            Err(ClientError::Api {
                status,
                code: envelope.error.code,
                message: envelope.error.message,
            })
        }
        Err(ureq::Error::Transport(error)) => {
            Err(ClientError::ServerUnavailable(error.to_string()))
        }
    }
}

fn decode_text_response(
    response: Result<ureq::Response, ureq::Error>,
) -> Result<(Option<String>, String), ClientError> {
    match response {
        Ok(response) => {
            let content_type = response.header("Content-Type").map(str::to_owned);
            let body = response
                .into_string()
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
            Ok((content_type, body))
        }
        Err(ureq::Error::Status(status, response)) => {
            let envelope = response.into_json::<ErrorEnvelope>().map_err(|error| {
                ClientError::InvalidResponse(format!(
                    "HTTP {status} 响应不包含标准错误 envelope：{error}"
                ))
            })?;
            Err(ClientError::Api {
                status,
                code: envelope.error.code,
                message: envelope.error.message,
            })
        }
        Err(ureq::Error::Transport(error)) => {
            Err(ClientError::ServerUnavailable(error.to_string()))
        }
    }
}

fn decode_bytes_response(
    response: Result<ureq::Response, ureq::Error>,
) -> Result<(Option<String>, Option<String>, Option<String>, Vec<u8>), ClientError> {
    match response {
        Ok(response) => {
            let content_type = response.header("Content-Type").map(str::to_owned);
            let attachment_id = response.header("X-KB-Attachment-ID").map(str::to_owned);
            let sha256 = response.header("X-KB-Attachment-SHA256").map(str::to_owned);
            let mut reader = response.into_reader();
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))?;
            Ok((content_type, attachment_id, sha256, bytes))
        }
        Err(ureq::Error::Status(status, response)) => {
            let envelope = response.into_json::<ErrorEnvelope>().map_err(|error| {
                ClientError::InvalidResponse(format!(
                    "HTTP {status} did not contain the error envelope: {error}"
                ))
            })?;
            Err(ClientError::Api {
                status,
                code: envelope.error.code,
                message: envelope.error.message,
            })
        }
        Err(ureq::Error::Transport(error)) => {
            Err(ClientError::ServerUnavailable(error.to_string()))
        }
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

pub(crate) fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_path_segment("board/#1"), "board%2F%231");
    }
}
