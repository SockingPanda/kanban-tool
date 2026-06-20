-- Add schema-level board isolation for ontology ledger and proposal links.
--
-- Historical atom references remain soft references. Nullable links keep their
-- existing ON DELETE SET NULL behavior and are board-scoped by triggers.

PRAGMA foreign_keys=OFF;

BEGIN;

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_observations_id_board
  ON label_ontology_observations(id, board_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_ontology_signals_id_board
  ON label_ontology_signals(id, board_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_label_semantic_proposals_id_board
  ON label_semantic_proposals(id, board_id);

DROP TABLE IF EXISTS label_ontology_action_signals_new;

CREATE TABLE label_ontology_action_signals_new (
  board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
  action_id TEXT NOT NULL,
  signal_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY(action_id, signal_id),
  FOREIGN KEY(action_id, board_id) REFERENCES label_ontology_actions(id, board_id) ON DELETE CASCADE,
  FOREIGN KEY(signal_id, board_id) REFERENCES label_ontology_signals(id, board_id) ON DELETE CASCADE
);

INSERT INTO label_ontology_action_signals_new(board_id, action_id, signal_id, created_at)
SELECT board_id, action_id, signal_id, created_at
FROM label_ontology_action_signals;

DROP TABLE label_ontology_action_signals;
ALTER TABLE label_ontology_action_signals_new RENAME TO label_ontology_action_signals;

CREATE INDEX IF NOT EXISTS idx_label_ontology_action_signals_signal
  ON label_ontology_action_signals(signal_id, action_id);

DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_insert;
DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_update;
DROP TRIGGER IF EXISTS trg_label_ontology_actions_board_insert;
DROP TRIGGER IF EXISTS trg_label_ontology_actions_board_update;
DROP TRIGGER IF EXISTS trg_label_semantic_proposals_board_insert;
DROP TRIGGER IF EXISTS trg_label_semantic_proposals_board_update;

CREATE TRIGGER trg_label_ontology_signals_board_insert
BEFORE INSERT ON label_ontology_signals
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match observation_id board_id')
    WHERE NOT EXISTS (
      SELECT 1 FROM label_ontology_observations
      WHERE id = NEW.observation_id
        AND board_id = NEW.board_id
    );
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match target_label_id board_id')
    WHERE NEW.target_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.target_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match superseded_by_signal_id board_id')
    WHERE NEW.superseded_by_signal_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_ontology_signals
        WHERE id = NEW.superseded_by_signal_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_label_ontology_signals_board_update
BEFORE UPDATE OF board_id, observation_id, target_label_id, superseded_by_signal_id ON label_ontology_signals
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match observation_id board_id')
    WHERE NOT EXISTS (
      SELECT 1 FROM label_ontology_observations
      WHERE id = NEW.observation_id
        AND board_id = NEW.board_id
    );
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match target_label_id board_id')
    WHERE NEW.target_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.target_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_signals.board_id must match superseded_by_signal_id board_id')
    WHERE NEW.superseded_by_signal_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_ontology_signals
        WHERE id = NEW.superseded_by_signal_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_label_ontology_actions_board_insert
BEFORE INSERT ON label_ontology_actions
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match parent_action_id board_id')
    WHERE NEW.parent_action_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_ontology_actions
        WHERE id = NEW.parent_action_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match target_label_id board_id')
    WHERE NEW.target_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.target_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match result_label_id board_id')
    WHERE NEW.result_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.result_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match result_proposal_id board_id')
    WHERE NEW.result_proposal_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_semantic_proposals
        WHERE id = NEW.result_proposal_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_label_ontology_actions_board_update
BEFORE UPDATE OF board_id, parent_action_id, target_label_id, result_label_id, result_proposal_id ON label_ontology_actions
BEGIN
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match parent_action_id board_id')
    WHERE NEW.parent_action_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_ontology_actions
        WHERE id = NEW.parent_action_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match target_label_id board_id')
    WHERE NEW.target_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.target_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match result_label_id board_id')
    WHERE NEW.result_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.result_label_id
          AND board_id = NEW.board_id
      );
  SELECT RAISE(ABORT, 'label_ontology_actions.board_id must match result_proposal_id board_id')
    WHERE NEW.result_proposal_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM label_semantic_proposals
        WHERE id = NEW.result_proposal_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_label_semantic_proposals_board_insert
BEFORE INSERT ON label_semantic_proposals
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals.board_id must match resolved_label_id board_id')
    WHERE NEW.resolved_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.resolved_label_id
          AND board_id = NEW.board_id
      );
END;

CREATE TRIGGER trg_label_semantic_proposals_board_update
BEFORE UPDATE OF board_id, resolved_label_id ON label_semantic_proposals
BEGIN
  SELECT RAISE(ABORT, 'label_semantic_proposals.board_id must match resolved_label_id board_id')
    WHERE NEW.resolved_label_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1 FROM labels
        WHERE id = NEW.resolved_label_id
          AND board_id = NEW.board_id
      );
END;

INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at)
VALUES (21, '021_board_isolation_ontology_links', '', CAST(strftime('%s','now') AS INTEGER) * 1000);

COMMIT;

PRAGMA foreign_keys=ON;
