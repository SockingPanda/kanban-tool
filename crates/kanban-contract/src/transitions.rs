use serde::{Deserialize, Serialize};

macro_rules! transition_path {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub task_id: String,
        }
    };
}

transition_path!(SpecifyTaskPath);
transition_path!(PromoteTaskPath);
transition_path!(ClaimTaskPath);
transition_path!(ReopenTaskPath);
transition_path!(ReclaimTaskPath);
transition_path!(HeartbeatTaskPath);
transition_path!(CompleteTaskPath);
transition_path!(SubmitReviewTaskPath);
transition_path!(BlockTaskPath);
transition_path!(UnblockTaskPath);
transition_path!(ArchiveTaskPath);

pub type SpecifyTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type PromoteTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type ClaimTaskResponse = crate::DataEnvelope<crate::ApiClaim>;
pub type ReopenTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type ReclaimTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type HeartbeatTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type CompleteTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type SubmitReviewTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type BlockTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type UnblockTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type ArchiveTaskResponse = crate::DataEnvelope<crate::ApiTask>;
