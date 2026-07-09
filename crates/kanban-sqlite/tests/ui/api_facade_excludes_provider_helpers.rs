#![allow(unused_imports)]

use kanban_sqlite::api::{
    DisabledLabelProposalProvider, LabelOntologyTrustedValidationInput, LabelProposalProvider,
    ManualLabelProposalProvider, build_context_pack_with_vector_store, label_atom_index_status_with,
    propose_task_label_with, propose_task_label_with_create_options, propose_task_label_with_store,
    propose_task_label_with_store_and_create_options, query_label_atom_index_by_vector_with,
    query_label_atom_index_with, rebuild_label_atom_index_with, rebuild_vector_store_with,
    suggest_task_labels_with, sync_vector_store_with, vector_store_status_with,
    validate_label_ontology_action_with_trusted_suggestions,
};

fn main() {}
