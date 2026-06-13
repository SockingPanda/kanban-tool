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
const LATEST_MIGRATION_VERSION: i64 = 4;
const LEGACY_INITIAL_MIGRATION_CHECKSUMS: &[&str] =
    &["fnv64:0ca871be950fc8a6", "fnv64:3b08da4e2b6041f5"];

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
    validate_schema_shape(conn)?;
    conn.pragma_update(None, "user_version", LATEST_MIGRATION_VERSION)
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn validate_or_apply_migration(conn: &Connection, migration: &Migration) -> Result<()> {
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
        Some((name, _stored)) if name != migration.name => {
            return Err(KanbanError::Storage(format!(
                "migration name mismatch for version {}: expected {}, found {name}",
                migration.version, migration.name
            )));
        }
        Some((_name, stored)) if stored.is_empty() => {
            conn.execute(
                "UPDATE schema_migrations SET checksum=?1 WHERE version=?2",
                params![checksum, migration.version],
            )
            .map_err(|err| KanbanError::Storage(err.to_string()))?;
        }
        Some((_name, stored)) if stored != checksum => {
            if is_allowed_legacy_migration_checksum(migration, &stored) {
                return Ok(());
            }
            return Err(KanbanError::Storage(format!(
                "migration checksum mismatch for {}: expected {checksum}, found {stored}",
                migration.name
            )));
        }
        Some((_name, _stored)) => {}
        None => {
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
        }
    }
    Ok(())
}

fn is_allowed_legacy_migration_checksum(migration: &Migration, stored: &str) -> bool {
    migration.version == 1 && LEGACY_INITIAL_MIGRATION_CHECKSUMS.contains(&stored)
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
