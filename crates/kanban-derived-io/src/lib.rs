use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperEnvelope {
    pub protocol: String,
    pub payload_json: String,
}

#[derive(Debug, Error)]
pub enum DerivedIoError {
    #[error("derived helper IO is not implemented yet")]
    NotImplemented,
}
