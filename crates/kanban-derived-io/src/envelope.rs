use kanban_core::{KanbanError, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperEnvelope {
    pub protocol: String,
    pub payload_json: String,
}

impl HelperEnvelope {
    pub const PROTOCOL: &'static str = "kanban-derived-helper.v1";

    pub fn new(payload: impl Serialize) -> Result<Self> {
        Self::with_protocol(Self::PROTOCOL, payload)
    }

    pub fn with_protocol(protocol: impl Into<String>, payload: impl Serialize) -> Result<Self> {
        Ok(Self {
            protocol: protocol.into(),
            payload_json: serde_json::to_string(&payload)
                .map_err(|err| KanbanError::InvalidInput(err.to_string()))?,
        })
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        if self.protocol != Self::PROTOCOL {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported helper protocol: {}",
                self.protocol
            )));
        }
        serde_json::from_str(&self.payload_json)
            .map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::HelperEnvelope;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Payload {
        value: String,
    }

    #[test]
    fn envelope_round_trips_payload() {
        let envelope = HelperEnvelope::new(Payload {
            value: "ok".to_owned(),
        })
        .unwrap();
        let decoded: Payload = envelope.decode().unwrap();
        assert_eq!(
            decoded,
            Payload {
                value: "ok".to_owned()
            }
        );
        assert_eq!(
            HelperEnvelope::from_json(&envelope.to_json().unwrap()).unwrap(),
            envelope
        );
    }
}
