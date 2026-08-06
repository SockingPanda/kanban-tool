use serde::Serialize;

use crate::{ContractSurface, MigrationState, endpoint_catalog};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SurfaceOperation {
    pub key: String,
    pub surface: ContractSurface,
    pub contracts: Vec<&'static str>,
    pub migration: MigrationState,
    pub exclusion: Option<&'static str>,
}

macro_rules! excluded {
    ($surface:ident, $key:literal, $reason:literal) => {
        SurfaceOperation {
            key: $key.to_owned(),
            surface: ContractSurface::$surface,
            contracts: vec![],
            migration: MigrationState::Excluded,
            exclusion: Some($reason),
        }
    };
}

macro_rules! adopted {
    ($surface:ident, $key:literal, $contract:literal) => {
        SurfaceOperation {
            key: $key.to_owned(),
            surface: ContractSurface::$surface,
            contracts: vec![$contract],
            migration: MigrationState::Adopted,
            exclusion: None,
        }
    };
}

fn non_transport_operations() -> Vec<SurfaceOperation> {
    let mut operations = vec![
        excluded!(
            Cli,
            "__complete",
            "隐藏的动态补全候选使用逐行文本协议，不是 JSON document"
        ),
        adopted!(Cli, "backup", "cli.maintenance-backup.output"),
        crate::board_catalog::surface_operation("board archive"),
        crate::board_catalog::surface_operation("board columns"),
        crate::board_catalog::surface_operation("board create"),
        adopted!(Cli, "board current", "cli.board-current.output"),
        crate::board_catalog::surface_operation("board list"),
        crate::board_catalog::surface_operation("board show"),
        adopted!(Cli, "board use", "cli.board-use.output"),
        adopted!(Cli, "checkpoint", "cli.checkpoint.output"),
        adopted!(Cli, "comment add", "cli.comment-add.output"),
        adopted!(Cli, "comment list", "cli.comment-list.output"),
        adopted!(Cli, "attachment add", "cli.attachment-add.output"),
        adopted!(Cli, "attachment list", "cli.attachment-list.output"),
        adopted!(Cli, "attachment remove", "cli.attachment-remove.output"),
        excluded!(
            Cli,
            "attachment download",
            "附件下载输出是原始 bytes 文件，不是 JSON machine envelope"
        ),
        excluded!(
            Cli,
            "completions",
            "shell completion script 是文本脚本，不是 JSON document"
        ),
        adopted!(Cli, "config show", "cli.config-show.output"),
        adopted!(Cli, "context build", "cli.context-build.output"),
        adopted!(Cli, "dep add", "cli.dep-add.output"),
        adopted!(Cli, "dep list", "cli.dep-list.output"),
        adopted!(Cli, "dep remove", "cli.dep-remove.output"),
        adopted!(Cli, "doctor", "cli.doctor.output"),
        adopted!(Cli, "entity list", "cli.entity-list.output"),
        adopted!(Cli, "entity show", "cli.entity-show.output"),
        adopted!(Cli, "entity upsert", "cli.entity-upsert.output"),
        adopted!(Cli, "events", "cli.events.output"),
        adopted!(Cli, "export", "cli.maintenance-export.output"),
        adopted!(Cli, "graph neighbors", "cli.graph-neighbors.output"),
        adopted!(Cli, "graph query", "cli.graph-query.output"),
        adopted!(Cli, "graph neighborhood", "cli.graph-neighborhood.output"),
        adopted!(Cli, "graph map", "cli.graph-map.output"),
        adopted!(Cli, "graph rebuild", "cli.graph-rebuild.output"),
        adopted!(Cli, "graph status", "cli.graph-status.output"),
        adopted!(Cli, "graph sync", "cli.graph-sync.output"),
        excluded!(
            Cli,
            "hook codex handle failure",
            "Codex hook handler 使用独立 stdin/stdout protocol，不走通用 CLI JSON envelope"
        ),
        excluded!(
            Cli,
            "hook codex handle task-create",
            "Codex hook handler 使用独立 stdin/stdout protocol，不走通用 CLI JSON envelope"
        ),
        adopted!(Cli, "hook codex install", "cli.hook-codex-install.output"),
        adopted!(Cli, "hook codex status", "cli.hook-codex-status.output"),
        adopted!(
            Cli,
            "hook codex uninstall",
            "cli.hook-codex-uninstall.output"
        ),
        adopted!(Cli, "import", "cli.maintenance-import.output"),
        adopted!(Cli, "index doctor", "cli.index-doctor.output"),
        adopted!(Cli, "index rebuild", "cli.index-rebuild.output"),
        adopted!(Cli, "index status", "cli.index-status.output"),
        adopted!(Cli, "index sync", "cli.index-sync.output"),
        adopted!(Cli, "init", "cli.init.output"),
        adopted!(
            Cli,
            "maintenance rebuild",
            "cli.maintenance-rebuild-v1.output"
        ),
        adopted!(Cli, "maintenance run", "cli.maintenance-run-v1.output"),
        adopted!(
            Cli,
            "maintenance status",
            "cli.maintenance-status-v1.output"
        ),
        adopted!(Cli, "stats", "cli.stats.output"),
        adopted!(Cli, "task archive", "cli.task-archive.output"),
        adopted!(Cli, "task block", "cli.task-block.output"),
        adopted!(Cli, "task claim", "cli.task-claim.output"),
        adopted!(Cli, "task create", "cli.task-create.output"),
        adopted!(Cli, "task done", "cli.task-done.output"),
        adopted!(Cli, "task heartbeat", "cli.task-heartbeat.output"),
        adopted!(Cli, "task release", "cli.task-release.output"),
        adopted!(Cli, "task list", "cli.task-list.output"),
        adopted!(Cli, "task promote", "cli.task-promote.output"),
        adopted!(Cli, "task reclaim", "cli.task-reclaim.output"),
        adopted!(Cli, "task reopen", "cli.task-reopen.output"),
        adopted!(Cli, "task review", "cli.task-review.output"),
        adopted!(Cli, "task show", "cli.task-show.output"),
        adopted!(Cli, "task specify", "cli.task-specify.output"),
        adopted!(Cli, "task step add", "cli.task-step-add.output"),
        adopted!(Cli, "task step done", "cli.task-step-done.output"),
        adopted!(Cli, "task step list", "cli.task-step-list.output"),
        adopted!(
            Cli,
            "task step not-required",
            "cli.task-step-not-required.output"
        ),
        adopted!(Cli, "task step remove", "cli.task-step-remove.output"),
        adopted!(Cli, "task step reopen", "cli.task-step-reopen.output"),
        adopted!(Cli, "task step skip", "cli.task-step-skip.output"),
        adopted!(Cli, "task step update", "cli.task-step-update.output"),
        adopted!(Cli, "task unblock", "cli.task-unblock.output"),
        adopted!(Cli, "task update", "cli.task-update.output"),
        adopted!(Cli, "vacuum", "cli.maintenance-vacuum.output"),
        adopted!(Cli, "vector configure", "cli.vector-configure.output"),
        adopted!(Cli, "vector query-chunks", "cli.vector-query-chunks.output"),
        adopted!(
            Cli,
            "vector query-label-atoms",
            "cli.vector-query-label-atoms.output"
        ),
        adopted!(Cli, "vector rebuild", "cli.vector-rebuild.output"),
        adopted!(Cli, "vector status", "cli.vector-status.output"),
        adopted!(Cli, "vector sync", "cli.vector-sync.output"),
        adopted!(Cli, "maintenance cleanup", "cli.maintenance-cleanup.output"),
        adopted!(Cli, "import-v30", "cli.import-v30.output"),
        adopted!(
            Metadata,
            "structured decision comment metadata input",
            "metadata.decision.input"
        ),
        adopted!(
            Metadata,
            "generic signal record input",
            "metadata.signal-record.input"
        ),
        adopted!(
            Metadata,
            "signal backlink comment metadata output",
            "metadata.signal-link.output"
        ),
        adopted!(
            Metadata,
            "label proposal candidate input",
            "metadata.label-proposal-candidate.input"
        ),
        adopted!(
            Metadata,
            "label ontology observation input",
            "metadata.ontology-record.input"
        ),
        adopted!(
            Metadata,
            "label ontology external validation evidence",
            "metadata.ontology-validation-evidence.input"
        ),
        adopted!(
            Api,
            "POST /api/v1/maintenance/{operation}",
            "api.maintenance-path.request"
        ),
        adopted!(
            Config,
            "project-local config after TOML decoding",
            "config.project.input"
        ),
        adopted!(
            Config,
            "selected dispatcher worker profile after TOML decoding",
            "config.selected-worker-profile.input"
        ),
    ];
    let insertion = operations
        .iter()
        .position(|operation| operation.key == "stats")
        .expect("legacy CLI surface must retain stats insertion anchor");
    operations.splice(
        insertion..insertion,
        crate::cli_labels_catalog::surface_catalog(),
    );
    operations.extend(crate::portable::surface_catalog());
    // Metadata/Config rows remain in the legacy vector for compatibility, but their declaration
    // source owns the projected surface facts. shared API error is intentionally not a surface
    // parent and therefore does not add a synthetic operation here.
    let source = crate::metadata_config_catalog::surface_catalog();
    for operation in &mut operations {
        if let Some(declaration) = source.iter().find(|candidate| {
            candidate.surface == operation.surface && candidate.key == operation.key
        }) {
            *operation = declaration.clone();
        }
    }
    operations
}

pub fn surface_operation_catalog() -> Vec<SurfaceOperation> {
    let mut operations = endpoint_catalog()
        .iter()
        .map(|endpoint| SurfaceOperation {
            key: String::new(),
            // 下方会对 debug 形式做规范化，以保留历史 catalog key。
            surface: endpoint.surface,
            contracts: endpoint_contract_references(endpoint),
            migration: endpoint.migration,
            exclusion: endpoint.exclusion,
        })
        .collect::<Vec<_>>();
    for (operation, endpoint) in operations.iter_mut().zip(endpoint_catalog()) {
        operation.key = format!(
            "{} {}",
            endpoint_method_name(endpoint.method),
            endpoint.path
        );
    }
    operations.extend(non_transport_operations());
    operations
}

fn endpoint_contract_references(endpoint: &crate::EndpointDescriptor) -> Vec<&'static str> {
    let mut contracts = [
        endpoint.obligations.path,
        endpoint.obligations.query,
        endpoint.obligations.headers,
        endpoint.obligations.body,
        endpoint.obligations.success,
        endpoint.obligations.sse,
    ]
    .into_iter()
    .filter_map(|obligation| match obligation {
        crate::EndpointObligation::Contract(id) => Some(id),
        _ => None,
    })
    .collect::<Vec<_>>();
    contracts.extend_from_slice(endpoint.shared_components);
    contracts
}

fn endpoint_method_name(method: crate::HttpMethod) -> &'static str {
    match method {
        crate::HttpMethod::Get => "GET",
        crate::HttpMethod::Post => "POST",
        crate::HttpMethod::Put => "PUT",
        crate::HttpMethod::Patch => "PATCH",
        crate::HttpMethod::Delete => "DELETE",
    }
}

pub fn surface_operation_keys(surface: ContractSurface) -> impl Iterator<Item = String> {
    surface_operation_catalog()
        .into_iter()
        .filter(move |operation| operation.surface == surface)
        .map(|operation| operation.key)
}
