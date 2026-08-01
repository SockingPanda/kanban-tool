//! Crash-safe publication of a fully closed staged SQLite database.
//!
//! The lifecycle guard owns the database and derived-store authorities. This
//! module owns the namespace transition and its durable journal. All four
//! paths are normalized, sibling, distinct paths; no caller may replace the
//! canonical file with an ad-hoc rename/remove sequence.

use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use kanban_core::{KanbanError, Result};
use kanban_local::{
    DatabaseFileIdentity, database_file_identity, database_file_identity_from_file,
    durable_create_new_file, durable_move_file_no_replace, durable_replace_file_contents,
    durable_sync_directory, open_database_file_for_identity,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::DatabaseReplaceGuard;

include!("database_replace_support.rs");
include!("database_replace_flow.rs");
include!("database_replace_validation.rs");

#[cfg(test)]
mod tests {
    include!("database_replace_tests.rs");
    include!("database_replace_tests_recovery.rs");
}
