use std::path::{Path, PathBuf};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_board_id};
use kanban_entity::PREDICATE_SEEDS;
use kanban_indexer::DERIVED_STORE_SEEDS;
use rusqlite::{Connection, OptionalExtension, params};

use serde::{Deserialize, Serialize};

use crate::connect_file;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/001_initial.sql");
const KNOWLEDGE_SUBSTRATE_MIGRATION: &str =
    include_str!("../../../migrations/002_knowledge_substrate.sql");
const COMMENT_AUTHOR_IDENTITY_MIGRATION: &str =
    include_str!("../../../migrations/003_comment_author_identity.sql");
const PRIORITY_LEVELS_MIGRATION: &str = include_str!("../../../migrations/004_priority_levels.sql");
const DECISION_COMMENT_KIND_MIGRATION: &str =
    include_str!("../../../migrations/005_decision_comment_kind.sql");
const COMMENT_METADATA_CONTRACT_MIGRATION: &str =
    include_str!("../../../migrations/006_comment_metadata_contract.sql");
const LABEL_SEMANTICS_ATOMS_MIGRATION: &str =
    include_str!("../../../migrations/007_label_semantics_atoms.sql");
const LABEL_ATOM_INDEX_BOARDS_MIGRATION: &str =
    include_str!("../../../migrations/008_label_atom_index_boards.sql");
const LABEL_SEMANTIC_PROPOSALS_MIGRATION: &str =
    include_str!("../../../migrations/009_label_semantic_proposals.sql");
const STABLE_LABEL_ATOM_HASHES_MIGRATION: &str =
    include_str!("../../../migrations/010_stable_label_atom_hashes.sql");
const LABEL_PROPOSAL_COSINE_COVERAGE_MIGRATION: &str =
    include_str!("../../../migrations/011_label_proposal_cosine_coverage.sql");
const LABEL_ONTOLOGY_LEDGER_MIGRATION: &str =
    include_str!("../../../migrations/012_label_ontology_ledger.sql");
const LABEL_ONTOLOGY_SUGGEST_INPUT_HASH_MIGRATION: &str =
    include_str!("../../../migrations/013_label_ontology_suggest_input_hash.sql");
const UNIQUE_LABEL_PROPOSAL_CREATE_ACTION_MIGRATION: &str =
    include_str!("../../../migrations/014_unique_label_proposal_create_action.sql");
const ADOPT_EXISTING_ATOM_ACTION_MIGRATION: &str =
    include_str!("../../../migrations/015_adopt_existing_atom_action.sql");
const REVERT_ONTOLOGY_MUTATION_ACTION_MIGRATION: &str =
    include_str!("../../../migrations/016_revert_ontology_mutation_action.sql");
const BOARD_ISOLATION_COMPOSITE_FK_MIGRATION: &str =
    include_str!("../../../migrations/017_board_isolation_composite_fk.sql");
const LABEL_ONTOLOGY_ROOT_ACTION_EFFECTS_MIGRATION: &str =
    include_str!("../../../migrations/018_label_ontology_root_action_effects.sql");
const LABEL_ONTOLOGY_VALIDATION_REQUIREMENT_MIGRATION: &str =
    include_str!("../../../migrations/019_label_ontology_validation_requirement.sql");
const BOARD_ISOLATION_TASK_HISTORY_MIGRATION: &str =
    include_str!("../../../migrations/020_board_isolation_task_history.sql");
const BOARD_ISOLATION_ONTOLOGY_LINKS_MIGRATION: &str =
    include_str!("../../../migrations/021_board_isolation_ontology_links.sql");
const TASK_SUBTASKS_EXECUTION_PLANS_MIGRATION: &str =
    include_str!("../../../migrations/022_task_subtasks_execution_plans.sql");
const TASK_STEPS_MIGRATION: &str = include_str!("../../../migrations/023_task_steps.sql");
const LATEST_MIGRATION_VERSION: i64 = 23;
const LEGACY_INITIAL_MIGRATION_CHECKSUMS: &[&str] = &[
    "fnv64:0ca871be950fc8a6",
    "fnv64:3b08da4e2b6041f5",
    "fnv64:61b5ea6d6ed1eabe",
];
const LEGACY_PRIORITY_LEVELS_MIGRATION_CHECKSUMS: &[&str] = &["fnv64:127ec944f1b716ff"];

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_initial",
        sql: INITIAL_MIGRATION,
    },
    Migration {
        version: 2,
        name: "002_knowledge_substrate",
        sql: KNOWLEDGE_SUBSTRATE_MIGRATION,
    },
    Migration {
        version: 3,
        name: "003_comment_author_identity",
        sql: COMMENT_AUTHOR_IDENTITY_MIGRATION,
    },
    Migration {
        version: 4,
        name: "004_priority_levels",
        sql: PRIORITY_LEVELS_MIGRATION,
    },
    Migration {
        version: 5,
        name: "005_decision_comment_kind",
        sql: DECISION_COMMENT_KIND_MIGRATION,
    },
    Migration {
        version: 6,
        name: "006_comment_metadata_contract",
        sql: COMMENT_METADATA_CONTRACT_MIGRATION,
    },
    Migration {
        version: 7,
        name: "007_label_semantics_atoms",
        sql: LABEL_SEMANTICS_ATOMS_MIGRATION,
    },
    Migration {
        version: 8,
        name: "008_label_atom_index_boards",
        sql: LABEL_ATOM_INDEX_BOARDS_MIGRATION,
    },
    Migration {
        version: 9,
        name: "009_label_semantic_proposals",
        sql: LABEL_SEMANTIC_PROPOSALS_MIGRATION,
    },
    Migration {
        version: 10,
        name: "010_stable_label_atom_hashes",
        sql: STABLE_LABEL_ATOM_HASHES_MIGRATION,
    },
    Migration {
        version: 11,
        name: "011_label_proposal_cosine_coverage",
        sql: LABEL_PROPOSAL_COSINE_COVERAGE_MIGRATION,
    },
    Migration {
        version: 12,
        name: "012_label_ontology_ledger",
        sql: LABEL_ONTOLOGY_LEDGER_MIGRATION,
    },
    Migration {
        version: 13,
        name: "013_label_ontology_suggest_input_hash",
        sql: LABEL_ONTOLOGY_SUGGEST_INPUT_HASH_MIGRATION,
    },
    Migration {
        version: 14,
        name: "014_unique_label_proposal_create_action",
        sql: UNIQUE_LABEL_PROPOSAL_CREATE_ACTION_MIGRATION,
    },
    Migration {
        version: 15,
        name: "015_adopt_existing_atom_action",
        sql: ADOPT_EXISTING_ATOM_ACTION_MIGRATION,
    },
    Migration {
        version: 16,
        name: "016_revert_ontology_mutation_action",
        sql: REVERT_ONTOLOGY_MUTATION_ACTION_MIGRATION,
    },
    Migration {
        version: 17,
        name: "017_board_isolation_composite_fk",
        sql: BOARD_ISOLATION_COMPOSITE_FK_MIGRATION,
    },
    Migration {
        version: 18,
        name: "018_label_ontology_root_action_effects",
        sql: LABEL_ONTOLOGY_ROOT_ACTION_EFFECTS_MIGRATION,
    },
    Migration {
        version: 19,
        name: "019_label_ontology_validation_requirement",
        sql: LABEL_ONTOLOGY_VALIDATION_REQUIREMENT_MIGRATION,
    },
    Migration {
        version: 20,
        name: "020_board_isolation_task_history",
        sql: BOARD_ISOLATION_TASK_HISTORY_MIGRATION,
    },
    Migration {
        version: 21,
        name: "021_board_isolation_ontology_links",
        sql: BOARD_ISOLATION_ONTOLOGY_LINKS_MIGRATION,
    },
    Migration {
        version: 22,
        name: "022_task_subtasks_execution_plans",
        sql: TASK_SUBTASKS_EXECUTION_PLANS_MIGRATION,
    },
    Migration {
        version: 23,
        name: "023_task_steps",
        sql: TASK_STEPS_MIGRATION,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitResult {
    pub db_path: PathBuf,
    pub board_id: String,
    pub board_slug: String,
}

pub fn init_database(path: impl AsRef<Path>, actor: &str) -> Result<InitResult> {
    let path = path.as_ref();
    let conn = connect_file(path)?;
    apply_migrations(&conn)?;
    ensure_default_board(&conn, actor, SystemClock.now_ms())?;
    let board_id = default_board_id(&conn)?;
    ensure_default_columns(&conn, &board_id, SystemClock.now_ms())?;
    ensure_knowledge_substrate(&conn)?;
    Ok(InitResult {
        db_path: path.to_path_buf(),
        board_id,
        board_slug: "default".to_owned(),
    })
}

fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(INITIAL_MIGRATION)
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    ensure_schema_migrations_shape(conn)?;
    for migration in MIGRATIONS {
        validate_or_apply_migration(conn, migration)?;
    }
    if crate::service::stable_label_atom_hash_backfill_needed(conn)? {
        crate::service::rebuild_label_atoms_for_stable_hash_migration(conn, SystemClock.now_ms())?;
    }
    validate_schema_shape(conn)?;
    conn.pragma_update(None, "user_version", LATEST_MIGRATION_VERSION)
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn validate_or_apply_migration(conn: &Connection, migration: &Migration) -> Result<bool> {
    let checksum = migration_checksum(migration.sql);
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
            [migration.version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    match row {
        Some((name, _stored)) if name != migration.name => Err(KanbanError::Storage(format!(
            "migration name mismatch for version {}: expected {}, found {name}",
            migration.version, migration.name
        ))),
        Some((_name, stored)) if stored.is_empty() => {
            conn.execute(
                "UPDATE schema_migrations SET checksum=?1 WHERE version=?2",
                params![checksum, migration.version],
            )
            .map_err(|err| KanbanError::Storage(err.to_string()))?;
            Ok(false)
        }
        Some((_name, stored)) if stored != checksum => {
            if is_allowed_legacy_migration_checksum(migration, &stored) {
                return Ok(false);
            }
            Err(KanbanError::Storage(format!(
                "migration checksum mismatch for {}: expected {checksum}, found {stored}",
                migration.name
            )))
        }
        Some((_name, _stored)) => Ok(false),
        None => {
            run_migration_preflight(conn, migration)?;
            conn.execute_batch(migration.sql)
                .map_err(|err| KanbanError::Storage(err.to_string()))?;
            conn.execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(version) DO UPDATE SET name=excluded.name, checksum=excluded.checksum",
                params![
                    migration.version,
                    migration.name,
                    checksum,
                    SystemClock.now_ms()
                ],
            )
            .map_err(|err| KanbanError::Storage(err.to_string()))?;
            Ok(true)
        }
    }
}

fn run_migration_preflight(conn: &Connection, migration: &Migration) -> Result<()> {
    if migration.version == 17 {
        ensure_no_cross_board_rows_for_composite_fk_migration(conn)?;
    }
    if migration.version == 20 {
        ensure_no_cross_board_rows_for_task_history_migration(conn)?;
    }
    if migration.version == 21 {
        ensure_no_cross_board_rows_for_ontology_links_migration(conn)?;
    }
    Ok(())
}

fn ensure_no_cross_board_rows_for_composite_fk_migration(conn: &Connection) -> Result<()> {
    for check in [
        BoardIsolationPreflight {
            table: "task_labels",
            sql: "SELECT tl.task_id || ':' || tl.label_id, tl.board_id, t.board_id, l.board_id \
                  FROM task_labels tl \
                  JOIN tasks t ON t.id = tl.task_id \
                  JOIN labels l ON l.id = tl.label_id \
                  WHERE tl.board_id != t.board_id OR tl.board_id != l.board_id \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "task_dependencies",
            sql: "SELECT d.parent_task_id || '->' || d.child_task_id, d.board_id, p.board_id, c.board_id \
                  FROM task_dependencies d \
                  JOIN tasks p ON p.id = d.parent_task_id \
                  JOIN tasks c ON c.id = d.child_task_id \
                  WHERE d.board_id != p.board_id OR d.board_id != c.board_id \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "task_runs",
            sql: "SELECT r.id, r.board_id, t.board_id, NULL \
                  FROM task_runs r \
                  JOIN tasks t ON t.id = r.task_id \
                  WHERE r.board_id != t.board_id \
                  LIMIT 1",
        },
    ] {
        if let Some(mismatch) = first_board_isolation_mismatch(conn, &check)? {
            return Err(KanbanError::Storage(format!(
                "cannot apply migration 017_board_isolation_composite_fk: {} cross-board row {} has row board {}, referenced boards {}; run kanban doctor and repair before migrating",
                check.table, mismatch.row_key, mismatch.row_board, mismatch.referenced_boards
            )));
        }
    }
    Ok(())
}

fn ensure_no_cross_board_rows_for_task_history_migration(conn: &Connection) -> Result<()> {
    for check in [
        BoardIsolationPreflight {
            table: "task_comments",
            sql: "SELECT c.id, c.board_id, COALESCE(t.board_id, 'missing task ' || c.task_id), NULL \
                  FROM task_comments c \
                  LEFT JOIN tasks t ON t.id = c.task_id \
                  WHERE t.id IS NULL OR c.board_id != t.board_id \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "task_events",
            sql: "SELECT e.event_id, e.board_id, COALESCE(t.board_id, 'missing task ' || e.task_id), NULL \
                  FROM task_events e \
                  LEFT JOIN tasks t ON t.id = e.task_id \
                  WHERE e.task_id IS NOT NULL AND (t.id IS NULL OR e.board_id != t.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "task_events",
            sql: "SELECT e.event_id, e.board_id, COALESCE(r.board_id, 'missing run ' || e.run_id), NULL \
                  FROM task_events e \
                  LEFT JOIN task_runs r ON r.id = e.run_id \
                  WHERE e.run_id IS NOT NULL AND (r.id IS NULL OR e.board_id != r.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "task_attachments",
            sql: "SELECT a.id, a.board_id, COALESCE(t.board_id, 'missing task ' || a.task_id), NULL \
                  FROM task_attachments a \
                  LEFT JOIN tasks t ON t.id = a.task_id \
                  WHERE t.id IS NULL OR a.board_id != t.board_id \
                  LIMIT 1",
        },
    ] {
        if let Some(mismatch) = first_board_isolation_mismatch(conn, &check)? {
            return Err(KanbanError::Storage(format!(
                "cannot apply migration 020_board_isolation_task_history: {} cross-board row {} has row board {}, referenced boards {}; run kanban doctor and repair before migrating",
                check.table, mismatch.row_key, mismatch.row_board, mismatch.referenced_boards
            )));
        }
    }
    Ok(())
}

fn ensure_no_cross_board_rows_for_ontology_links_migration(conn: &Connection) -> Result<()> {
    for check in [
        BoardIsolationPreflight {
            table: "label_semantic_proposals",
            sql: "SELECT p.id, p.board_id, COALESCE(t.board_id, 'missing task ' || p.task_id), NULL \
                  FROM label_semantic_proposals p \
                  LEFT JOIN tasks t ON t.id = p.task_id \
                  WHERE t.id IS NULL OR p.board_id != t.board_id \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_semantic_proposals",
            sql: "SELECT p.id, p.board_id, COALESCE(l.board_id, 'missing resolved label ' || p.resolved_label_id), NULL \
                  FROM label_semantic_proposals p \
                  LEFT JOIN labels l ON l.id = p.resolved_label_id \
                  WHERE p.resolved_label_id IS NOT NULL AND (l.id IS NULL OR p.board_id != l.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_signals",
            sql: "SELECT s.id, s.board_id, COALESCE(o.board_id, 'missing observation ' || s.observation_id), NULL \
                  FROM label_ontology_signals s \
                  LEFT JOIN label_ontology_observations o ON o.id = s.observation_id \
                  WHERE o.id IS NULL OR s.board_id != o.board_id \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_signals",
            sql: "SELECT s.id, s.board_id, COALESCE(l.board_id, 'missing target label ' || s.target_label_id), NULL \
                  FROM label_ontology_signals s \
                  LEFT JOIN labels l ON l.id = s.target_label_id \
                  WHERE s.target_label_id IS NOT NULL AND (l.id IS NULL OR s.board_id != l.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_signals",
            sql: "SELECT s.id, s.board_id, COALESCE(r.board_id, 'missing superseding signal ' || s.superseded_by_signal_id), NULL \
                  FROM label_ontology_signals s \
                  LEFT JOIN label_ontology_signals r ON r.id = s.superseded_by_signal_id \
                  WHERE s.superseded_by_signal_id IS NOT NULL AND (r.id IS NULL OR s.board_id != r.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_actions",
            sql: "SELECT a.id, a.board_id, COALESCE(p.board_id, 'missing parent action ' || a.parent_action_id), NULL \
                  FROM label_ontology_actions a \
                  LEFT JOIN label_ontology_actions p ON p.id = a.parent_action_id \
                  WHERE a.parent_action_id IS NOT NULL AND (p.id IS NULL OR a.board_id != p.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_actions",
            sql: "SELECT a.id, a.board_id, COALESCE(l.board_id, 'missing target label ' || a.target_label_id), NULL \
                  FROM label_ontology_actions a \
                  LEFT JOIN labels l ON l.id = a.target_label_id \
                  WHERE a.target_label_id IS NOT NULL AND (l.id IS NULL OR a.board_id != l.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_actions",
            sql: "SELECT a.id, a.board_id, COALESCE(l.board_id, 'missing result label ' || a.result_label_id), NULL \
                  FROM label_ontology_actions a \
                  LEFT JOIN labels l ON l.id = a.result_label_id \
                  WHERE a.result_label_id IS NOT NULL AND (l.id IS NULL OR a.board_id != l.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_actions",
            sql: "SELECT a.id, a.board_id, COALESCE(p.board_id, 'missing result proposal ' || a.result_proposal_id), NULL \
                  FROM label_ontology_actions a \
                  LEFT JOIN label_semantic_proposals p ON p.id = a.result_proposal_id \
                  WHERE a.result_proposal_id IS NOT NULL AND (p.id IS NULL OR a.board_id != p.board_id) \
                  LIMIT 1",
        },
        BoardIsolationPreflight {
            table: "label_ontology_action_signals",
            sql: "SELECT x.action_id || ':' || x.signal_id, x.board_id, \
                         COALESCE(a.board_id, 'missing action ' || x.action_id), \
                         COALESCE(s.board_id, 'missing signal ' || x.signal_id) \
                  FROM label_ontology_action_signals x \
                  LEFT JOIN label_ontology_actions a ON a.id = x.action_id \
                  LEFT JOIN label_ontology_signals s ON s.id = x.signal_id \
                  WHERE a.id IS NULL OR s.id IS NULL OR x.board_id != a.board_id OR x.board_id != s.board_id \
                  LIMIT 1",
        },
    ] {
        if let Some(mismatch) = first_board_isolation_mismatch(conn, &check)? {
            return Err(KanbanError::Storage(format!(
                "cannot apply migration 021_board_isolation_ontology_links: {} cross-board or orphan row {} has row board {}, referenced boards {}; run kanban doctor and repair before migrating",
                check.table, mismatch.row_key, mismatch.row_board, mismatch.referenced_boards
            )));
        }
    }
    Ok(())
}

struct BoardIsolationPreflight {
    table: &'static str,
    sql: &'static str,
}

struct BoardIsolationMismatch {
    row_key: String,
    row_board: String,
    referenced_boards: String,
}

fn first_board_isolation_mismatch(
    conn: &Connection,
    check: &BoardIsolationPreflight,
) -> Result<Option<BoardIsolationMismatch>> {
    conn.query_row(check.sql, [], |row| {
        let row_key: String = row.get(0)?;
        let row_board: String = row.get(1)?;
        let first_ref_board: String = row.get(2)?;
        let second_ref_board: Option<String> = row.get(3)?;
        let referenced_boards = match second_ref_board {
            Some(second_ref_board) => format!("{first_ref_board}, {second_ref_board}"),
            None => first_ref_board,
        };
        Ok(BoardIsolationMismatch {
            row_key,
            row_board,
            referenced_boards,
        })
    })
    .optional()
    .map_err(|err| KanbanError::Storage(err.to_string()))
}

fn is_allowed_legacy_migration_checksum(migration: &Migration, stored: &str) -> bool {
    match migration.version {
        1 => LEGACY_INITIAL_MIGRATION_CHECKSUMS.contains(&stored),
        4 => LEGACY_PRIORITY_LEVELS_MIGRATION_CHECKSUMS.contains(&stored),
        _ => false,
    }
}

fn ensure_schema_migrations_shape(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "schema_migrations", "checksum")? {
        conn.execute(
            "ALTER TABLE schema_migrations ADD COLUMN checksum TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}

fn validate_schema_shape(conn: &Connection) -> Result<()> {
    let required = [
        (
            "boards",
            &["id", "slug", "name", "created_at", "updated_at"][..],
        ),
        (
            "tasks",
            &[
                "id",
                "board_id",
                "seq",
                "title",
                "description",
                "status",
                "claim_token",
                "claim_expires_at",
                "current_run_id",
                "lock_version",
            ][..],
        ),
        (
            "task_dependencies",
            &["board_id", "parent_task_id", "child_task_id"][..],
        ),
        (
            "task_steps",
            &[
                "id",
                "board_id",
                "parent_task_id",
                "position",
                "title",
                "linked_task_id",
                "required",
                "status",
                "created_by",
                "created_at",
                "updated_by",
                "updated_at",
            ][..],
        ),
        (
            "task_execution_plans",
            &[
                "board_id",
                "task_id",
                "state",
                "reason",
                "updated_by",
                "updated_at",
            ][..],
        ),
        (
            "task_runs",
            &["id", "board_id", "task_id", "status", "claim_token"][..],
        ),
        (
            "task_events",
            &[
                "id",
                "event_id",
                "board_id",
                "task_id",
                "kind",
                "payload_json",
            ][..],
        ),
        (
            "task_comments",
            &[
                "id",
                "board_id",
                "task_id",
                "author",
                "author_type",
                "agent_type",
                "body",
                "kind",
                "metadata_json",
                "created_at",
            ][..],
        ),
        (
            "entities",
            &[
                "uri",
                "kind",
                "source_table",
                "source_id",
                "created_at",
                "updated_at",
            ][..],
        ),
        (
            "relation_predicates",
            &["name", "authoritative_store", "created_at"][..],
        ),
        (
            "entity_relations",
            &[
                "subject_uri",
                "predicate",
                "object_uri",
                "authoritative_store",
            ][..],
        ),
        (
            "index_outbox",
            &["target", "entity_uri", "action", "status", "attempts"][..],
        ),
        (
            "derived_store_state",
            &["store_name", "schema_version", "last_event_id", "dirty"][..],
        ),
        (
            "label_semantics",
            &[
                "label_id",
                "board_id",
                "description",
                "applies_when",
                "excludes_when",
                "positive_examples",
                "negative_examples",
            ][..],
        ),
        (
            "label_atoms",
            &[
                "id",
                "label_id",
                "board_id",
                "polarity",
                "kind",
                "text",
                "ordinal",
                "content_hash",
            ][..],
        ),
        (
            "label_atom_index_boards",
            &["store_name", "board_id", "dirty", "updated_at"][..],
        ),
        (
            "label_semantic_proposals",
            &[
                "id",
                "board_id",
                "task_id",
                "status",
                "name",
                "diagnostics_json",
                "resolved_label_id",
            ][..],
        ),
        (
            "label_ontology_observations",
            &[
                "id",
                "board_id",
                "task_id",
                "task_ref_snapshot",
                "task_snapshot_json",
                "suggest_input_hash",
                "agent_candidates_json",
                "suggestion_snapshot_json",
                "final_decision_json",
                "capture_fingerprint",
            ][..],
        ),
        (
            "label_ontology_signals",
            &[
                "id",
                "observation_id",
                "board_id",
                "kind",
                "status",
                "proposed_action",
                "signal_key",
            ][..],
        ),
        (
            "label_ontology_actions",
            &[
                "id",
                "board_id",
                "action_type",
                "reason",
                "validation_status",
                "change_json",
                "validation_json",
            ][..],
        ),
        (
            "label_ontology_action_signals",
            &["board_id", "action_id", "signal_id", "created_at"][..],
        ),
        (
            "label_ontology_action_atom_effects",
            &[
                "board_id",
                "action_id",
                "label_id_snapshot",
                "atom_id_snapshot",
                "atom_content_hash",
                "polarity",
                "kind",
                "text",
                "effect",
                "created_at",
            ][..],
        ),
    ];
    for (table, columns) in required {
        for column in columns {
            if !table_has_column(conn, table, column)? {
                return Err(KanbanError::Storage(format!(
                    "schema validation failed: missing column {table}.{column}"
                )));
            }
        }
    }
    Ok(())
}

fn ensure_knowledge_substrate(conn: &Connection) -> Result<()> {
    let now = SystemClock.now_ms();
    seed_relation_predicates(conn, now)?;
    seed_derived_store_state(conn, now)?;
    backfill_entities(conn)?;
    backfill_dependency_relations(conn, now)?;
    Ok(())
}

fn seed_relation_predicates(conn: &Connection, now: i64) -> Result<()> {
    for predicate in PREDICATE_SEEDS {
        let seed = predicate.seed();
        conn.execute(
            "INSERT INTO relation_predicates(name, domain_kind, range_kind, cardinality, authoritative_store, description, created_at) \
             VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5) \
             ON CONFLICT(name) DO UPDATE SET domain_kind=excluded.domain_kind, range_kind=excluded.range_kind, authoritative_store=excluded.authoritative_store",
            params![
                seed.name,
                seed.domain_kind,
                seed.range_kind,
                seed.authoritative_store,
                now
            ],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}

fn seed_derived_store_state(conn: &Connection, now: i64) -> Result<()> {
    for seed in DERIVED_STORE_SEEDS {
        conn.execute(
            "INSERT OR IGNORE INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
             VALUES (?1, ?2, 0, 0, NULL, NULL, NULL, ?3)",
            params![seed.store_name, seed.schema_version, now],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}

fn backfill_entities(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://board/' || id, 'board', 'boards', id, id, NULL, name, description, NULL, created_at, updated_at, archived_at FROM boards WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://column/' || id, 'column', 'board_columns', id, board_id, NULL, title, status, NULL, created_at, updated_at, NULL FROM board_columns WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://task/' || id, 'task', 'tasks', id, board_id, id, title, description, NULL, created_at, updated_at, archived_at FROM tasks WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://run/' || id, 'run', 'task_runs', id, board_id, task_id, id, COALESCE(summary, error), NULL, started_at, COALESCE(finished_at, last_heartbeat_at, started_at), NULL FROM task_runs WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://event/' || event_id, 'event', 'task_events', event_id, board_id, task_id, kind, payload_json, NULL, created_at, created_at, NULL FROM task_events WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://comment/' || id, 'comment', 'task_comments', id, board_id, task_id, author, body, NULL, created_at, created_at, NULL FROM task_comments WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://artifact/' || id, 'attachment', 'task_attachments', id, board_id, task_id, filename, rel_path, sha256, created_at, created_at, NULL FROM task_attachments WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://label/' || id, 'label', 'labels', id, board_id, NULL, name, color, NULL, created_at, updated_at, NULL FROM labels WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://task-label/' || task_id || '/' || label_id, 'task_label', 'task_labels', task_id || ':' || label_id, board_id, task_id, label_id, NULL, NULL, created_at, created_at, NULL FROM task_labels WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://setting/' || key, 'setting', 'app_settings', key, NULL, NULL, key, value_json, NULL, updated_at, updated_at, NULL FROM app_settings WHERE true \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn backfill_dependency_relations(conn: &Connection, now: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || id, 'belongs_to_board', 'kb://board/' || board_id, 'kb://graph/indexed', 'sqlite', 'tasks', id, NULL, '{}', created_at, ?1 FROM tasks",
        [now],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || child_task_id, 'depends_on', 'kb://task/' || parent_task_id, 'kb://graph/indexed', 'sqlite', 'task_dependencies', parent_task_id || '->' || child_task_id, NULL, '{}', created_at, ?1 FROM task_dependencies",
        [now],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| KanbanError::Storage(err.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(columns.iter().any(|name| name == column))
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}

fn ensure_default_board(conn: &Connection, actor: &str, now_ms: i64) -> Result<()> {
    let existing: Option<String> = conn
        .query_row("SELECT id FROM boards WHERE slug = 'default'", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    if existing.is_some() {
        return Ok(());
    }

    let board_id = new_board_id();
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES (?1, 'default', 'Default', NULL, ?2, ?2, NULL)",
        params![board_id, now_ms],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, NULL, NULL, 'board.created', ?3, '{}', ?4)",
        params![kanban_core::new_event_id(), board_id, actor, now_ms],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn default_board_id(conn: &Connection) -> Result<String> {
    conn.query_row("SELECT id FROM boards WHERE slug = 'default'", [], |row| {
        row.get(0)
    })
    .map_err(|err| KanbanError::Storage(err.to_string()))
}

fn ensure_default_columns(conn: &Connection, board_id: &str, now_ms: i64) -> Result<()> {
    let defaults = [
        ("triage", "Triage", 10, 0),
        ("todo", "Todo", 20, 0),
        ("scheduled", "Scheduled", 30, 0),
        ("ready", "Ready", 40, 0),
        ("running", "Running", 50, 0),
        ("blocked", "Blocked", 60, 0),
        ("review", "Review", 70, 0),
        ("done", "Done", 80, 0),
        ("archived", "Archived", 90, 1),
    ];
    for (status, title, position, hidden) in defaults {
        let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
        conn.execute(
            "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
            params![id, board_id, status, title, position, hidden, now_ms],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}
