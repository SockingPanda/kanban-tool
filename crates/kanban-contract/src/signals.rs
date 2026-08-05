//! Generic signal ledger HTTP 请求与响应契约。

use crate::structured_metadata::SignalEvidence;
use crate::{ApiComment, DataEnvelope, SignalWire};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecordSignalRequest {
    pub kind: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub task_ref: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub comment_id: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub dedupe_key: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub evidence: Option<SignalEvidence>,
    #[serde(default)]
    pub comment: Option<SignalCommentRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SignalCommentRequest {
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SignalRecordResult {
    pub signal: SignalWire,
    pub backlink_comment: Option<ApiComment>,
}

pub type RecordSignalResponse = DataEnvelope<SignalRecordResult>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReviewSignalsRequest {
    pub signal_ids: Vec<String>,
    pub reason: String,
    #[serde(default, alias = "by")]
    pub replacement_signal_id: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_updated_at: Option<i64>,
}

pub type ConfirmSignalsResponse = DataEnvelope<Vec<SignalWire>>;
pub type RejectSignalsResponse = DataEnvelope<Vec<SignalWire>>;
pub type ResolveSignalsResponse = DataEnvelope<Vec<SignalWire>>;
pub type SupersedeSignalsResponse = DataEnvelope<Vec<SignalWire>>;
