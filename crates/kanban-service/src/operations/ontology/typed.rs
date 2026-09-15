//! Host 共用的具名 ontology facade；写方法在整个 store 操作与返回转换期间持有 mutation gate。

use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, LabelAtomRecord, LabelSemanticsRecord, error::store_error};

use super::{commands::*, records::*};

fn ontology_board(board: &str) -> Result<&str> {
    let board = board.trim();
    if board.is_empty() {
        return Err(KanbanError::InvalidInput(
            "board is required for label ontology operation".to_owned(),
        ));
    }
    Ok(board)
}

impl<C: Clock> KanbanService<C> {
    pub async fn list_label_semantics(&self, board: &str) -> Result<Vec<LabelSemanticsRecord>> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .list_label_semantics(board)
            .await
            .map_err(store_error)?;
        Ok(record.into_iter().map(Into::into).collect())
    }

    pub async fn get_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
    ) -> Result<LabelSemanticsRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .get_label_semantics(board, label_ref)
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn upsert_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        command: UpsertLabelSemanticsCommand,
    ) -> Result<LabelSemanticsRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .upsert_label_semantics(board, label_ref, command.into())
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn delete_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        command: DeleteLabelSemanticsCommand,
    ) -> Result<bool> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .delete_label_semantics(
                board,
                label_ref,
                &command.expected_semantics_hash,
                &command.reason,
                &command.actor,
            )
            .await
            .map_err(store_error)?;
        Ok(record)
    }

    pub async fn list_label_atoms(&self, board: &str) -> Result<Vec<LabelAtomRecord>> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .list_label_atoms(board)
            .await
            .map_err(store_error)?;
        Ok(record.into_iter().map(Into::into).collect())
    }

    pub async fn explain_label_atom(
        &self,
        board: &str,
        atom_ref: &str,
    ) -> Result<LabelAtomExplainRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .explain_label_atom(board, atom_ref)
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn label_atom_index_status(&self, board: &str) -> Result<LabelAtomIndexStatusRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .label_atom_index_status(board)
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn rebuild_label_atom_index(
        &self,
        board: &str,
    ) -> Result<LabelAtomIndexStatusRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .rebuild_label_atom_index(board)
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn query_label_atom_index(
        &self,
        board: &str,
        query: LabelAtomIndexQuery,
    ) -> Result<LabelAtomIndexQueryRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .query_label_atom_index(
                board,
                query.query.as_deref(),
                query.polarity.as_deref(),
                query.limit,
            )
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn suggest_task_labels(
        &self,
        board: &str,
        task_ref: &str,
        options: LabelSuggestionOptions,
    ) -> Result<LabelSuggestionResultRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .suggest_task_labels(board, task_ref, options.into())
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn propose_task_label(
        &self,
        board: &str,
        task_ref: &str,
        command: LabelProposalCommand,
    ) -> Result<LabelProposalAttemptRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .propose_task_label(board, task_ref, command.into())
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn list_label_proposals(
        &self,
        board: &str,
        task_ref: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<LabelSemanticProposalRecord>> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .list_label_proposals(board, task_ref, status)
            .await
            .map_err(store_error)?;
        Ok(record.into_iter().map(Into::into).collect())
    }

    pub async fn get_label_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<LabelSemanticProposalRecord> {
        let record = self
            .store
            .get_label_proposal(proposal_id)
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn decide_label_proposal(
        &self,
        command: LabelProposalDecisionCommand,
    ) -> Result<LabelSemanticProposalRecord> {
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .decide_label_proposal(command.into())
            .await
            .map_err(store_error)?;
        Ok(record.into())
    }

    pub async fn record_label_ontology_observation(
        &self,
        board: &str,
        command: OntologyObservationCommand,
    ) -> Result<LabelOntologyObservationRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .record_label_ontology_observation(board, command.into())
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn list_label_ontology_signals(
        &self,
        board: &str,
        query: LabelOntologySignalQuery,
    ) -> Result<Vec<LabelOntologySignalRecord>> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .list_label_ontology_signals(
                board,
                &query.statuses,
                &query.kinds,
                (
                    query.task_ref.as_deref(),
                    query.target_label_ref.as_deref(),
                    query.proposed_label_name.as_deref(),
                ),
                query.include_all,
                query.limit,
            )
            .await
            .map_err(store_error)?;
        record.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn get_label_ontology_signal(
        &self,
        signal_id: &str,
    ) -> Result<LabelOntologySignalDetailRecord> {
        let record = self
            .store
            .get_label_ontology_signal(signal_id)
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn review_label_ontology(
        &self,
        board: &str,
        query: LabelOntologyReviewQuery,
    ) -> Result<Vec<LabelOntologyReviewGroupRecord>> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .review_label_ontology(board, &query.group_by, query.include_all, query.limit)
            .await
            .map_err(store_error)?;
        record.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn create_label_ontology_action(
        &self,
        board: &str,
        command: OntologyActionCommand,
    ) -> Result<LabelOntologyActionRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .create_label_ontology_action(board, command.into())
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn apply_label_ontology_atom(
        &self,
        board: &str,
        command: OntologyApplyAtomCommand,
    ) -> Result<LabelOntologyActionRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .apply_label_ontology_atom(board, command.into())
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn revert_label_ontology_mutation(
        &self,
        board: &str,
        command: OntologyRevertCommand,
    ) -> Result<LabelOntologyActionRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .revert_label_ontology_mutation(board, command.into())
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn validate_label_ontology_action(
        &self,
        board: &str,
        command: OntologyValidateCommand,
    ) -> Result<LabelOntologyActionRecord> {
        let board = ontology_board(board)?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .validate_label_ontology_action(board, command.into())
            .await
            .map_err(store_error)?;
        record.try_into()
    }

    pub async fn label_ontology_quality(
        &self,
        board: &str,
        sample_limit: usize,
    ) -> Result<LabelOntologyQualityRecord> {
        let board = ontology_board(board)?;
        let record = self
            .store
            .label_ontology_quality(board, sample_limit)
            .await
            .map_err(store_error)?;
        record.try_into()
    }
}
