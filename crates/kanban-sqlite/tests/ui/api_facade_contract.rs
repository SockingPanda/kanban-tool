use std::path::{Path, PathBuf};

use kanban_sqlite::api;

fn adapter_contract_path_compiles(path: &Path, task_id: &str, label_id: &str) {
    let create = api::CreateTask::ready("facade contract");
    let _ = api::create_task(path, "default", "tester", create);
    let _ = api::mark_execution_plan_not_required(path, "default", "tester", task_id, "contract");
    let _ = api::get_task(path, "default", task_id);
    let _ = api::get_task_by_id_global(path, task_id);
    let _ = api::list_runs(path, "default", Some(task_id));
    let _ = api::list_events_after(
        path,
        "default",
        api::EventListOptions {
            task_ref: Some(task_id.to_owned()),
            after: 0,
            limit: 10,
        },
    );
    let _ = api::get_label_semantics_by_id(path, "default", label_id);
    let _ = api::get_label_ontology_signal(path, "signal_id");

    let _dispatch = api::DispatchOptions {
        actor: "dispatcher".to_owned(),
        command: "true".to_owned(),
        worker_profile: "contract".to_owned(),
        claim_ttl_ms: 60_000,
        heartbeat_interval_ms: 30_000,
        on_success: api::FinishPolicy::Done,
        on_failure: api::FinishPolicy::Blocked,
        log_dir: PathBuf::from("logs"),
    };
}

fn assert_record_types(
    _task: api::TaskRecord,
    _label: api::LabelRecord,
    _run: api::RunRecord,
    _event: api::EventRecord,
    _signal: api::LabelOntologySignalDetail,
) {
}

fn main() {}
