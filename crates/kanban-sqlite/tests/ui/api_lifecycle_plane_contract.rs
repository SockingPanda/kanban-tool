#![allow(unused_imports)]

use kanban_sqlite::api::lifecycle::{
    DatabaseReplaceGuard, DatabaseRuntimeGuard, begin_database_replace, begin_database_runtime,
};

fn main() {}

fn lifecycle_methods_are_stable() {
    let _ = DatabaseReplaceGuard::validate_database_identities;
    let _ = DatabaseReplaceGuard::fence_staged_database_for_replace;
    let _ = DatabaseReplaceGuard::rebind_after_namespace_publish;
}
