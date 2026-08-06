//! CLI shell、queue、knowledge 与 host-admin 命令的唯一 declaration source。
//!
//! 本模块覆盖 `surface.rs::non_transport_operations` 中从 `__complete` 到
//! `maintenance status` 的连续区段，但刻意不重复 `board_catalog` 已声明的五个
//! board CLI parent。声明保留既有 CLI key、output contract、schema artifact、fixture、
//! adoption witness 与 exclusion；真实 Clap command 和输出序列化仍由 `kanban-cli` 拥有。

use crate::{
    AdoptionLocator, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, MigrationState, OperationContract, OperationDeclaration,
    SurfaceOperation,
};

const CLI_QUEUE_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_queue_adoption",
    exact_test: "queue_cli_uses_real_host_for_config_board_and_task_commands",
};

const CLI_HISTORY_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_history_adoption",
    exact_test: "history_cli_covers_runs_logs_comments_attachments_events_and_stats",
};

const CLI_STEPS_DEPENDENCIES_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_steps_dependencies_adoption",
    exact_test: "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
};

const CLI_KNOWLEDGE_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_knowledge_adoption",
    exact_test: "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
};

const CLI_ADMIN_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_admin_adoption",
    exact_test: "maintenance_admin_commands_use_real_host_and_typed_json",
};

const CLI_HOOK_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_admin_adoption",
    exact_test: "codex_hooks_install_handle_status_and_uninstall_use_real_binary",
};

macro_rules! cli_contract_path {
    // Attachment CLI rows historically use the logical operation as path because the contract
    // is produced by the server-side file-backed round-trip test.
    ("attachment-add", $operation:literal) => {
        $operation
    };
    ("attachment-list", $operation:literal) => {
        $operation
    };
    ("attachment-remove", $operation:literal) => {
        $operation
    };
    ($contract_slug:literal, $operation:literal) => {
        concat!("kanban ", $operation, " --json stdout")
    };
}

macro_rules! adopted_cli_operation {
    (
        $operation_id:literal,
        $key:literal,
        $contract_slug:literal,
        $operation:literal,
        $schema_title:literal,
        $schema_type:ty,
        $witness:expr
    ) => {{
        static CONTRACTS: &[ContractDeclaration] = &[{
            let contract = ContractDeclaration::new(
                concat!("cli.", $contract_slug, ".output"),
                cli_contract_path!($contract_slug, $operation),
                ContractDirection::Serialize,
                None,
                ContractStrictness::DenyUnknownFields,
                ContractGranularity::Exact,
                ContractBinding::ExactSurface,
            )
            .with_schema(
                concat!("urn:kanban-tool:schema:cli:", $contract_slug, "-output:v1"),
                concat!("cli/", $contract_slug, "-output.v1.schema.json"),
                $schema_title,
                concat!(
                    "schemas/fixtures/cli/",
                    $contract_slug,
                    "-output.v1.valid.json"
                ),
                concat!(
                    "schemas/fixtures/cli/",
                    $contract_slug,
                    "-output.v1.invalid.json"
                ),
            )
            .with_adoption($witness, $witness);
            #[cfg(feature = "schema")]
            let contract = contract.with_schema_type::<$schema_type>();
            contract
        }];
        OperationDeclaration::new(
            $operation_id,
            ContractSurface::Cli,
            None,
            None,
            $key,
            $key,
            MigrationState::Adopted,
            CONTRACTS,
        )
    }};
}

macro_rules! excluded_cli_operation {
    ($operation_id:literal, $key:literal, $reason:literal) => {
        OperationDeclaration::new(
            $operation_id,
            ContractSurface::Cli,
            None,
            None,
            $key,
            $key,
            MigrationState::Excluded,
            &[],
        )
        .with_exclusion($reason)
    };
}

const CLI_SHELL_OPERATIONS: &[OperationDeclaration] = &[
    excluded_cli_operation!(
        "cli.__complete",
        "__complete",
        "隐藏的动态补全候选使用逐行文本协议，不是 JSON document"
    ),
    adopted_cli_operation!(
        "cli.backup",
        "backup",
        "maintenance-backup",
        "backup",
        "Kanban CLI maintenance backup output v1",
        crate::BackupResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.board-current",
        "board current",
        "board-current",
        "board current",
        "Kanban CLI board current output v1",
        crate::CliBoardCurrentOutput,
        CLI_QUEUE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.board-use",
        "board use",
        "board-use",
        "board use",
        "Kanban CLI board use output v1",
        crate::CliBoardUseOutput,
        CLI_QUEUE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.checkpoint",
        "checkpoint",
        "checkpoint",
        "checkpoint",
        "Kanban CLI checkpoint output v1",
        crate::CheckpointResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.comment-add",
        "comment add",
        "comment-add",
        "comment add",
        "Kanban CLI comment add output v1",
        crate::CliCommentAddOutput,
        CLI_HISTORY_WITNESS
    ),
    adopted_cli_operation!(
        "cli.comment-list",
        "comment list",
        "comment-list",
        "comment list",
        "Kanban CLI comment list output v1",
        crate::CliCommentListOutput,
        CLI_HISTORY_WITNESS
    ),
    adopted_cli_operation!(
        "cli.attachment-add",
        "attachment add",
        "attachment-add",
        "attachment add",
        "Kanban CLI attachment add output v1",
        crate::CliAttachmentAddOutput,
        CLI_HISTORY_WITNESS
    ),
    adopted_cli_operation!(
        "cli.attachment-list",
        "attachment list",
        "attachment-list",
        "attachment list",
        "Kanban CLI attachment list output v1",
        crate::CliAttachmentListOutput,
        CLI_HISTORY_WITNESS
    ),
    adopted_cli_operation!(
        "cli.attachment-remove",
        "attachment remove",
        "attachment-remove",
        "attachment remove",
        "Kanban CLI attachment remove output v1",
        crate::CliAttachmentRemoveOutput,
        CLI_HISTORY_WITNESS
    ),
    excluded_cli_operation!(
        "cli.attachment-download",
        "attachment download",
        "附件下载输出是原始 bytes 文件，不是 JSON machine envelope"
    ),
    excluded_cli_operation!(
        "cli.completions",
        "completions",
        "shell completion script 是文本脚本，不是 JSON document"
    ),
    adopted_cli_operation!(
        "cli.config-show",
        "config show",
        "config-show",
        "config show",
        "Kanban CLI config show output v1",
        crate::CliConfigShowOutput,
        CLI_QUEUE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.context-build",
        "context build",
        "context-build",
        "context build",
        "Kanban CLI context build output v1",
        crate::CliContextBuildOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.dep-add",
        "dep add",
        "dep-add",
        "dep add",
        "Kanban CLI dependency add output v1",
        crate::CliDependencyAddOutput,
        CLI_STEPS_DEPENDENCIES_WITNESS
    ),
    adopted_cli_operation!(
        "cli.dep-list",
        "dep list",
        "dep-list",
        "dep list",
        "Kanban CLI dependency list output v1",
        crate::CliDependencyListOutput,
        CLI_STEPS_DEPENDENCIES_WITNESS
    ),
    adopted_cli_operation!(
        "cli.dep-remove",
        "dep remove",
        "dep-remove",
        "dep remove",
        "Kanban CLI dependency remove output v1",
        crate::CliDependencyRemoveOutput,
        CLI_STEPS_DEPENDENCIES_WITNESS
    ),
    adopted_cli_operation!(
        "cli.doctor",
        "doctor",
        "doctor",
        "doctor",
        "Kanban CLI doctor output v1",
        crate::CliDoctorOutput,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.entity-list",
        "entity list",
        "entity-list",
        "entity list",
        "Kanban CLI entity list output v1",
        crate::CliEntityListOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.entity-show",
        "entity show",
        "entity-show",
        "entity show",
        "Kanban CLI entity show output v1",
        crate::CliEntityShowOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.entity-upsert",
        "entity upsert",
        "entity-upsert",
        "entity upsert",
        "Kanban CLI entity upsert output v1",
        crate::CliEntityUpsertOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.events",
        "events",
        "events",
        "events",
        "Kanban CLI events output v1",
        crate::CliEventsOutput,
        CLI_HISTORY_WITNESS
    ),
    adopted_cli_operation!(
        "cli.export",
        "export",
        "maintenance-export",
        "export",
        "Kanban CLI maintenance export output v1",
        crate::ExportResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-neighbors",
        "graph neighbors",
        "graph-neighbors",
        "graph neighbors",
        "Kanban CLI graph neighbors output v1",
        crate::CliGraphNeighborsOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-query",
        "graph query",
        "graph-query",
        "graph query",
        "Kanban CLI graph query output v1",
        crate::CliGraphQueryOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-neighborhood",
        "graph neighborhood",
        "graph-neighborhood",
        "graph neighborhood",
        "Kanban CLI graph neighborhood output v1",
        crate::CliGraphNeighborhoodOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-map",
        "graph map",
        "graph-map",
        "graph map",
        "Kanban CLI graph map output v1",
        crate::CliGraphMapOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-rebuild",
        "graph rebuild",
        "graph-rebuild",
        "graph rebuild",
        "Kanban CLI graph rebuild output v1",
        crate::CliGraphRebuildOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-status",
        "graph status",
        "graph-status",
        "graph status",
        "Kanban CLI graph status output v1",
        crate::CliGraphStatusOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.graph-sync",
        "graph sync",
        "graph-sync",
        "graph sync",
        "Kanban CLI graph sync output v1",
        crate::CliGraphSyncOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    excluded_cli_operation!(
        "cli.hook-codex-handle-failure",
        "hook codex handle failure",
        "Codex hook handler 使用独立 stdin/stdout protocol，不走通用 CLI JSON envelope"
    ),
    excluded_cli_operation!(
        "cli.hook-codex-handle-task-create",
        "hook codex handle task-create",
        "Codex hook handler 使用独立 stdin/stdout protocol，不走通用 CLI JSON envelope"
    ),
    adopted_cli_operation!(
        "cli.hook-codex-install",
        "hook codex install",
        "hook-codex-install",
        "hook codex install",
        "Kanban CLI hook codex install output v1",
        crate::CliHookCodexInstallOutput,
        CLI_HOOK_WITNESS
    ),
    adopted_cli_operation!(
        "cli.hook-codex-status",
        "hook codex status",
        "hook-codex-status",
        "hook codex status",
        "Kanban CLI hook codex status output v1",
        crate::CliHookCodexStatusOutput,
        CLI_HOOK_WITNESS
    ),
    adopted_cli_operation!(
        "cli.hook-codex-uninstall",
        "hook codex uninstall",
        "hook-codex-uninstall",
        "hook codex uninstall",
        "Kanban CLI hook codex uninstall output v1",
        crate::CliHookCodexUninstallOutput,
        CLI_HOOK_WITNESS
    ),
    adopted_cli_operation!(
        "cli.import",
        "import",
        "maintenance-import",
        "import",
        "Kanban CLI maintenance import output v1",
        crate::ImportResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.index-doctor",
        "index doctor",
        "index-doctor",
        "index doctor",
        "Kanban CLI index doctor output v1",
        crate::CliIndexDoctorOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.index-rebuild",
        "index rebuild",
        "index-rebuild",
        "index rebuild",
        "Kanban CLI index rebuild output v1",
        crate::CliIndexRebuildOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.index-status",
        "index status",
        "index-status",
        "index status",
        "Kanban CLI index status output v1",
        crate::CliIndexStatusOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.index-sync",
        "index sync",
        "index-sync",
        "index sync",
        "Kanban CLI index sync output v1",
        crate::CliIndexSyncOutput,
        CLI_KNOWLEDGE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.init",
        "init",
        "init",
        "init",
        "Kanban CLI init output v1",
        crate::CliInitOutput,
        CLI_QUEUE_WITNESS
    ),
    adopted_cli_operation!(
        "cli.maintenance-rebuild-v1",
        "maintenance rebuild",
        "maintenance-rebuild",
        "maintenance rebuild",
        "Kanban CLI maintenance rebuild output v1",
        crate::MaintenanceRunResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.maintenance-run-v1",
        "maintenance run",
        "maintenance-run",
        "maintenance run",
        "Kanban CLI maintenance run output v1",
        crate::MaintenanceRunResponse,
        CLI_ADMIN_WITNESS
    ),
    adopted_cli_operation!(
        "cli.maintenance-status-v1",
        "maintenance status",
        "maintenance-status",
        "maintenance status",
        "Kanban CLI maintenance status output v1",
        crate::MaintenanceStatusResponse,
        CLI_ADMIN_WITNESS
    ),
];

/// 返回 CLI shell 区段的 parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    CLI_SHELL_OPERATIONS
}

/// 返回该区段的全部 output contract projection，保持 source 顺序。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(CLI_SHELL_OPERATIONS).contracts()
}

/// 按 canonical CLI key 查找 parent projection。
pub fn surface_operation(key: &str) -> Option<SurfaceOperation> {
    crate::CatalogProjection::new(CLI_SHELL_OPERATIONS)
        .surfaces()
        .into_iter()
        .find(|operation| operation.key == key)
}

/// 返回该区段的全部 non-HTTP surface projection。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(CLI_SHELL_OPERATIONS).surfaces()
}

/// 判断 contract 是否属于 CLI shell source。
pub fn owns_contract(id: &str) -> bool {
    CLI_SHELL_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

/// 判断 operation parent 是否属于 CLI shell source。
pub fn owns_operation(id: &str) -> bool {
    CLI_SHELL_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(feature = "schema")]
/// 返回该区段的显式 schema roots。
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(CLI_SHELL_OPERATIONS).schemas()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_shell_source_keeps_contiguous_surface_slice_without_board_rows() {
        let keys = surface_catalog()
            .into_iter()
            .map(|operation| operation.key)
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 44);
        assert_eq!(keys[0], "__complete");
        assert_eq!(keys.last().map(String::as_str), Some("maintenance status"));
        for key in [
            "board archive",
            "board columns",
            "board create",
            "board list",
            "board show",
        ] {
            assert!(!keys.iter().any(|candidate| candidate == key));
        }
        assert_eq!(operation_contracts().len(), 39);
    }

    #[test]
    fn cli_shell_source_preserves_legacy_contract_projection() {
        let inventory = crate::operation_inventory();
        for source in operation_contracts() {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "CLI contract must be projected once: {}",
                source.id
            );
            assert_eq!(matches[0], &source, "CLI contract changed: {}", source.id);
        }

        let legacy = crate::surface_operation_catalog();
        for source in surface_catalog() {
            let matching = legacy
                .iter()
                .filter(|operation| operation.key == source.key)
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "CLI surface must be projected once: {}",
                source.key
            );
            assert_eq!(matching[0], &source, "CLI surface changed: {}", source.key);
        }
    }
}
