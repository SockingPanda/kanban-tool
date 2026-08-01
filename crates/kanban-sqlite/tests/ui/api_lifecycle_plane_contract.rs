#![allow(unused_imports)]

use std::path::Path;

use kanban_sqlite::api::lifecycle::{
    DatabaseReplaceGuard, DatabaseReplaceOptions, DatabaseReplaceReport, DatabaseRuntimeGuard,
    begin_database_replace, begin_database_runtime, publish_staged_database,
    publish_staged_database_with_options, resume_staged_database_replace,
    resume_staged_database_replace_with_options,
};

fn main() {}

fn lifecycle_methods_are_stable() {
    let _ = |path: &Path| begin_database_replace(path);
    let _ = |path: &Path| begin_database_runtime(path);
    let _: Option<DatabaseRuntimeGuard> = None;
    let _ = DatabaseReplaceGuard::validate_database_identities;
    let _ = DatabaseReplaceGuard::fence_staged_database_for_replace;
    let _ = DatabaseReplaceGuard::rebind_after_namespace_publish;
    let _ = DatabaseReplaceOptions::default;
    let _: Option<DatabaseReplaceReport> = None;
    let _ = |guard: &mut DatabaseReplaceGuard, path: &Path| {
        publish_staged_database(guard, path, path, path, path)
    };
    let _ = |guard: &mut DatabaseReplaceGuard, path: &Path, options: DatabaseReplaceOptions| {
        publish_staged_database_with_options(guard, path, path, path, path, options)
    };
    let _ =
        |guard: &mut DatabaseReplaceGuard, path: &Path| resume_staged_database_replace(guard, path);
    let _ = |guard: &mut DatabaseReplaceGuard, path: &Path, options: DatabaseReplaceOptions| {
        resume_staged_database_replace_with_options(guard, path, options)
    };
}
