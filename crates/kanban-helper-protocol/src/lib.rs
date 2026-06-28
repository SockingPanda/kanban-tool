use kanban_core::{KanbanError, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const HELPER_PROTOCOL: &str = "kanban-derived-helper.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperEnvelope {
    pub protocol: String,
    pub payload_json: String,
}

impl HelperEnvelope {
    pub const PROTOCOL: &'static str = HELPER_PROTOCOL;

    pub fn new(payload: impl Serialize) -> Result<Self> {
        Self::with_protocol(HELPER_PROTOCOL, payload)
    }

    pub fn with_protocol(protocol: impl Into<String>, payload: impl Serialize) -> Result<Self> {
        Ok(Self {
            protocol: protocol.into(),
            payload_json: serde_json::to_string(&payload)
                .map_err(|err| KanbanError::InvalidInput(err.to_string()))?,
        })
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        self.ensure_supported_protocol()?;
        serde_json::from_str(&self.payload_json)
            .map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let envelope: Self =
            serde_json::from_str(json).map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        envelope.ensure_supported_protocol()?;
        Ok(envelope)
    }

    fn ensure_supported_protocol(&self) -> Result<()> {
        if self.protocol != HELPER_PROTOCOL {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported helper protocol: {}",
                self.protocol
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{HELPER_PROTOCOL, HelperEnvelope};
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
        assert_eq!(envelope.protocol, HELPER_PROTOCOL);
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

    #[test]
    fn from_json_rejects_unknown_protocol() {
        let envelope = HelperEnvelope::with_protocol(
            "kanban-derived-helper.v0",
            Payload {
                value: "old".to_owned(),
            },
        )
        .unwrap();

        let error = HelperEnvelope::from_json(&envelope.to_json().unwrap()).unwrap_err();
        assert!(error.to_string().contains("unsupported helper protocol"));
    }
}
