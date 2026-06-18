-- Ensure proposal creation provenance has a single canonical action per proposal.

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_actions_unique_create_proposal
  ON label_ontology_actions(board_id, result_proposal_id)
  WHERE action_type = 'create_label_proposal'
    AND result_proposal_id IS NOT NULL;

COMMIT;
