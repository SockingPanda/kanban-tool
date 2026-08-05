use serde::{Deserialize, Serialize};

use crate::{MetadataEnvelope, NextAfterMeta, StreamEventData};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListEventsResponse {
    pub data: Vec<StreamEventData>,
    pub meta: NextAfterMeta,
}

impl ListEventsResponse {
    pub fn new(data: Vec<StreamEventData>, meta: NextAfterMeta) -> Self {
        Self { data, meta }
    }
}

impl From<MetadataEnvelope<Vec<StreamEventData>, NextAfterMeta>> for ListEventsResponse {
    fn from(envelope: MetadataEnvelope<Vec<StreamEventData>, NextAfterMeta>) -> Self {
        Self::new(envelope.data, envelope.meta)
    }
}
