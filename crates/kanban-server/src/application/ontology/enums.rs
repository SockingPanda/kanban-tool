//! Wire 枚举的封闭映射；新增 variant 必须显式处理，未知存储 literal 报错。

use kanban_protocol as wire;
use kanban_service::KanbanError;

use crate::error::ApiError;

pub(super) fn signal_kind_literal(value: &wire::LabelOntologySignalKindWire) -> &'static str {
    match value {
        wire::LabelOntologySignalKindWire::FalseNegative => "false_negative",
        wire::LabelOntologySignalKindWire::FalsePositive => "false_positive",
        wire::LabelOntologySignalKindWire::VocabularyGap => "vocabulary_gap",
        wire::LabelOntologySignalKindWire::NameIssue => "name_issue",
        wire::LabelOntologySignalKindWire::BoundaryIssue => "boundary_issue",
        wire::LabelOntologySignalKindWire::StructureIssue => "structure_issue",
    }
}

pub(super) fn proposed_action_literal(
    value: &wire::LabelOntologyProposedActionWire,
) -> &'static str {
    match value {
        wire::LabelOntologyProposedActionWire::Observe => "observe",
        wire::LabelOntologyProposedActionWire::AddPositiveAtom => "add_positive_atom",
        wire::LabelOntologyProposedActionWire::AddNegativeAtom => "add_negative_atom",
        wire::LabelOntologyProposedActionWire::UpdateSemantics => "update_semantics",
        wire::LabelOntologyProposedActionWire::BootstrapLabel => "bootstrap_label",
        wire::LabelOntologyProposedActionWire::RenameLabel => "rename_label",
        wire::LabelOntologyProposedActionWire::SplitLabel => "split_label",
        wire::LabelOntologyProposedActionWire::MergeLabels => "merge_labels",
    }
}

pub(super) fn suggest_state_literal(value: &wire::LabelOntologySuggestStateWire) -> &'static str {
    match value {
        wire::LabelOntologySuggestStateWire::Selected => "selected",
        wire::LabelOntologySuggestStateWire::Candidate => "candidate",
        wire::LabelOntologySuggestStateWire::Absent => "absent",
        wire::LabelOntologySuggestStateWire::Unavailable => "unavailable",
    }
}

pub(super) fn proposal_status_literal(value: &wire::LabelProposalStatusWire) -> &'static str {
    match value {
        wire::LabelProposalStatusWire::Proposed => "proposed",
        wire::LabelProposalStatusWire::Accepted => "accepted",
        wire::LabelProposalStatusWire::Rejected => "rejected",
    }
}

pub(super) fn review_group_literal(value: &wire::LabelOntologyReviewGroupByWire) -> &'static str {
    match value {
        wire::LabelOntologyReviewGroupByWire::Label => "label",
        wire::LabelOntologyReviewGroupByWire::CandidateAtom => "candidate_atom",
        wire::LabelOntologyReviewGroupByWire::ProposedLabel => "proposed_label",
    }
}

pub(super) fn action_type_literal(value: &wire::LabelOntologyActionTypeWire) -> &'static str {
    match value {
        wire::LabelOntologyActionTypeWire::Confirm => "confirm",
        wire::LabelOntologyActionTypeWire::Reject => "reject",
        wire::LabelOntologyActionTypeWire::Supersede => "supersede",
        wire::LabelOntologyActionTypeWire::ResolveNoChange => "resolve_no_change",
        wire::LabelOntologyActionTypeWire::AddPositiveAtom => "add_positive_atom",
        wire::LabelOntologyActionTypeWire::AddNegativeAtom => "add_negative_atom",
        wire::LabelOntologyActionTypeWire::AdoptExistingAtom => "adopt_existing_atom",
        wire::LabelOntologyActionTypeWire::UpdateSemantics => "update_semantics",
        wire::LabelOntologyActionTypeWire::CreateLabelProposal => "create_label_proposal",
        wire::LabelOntologyActionTypeWire::BootstrapLabel => "bootstrap_label",
        wire::LabelOntologyActionTypeWire::RenameLabel => "rename_label",
        wire::LabelOntologyActionTypeWire::SplitLabel => "split_label",
        wire::LabelOntologyActionTypeWire::MergeLabels => "merge_labels",
        wire::LabelOntologyActionTypeWire::RevertOntologyMutation => "revert_ontology_mutation",
        wire::LabelOntologyActionTypeWire::Validate => "validate",
    }
}

pub(super) fn validation_status_literal(
    value: &wire::LabelOntologyValidationStatusWire,
) -> &'static str {
    match value {
        wire::LabelOntologyValidationStatusWire::NotRequired => "not_required",
        wire::LabelOntologyValidationStatusWire::Pending => "pending",
        wire::LabelOntologyValidationStatusWire::Passed => "passed",
        wire::LabelOntologyValidationStatusWire::Failed => "failed",
        wire::LabelOntologyValidationStatusWire::Partial => "partial",
    }
}

pub(super) fn parse_proposal_status(
    value: &str,
) -> Result<wire::LabelProposalStatusWire, ApiError> {
    match value {
        "proposed" => Ok(wire::LabelProposalStatusWire::Proposed),
        "accepted" => Ok(wire::LabelProposalStatusWire::Accepted),
        "rejected" => Ok(wire::LabelProposalStatusWire::Rejected),
        _ => {
            Err(KanbanError::Storage(format!("ontology proposal status 枚举无效：{value}")).into())
        }
    }
}

pub(super) fn parse_action_type(
    value: &str,
) -> Result<wire::LabelOntologyActionTypeWire, ApiError> {
    match value {
        "confirm" => Ok(wire::LabelOntologyActionTypeWire::Confirm),
        "reject" => Ok(wire::LabelOntologyActionTypeWire::Reject),
        "supersede" => Ok(wire::LabelOntologyActionTypeWire::Supersede),
        "resolve_no_change" => Ok(wire::LabelOntologyActionTypeWire::ResolveNoChange),
        "add_positive_atom" => Ok(wire::LabelOntologyActionTypeWire::AddPositiveAtom),
        "add_negative_atom" => Ok(wire::LabelOntologyActionTypeWire::AddNegativeAtom),
        "adopt_existing_atom" => Ok(wire::LabelOntologyActionTypeWire::AdoptExistingAtom),
        "update_semantics" => Ok(wire::LabelOntologyActionTypeWire::UpdateSemantics),
        "create_label_proposal" => Ok(wire::LabelOntologyActionTypeWire::CreateLabelProposal),
        "bootstrap_label" => Ok(wire::LabelOntologyActionTypeWire::BootstrapLabel),
        "rename_label" => Ok(wire::LabelOntologyActionTypeWire::RenameLabel),
        "split_label" => Ok(wire::LabelOntologyActionTypeWire::SplitLabel),
        "merge_labels" => Ok(wire::LabelOntologyActionTypeWire::MergeLabels),
        "revert_ontology_mutation" => Ok(wire::LabelOntologyActionTypeWire::RevertOntologyMutation),
        "validate" => Ok(wire::LabelOntologyActionTypeWire::Validate),
        _ => Err(KanbanError::Storage(format!("ontology action_type 枚举无效：{value}")).into()),
    }
}

pub(super) fn parse_validation_requirement(
    value: &str,
) -> Result<wire::LabelOntologyValidationRequirementWire, ApiError> {
    match value {
        "none" => Ok(wire::LabelOntologyValidationRequirementWire::None),
        "required" => Ok(wire::LabelOntologyValidationRequirementWire::Required),
        "unsupported" => Ok(wire::LabelOntologyValidationRequirementWire::Unsupported),
        _ => Err(KanbanError::Storage(format!(
            "ontology validation_requirement 枚举无效：{value}"
        ))
        .into()),
    }
}

pub(super) fn parse_validation_status(
    value: &str,
) -> Result<wire::LabelOntologyValidationStatusWire, ApiError> {
    match value {
        "not_required" => Ok(wire::LabelOntologyValidationStatusWire::NotRequired),
        "pending" => Ok(wire::LabelOntologyValidationStatusWire::Pending),
        "passed" => Ok(wire::LabelOntologyValidationStatusWire::Passed),
        "failed" => Ok(wire::LabelOntologyValidationStatusWire::Failed),
        "partial" => Ok(wire::LabelOntologyValidationStatusWire::Partial),
        _ => Err(
            KanbanError::Storage(format!("ontology validation_status 枚举无效：{value}")).into(),
        ),
    }
}

pub(super) fn parse_validation_outcome(
    value: &str,
) -> Result<wire::LabelOntologyValidationEffectiveOutcomeWire, ApiError> {
    match value {
        "not_required" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::NotRequired),
        "unsupported" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::Unsupported),
        "pending" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::Pending),
        "passed" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::Passed),
        "failed" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::Failed),
        "partial" => Ok(wire::LabelOntologyValidationEffectiveOutcomeWire::Partial),
        _ => Err(KanbanError::Storage(format!(
            "ontology validation_effective_outcome 枚举无效：{value}"
        ))
        .into()),
    }
}

pub(super) fn parse_review_group(
    value: &str,
) -> Result<wire::LabelOntologyReviewGroupByWire, ApiError> {
    match value {
        "label" => Ok(wire::LabelOntologyReviewGroupByWire::Label),
        "candidate_atom" => Ok(wire::LabelOntologyReviewGroupByWire::CandidateAtom),
        "proposed_label" => Ok(wire::LabelOntologyReviewGroupByWire::ProposedLabel),
        _ => Err(KanbanError::Storage(format!("ontology group_by 枚举无效：{value}")).into()),
    }
}
