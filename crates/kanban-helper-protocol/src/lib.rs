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
        deserialize_json_with_path(&self.payload_json, "payload_json")
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let envelope: Self = deserialize_json_with_path(json, "helper envelope")?;
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

fn deserialize_json_with_path<T: DeserializeOwned>(json: &str, context: &str) -> Result<T> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|err| {
        let path = json_path(context, &err.path().to_string());
        invalid_json_with_path(context, path, err.into_inner())
    })?;
    deserializer
        .end()
        .map_err(|err| invalid_json_with_path(context, json_path(context, "."), err))?;
    Ok(value)
}

fn invalid_json_with_path(context: &str, path: String, source: serde_json::Error) -> KanbanError {
    KanbanError::InvalidInput(format!("{context} parse error at {path}: {source}"))
}

fn json_path(context: &str, path: &str) -> String {
    if path == "." {
        context.to_owned()
    } else if context == "payload_json" {
        format!("payload_json.{path}")
    } else {
        path.to_owned()
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

    #[test]
    fn decode_error_includes_payload_json_path() {
        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        struct NestedPayload {
            nested: Nested,
        }

        #[allow(dead_code)]
        #[derive(Debug, Deserialize)]
        struct Nested {
            count: usize,
        }

        let envelope = HelperEnvelope {
            protocol: HELPER_PROTOCOL.to_owned(),
            payload_json: r#"{"nested":{"count":"many"}}"#.to_owned(),
        };

        let error = envelope.decode::<NestedPayload>().unwrap_err().to_string();

        assert!(error.contains("payload_json.nested.count"), "{error}");
    }

    #[test]
    fn from_json_rejects_trailing_characters() {
        let envelope = HelperEnvelope::new(Payload {
            value: "ok".to_owned(),
        })
        .unwrap()
        .to_json()
        .unwrap();
        let json = format!("{envelope} trailing");

        let error = HelperEnvelope::from_json(&json).unwrap_err().to_string();

        assert!(error.contains("helper envelope"), "{error}");
        assert!(error.contains("trailing"), "{error}");
    }

    #[test]
    fn decode_rejects_payload_trailing_characters() {
        let envelope = HelperEnvelope {
            protocol: HELPER_PROTOCOL.to_owned(),
            payload_json: r#"{"value":"ok"} trailing"#.to_owned(),
        };

        let error = envelope.decode::<Payload>().unwrap_err().to_string();

        assert!(error.contains("payload_json"), "{error}");
        assert!(error.contains("trailing"), "{error}");
    }

    #[test]
    fn from_json_error_includes_envelope_json_path() {
        let error = HelperEnvelope::from_json(r#"{"protocol":7,"payload_json":"{}"}"#)
            .unwrap_err()
            .to_string();

        assert!(error.contains("protocol"), "{error}");
    }
}
