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
transition_path!(ReleaseTaskPath);
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
pub type ReleaseTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type CompleteTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type SubmitReviewTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type BlockTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type UnblockTaskResponse = crate::DataEnvelope<crate::ApiTask>;
pub type ArchiveTaskResponse = crate::DataEnvelope<crate::ApiTask>;

#[cfg(test)]
mod tests {
    use super::{
        BlockTaskPath, BlockTaskResponse, CompleteTaskPath, CompleteTaskResponse, ReleaseTaskPath,
        ReleaseTaskResponse, SubmitReviewTaskPath, SubmitReviewTaskResponse,
    };

    #[test]
    fn release_task_path_contract() {
        let path: ReleaseTaskPath =
            serde_json::from_value(serde_json::json!({"task_id": "t_fixture"})).unwrap();
        assert_eq!(path.task_id, "t_fixture");
    }

    #[test]
    fn release_task_response_contract() {
        let fixture =
            include_str!("../../../schemas/fixtures/api/release-task-response.v1.valid.json");
        let response: ReleaseTaskResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(response.data.id, "t_fixture");
        assert_eq!(response.data.status.as_str(), "ready");
    }

    #[test]
    fn submit_review_task_path_contract() {
        let path: SubmitReviewTaskPath =
            serde_json::from_value(serde_json::json!({"task_id": "t_fixture"})).unwrap();
        assert_eq!(path.task_id, "t_fixture");
    }

    #[test]
    fn submit_review_task_response_contract() {
        let fixture =
            include_str!("../../../schemas/fixtures/api/submit-review-task-response.v1.valid.json");
        let response: SubmitReviewTaskResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(response.data.id, "t_fixture");
        assert_eq!(response.data.status.as_str(), "review");
    }

    #[test]
    fn complete_task_path_contract() {
        let path: CompleteTaskPath =
            serde_json::from_value(serde_json::json!({"task_id": "t_fixture"})).unwrap();
        assert_eq!(path.task_id, "t_fixture");
    }

    #[test]
    fn complete_task_response_contract() {
        let fixture =
            include_str!("../../../schemas/fixtures/api/complete-task-response.v1.valid.json");
        let response: CompleteTaskResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(response.data.id, "t_fixture");
        assert_eq!(response.data.status.as_str(), "done");
    }

    #[test]
    fn block_task_path_contract() {
        let fixture = include_str!("../../../schemas/fixtures/api/block-task-path.v1.valid.json");
        let path: BlockTaskPath = serde_json::from_str(fixture).unwrap();
        assert_eq!(path.task_id, "t_fixture");
        assert_eq!(
            serde_json::to_value(path).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }

    #[test]
    fn block_task_response_contract() {
        let fixture =
            include_str!("../../../schemas/fixtures/api/block-task-response.v1.valid.json");
        let response: BlockTaskResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(response.data.id, "t_fixture");
        assert_eq!(response.data.status.as_str(), "blocked");
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }
}
