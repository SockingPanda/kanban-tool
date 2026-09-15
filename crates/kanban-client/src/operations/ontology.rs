//! 面向 label semantics 与 ontology surface 的 typed localhost 客户端。

use kanban_protocol::{ListBoardLabelProposalsResponse, ListTaskLabelProposalsResponse};
use serde_json::Value;

use crate::{KanbanClient, error::ClientError, transport::rpc};

mod compat;

use compat::{
    actor_input, decision_input, from_value, input_without_selector, proposal_input, review_query,
    signals_query, suggestion_query,
};

fn data<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ClientError> {
    value
        .get("data")
        .cloned()
        .ok_or_else(|| ClientError::InvalidResponse("响应缺少 data".to_owned()))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        })
}

impl KanbanClient {
    pub async fn list_label_semantics(&self, board: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::ListLabelSemanticsResponse = rpc!(
            self,
            list_label_semantics,
            ListLabelSemanticsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn get_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::GetLabelSemanticsResponse = rpc!(
            self,
            get_label_semantics,
            GetLabelSemanticsRequest,
            kanban_protocol::LabelSemanticsPath {
                board: board.to_owned(),
                label_id: label_ref.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn upsert_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::UpsertLabelSemanticsResponse = rpc!(
            self,
            upsert_label_semantics,
            UpsertLabelSemanticsRequest,
            kanban_protocol::LabelSemanticsPath {
                board: board.to_owned(),
                label_id: label_ref.to_owned()
            },
            (),
            input_without_selector(body, "label_ref")?
        )?;
        to_value(response)
    }

    pub async fn delete_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        expected_hash: &str,
        reason: &str,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::DeleteResponse = rpc!(
            self,
            delete_label_semantics,
            DeleteLabelSemanticsRequest,
            kanban_protocol::LabelSemanticsPath {
                board: board.to_owned(),
                label_id: label_ref.to_owned()
            },
            kanban_protocol::DeleteLabelSemanticsQuery {
                expected_semantics_hash: expected_hash.to_owned(),
                reason: reason.to_owned()
            },
            ()
        )?;
        to_value(response)
    }

    pub async fn list_label_atoms(&self, board: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::ListLabelAtomsResponse = rpc!(
            self,
            list_label_atoms,
            ListLabelAtomsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn explain_label_atom(
        &self,
        board: &str,
        atom_ref: &str,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::ExplainLabelAtomResponse = rpc!(
            self,
            explain_label_atom,
            ExplainLabelAtomRequest,
            kanban_protocol::LabelAtomPath {
                board: board.to_owned(),
                atom_ref: atom_ref.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn label_atom_index_status(&self, board: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelAtomIndexStatusResponse = rpc!(
            self,
            label_atom_index_status,
            LabelAtomIndexStatusRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn rebuild_label_atom_index(&self, board: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::RebuildLabelAtomIndexResponse = rpc!(
            self,
            rebuild_label_atom_index,
            RebuildLabelAtomIndexRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn query_label_atom_index(
        &self,
        board: &str,
        query: Option<&str>,
        polarity: Option<&str>,
        limit: usize,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::rpc::dto::LabelAtomIndexQueryResponse = rpc!(
            self,
            query_label_atom_index,
            QueryLabelAtomIndexRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            kanban_protocol::LabelAtomIndexQuery {
                q: query.map(str::to_owned),
                polarity: polarity.map(str::to_owned),
                limit,
                vector_json: None,
                embedding_model: None,
                include_vector: false
            },
            ()
        )?;
        to_value(kanban_protocol::DataEnvelope::new(response.data.data))
    }

    pub async fn suggest_task_labels(
        &self,
        task_id: &str,
        board: Option<&str>,
        options: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::SuggestTaskLabelsResponse = rpc!(
            self,
            suggest_task_labels,
            SuggestTaskLabelsRequest,
            kanban_protocol::TaskLabelSurfacePath {
                task_id: task_id.to_owned()
            },
            suggestion_query(board.or(Some("default")), options)?,
            ()
        )?;
        to_value(response)
    }

    pub async fn list_label_proposals(
        &self,
        board: &str,
        task_id: Option<&str>,
        status: Option<&str>,
    ) -> Result<Value, ClientError> {
        if let Some(task_id) = task_id {
            let response = self
                .list_task_label_proposals(board, task_id, status)
                .await?;
            serde_json::to_value(response)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        } else {
            let response = self.list_board_label_proposals(board, status).await?;
            serde_json::to_value(response)
                .map_err(|error| ClientError::InvalidResponse(error.to_string()))
        }
    }

    pub async fn list_task_label_proposals(
        &self,
        board: &str,
        task_id: &str,
        status: Option<&str>,
    ) -> Result<ListTaskLabelProposalsResponse, ClientError> {
        let response: kanban_protocol::ListTaskLabelProposalsResponse = rpc!(
            self,
            list_task_label_proposals,
            ListTaskLabelProposalsRequest,
            kanban_protocol::TaskLabelSurfacePath {
                task_id: task_id.to_owned()
            },
            kanban_protocol::rpc::dto::TaskLabelProposalQuery {
                board: Some(board.to_owned()),
                status: status.map(str::to_owned)
            },
            ()
        )?;
        Ok(response)
    }

    pub async fn list_board_label_proposals(
        &self,
        board: &str,
        status: Option<&str>,
    ) -> Result<ListBoardLabelProposalsResponse, ClientError> {
        let response: kanban_protocol::ListBoardLabelProposalsResponse = rpc!(
            self,
            list_board_label_proposals,
            ListBoardLabelProposalsRequest,
            kanban_protocol::ListBoardLabelProposalsPath {
                board: board.to_owned()
            },
            kanban_protocol::ListBoardLabelProposalsQuery {
                status: status
                    .map(|status| from_value(serde_json::Value::String(status.to_owned())))
                    .transpose()?
            },
            ()
        )?;
        Ok(response)
    }

    pub async fn propose_task_label(
        &self,
        board: &str,
        task_id: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::ProposeTaskLabelResponse = rpc!(
            self,
            propose_task_label,
            ProposeTaskLabelRequest,
            kanban_protocol::TaskLabelSurfacePath {
                task_id: task_id.to_owned()
            },
            suggestion_query(Some(board), serde_json::json!({}))?,
            proposal_input(body)?
        )?;
        to_value(response)
    }

    pub async fn get_label_proposal(&self, proposal_id: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::GetLabelProposalResponse = rpc!(
            self,
            get_label_proposal,
            GetLabelProposalRequest,
            kanban_protocol::ProposalPath {
                proposal_id: proposal_id.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn decide_label_proposal(
        &self,
        proposal_id: &str,
        accept: bool,
        body: Value,
    ) -> Result<Value, ClientError> {
        let path = kanban_protocol::ProposalPath {
            proposal_id: proposal_id.to_owned(),
        };
        let request = decision_input(body)?;
        let response: kanban_protocol::LabelProposalDecisionResponse = if accept {
            rpc!(
                self,
                accept_label_proposal,
                AcceptLabelProposalRequest,
                path,
                (),
                request
            )?
        } else {
            rpc!(
                self,
                reject_label_proposal,
                RejectLabelProposalRequest,
                path,
                (),
                request
            )?
        };
        to_value(response)
    }

    pub async fn record_label_ontology_observation(
        &self,
        board: &str,
        task_id: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::RecordLabelOntologyObservationResponse = rpc!(
            self,
            record_label_ontology_observation,
            RecordLabelOntologyObservationRequest,
            kanban_protocol::TaskLabelSurfacePath {
                task_id: task_id.to_owned()
            },
            kanban_protocol::rpc::dto::TaskLabelBoardQuery {
                board: Some(board.to_owned())
            },
            actor_input(body, Some("task_ref"))?
        )?;
        to_value(response)
    }

    pub async fn list_label_ontology_signals(
        &self,
        board: &str,
        query: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelOntologySignalsResponse = rpc!(
            self,
            list_label_ontology_signals,
            ListLabelOntologySignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            signals_query(query)?,
            ()
        )?;
        to_value(response)
    }

    pub async fn get_label_ontology_signal(&self, signal_id: &str) -> Result<Value, ClientError> {
        let response: kanban_protocol::GetLabelOntologySignalResponse = rpc!(
            self,
            get_label_ontology_signal,
            GetLabelOntologySignalRequest,
            kanban_protocol::SignalPath {
                signal_id: signal_id.to_owned()
            },
            (),
            ()
        )?;
        to_value(response)
    }

    pub async fn review_label_ontology(
        &self,
        board: &str,
        query: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::ReviewLabelOntologyResponse = rpc!(
            self,
            review_label_ontology,
            ReviewLabelOntologyRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            review_query(query)?,
            ()
        )?;
        to_value(response)
    }

    pub async fn create_label_ontology_action(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelOntologyActionResponse = rpc!(
            self,
            create_label_ontology_action,
            CreateLabelOntologyActionRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            actor_input(body, None)?
        )?;
        to_value(response)
    }

    pub async fn apply_label_ontology_atom(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelOntologyActionResponse = rpc!(
            self,
            apply_label_ontology_atom,
            ApplyLabelOntologyAtomRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            actor_input(body, None)?
        )?;
        to_value(response)
    }

    pub async fn revert_label_ontology(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelOntologyActionResponse = rpc!(
            self,
            revert_label_ontology_mutation,
            RevertLabelOntologyMutationRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            actor_input(body, None)?
        )?;
        to_value(response)
    }

    pub async fn validate_label_ontology(
        &self,
        board: &str,
        body: Value,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::LabelOntologyActionResponse = rpc!(
            self,
            validate_label_ontology_action,
            ValidateLabelOntologyActionRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            actor_input(body, None)?
        )?;
        to_value(response)
    }

    pub async fn label_ontology_quality(
        &self,
        board: &str,
        sample_limit: usize,
    ) -> Result<Value, ClientError> {
        let response: kanban_protocol::cli_labels::CliLabelOntologyQualityOutput = rpc!(
            self,
            get_label_ontology_quality,
            GetLabelOntologyQualityRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            kanban_protocol::rpc::dto::LabelOntologyQualityQuery { sample_limit },
            ()
        )?;
        to_value(response)
    }

    pub fn ontology_data<T: serde::de::DeserializeOwned>(
        &self,
        response: Value,
    ) -> Result<T, ClientError> {
        data(response)
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, ClientError> {
    serde_json::to_value(value).map_err(|error| ClientError::InvalidResponse(error.to_string()))
}
