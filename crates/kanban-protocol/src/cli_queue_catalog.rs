//! Queue、task、step、dependency、stats、vector 与 maintenance CLI 的声明 source。
//!
//! 每个 parent 只描述一个 Clap leaf 的 `--json` stdout contract；schema、fixture 和
//! producer/consumer witness 都在声明点固定。CLI adapter、host、service 与真实测试仍
//! 由各自 crate 持有；本模块只提供协议事实及其 deterministic projection。

#[cfg(feature = "schema")]
use crate::cli_helpers::{
    CliVectorConfigureOutput, CliVectorQueryChunksOutput, CliVectorQueryLabelAtomsOutput,
    CliVectorRebuildOutput, CliVectorStatusOutput, CliVectorSyncOutput,
};
use crate::{
    AdoptionLocator, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, MigrationState, OperationContract, OperationDeclaration,
    SurfaceOperation,
};
#[cfg(feature = "schema")]
use crate::{
    CliDependencyAddOutput, CliDependencyListOutput, CliDependencyRemoveOutput, CliStatsOutput,
    CliTaskArchiveOutput, CliTaskBlockOutput, CliTaskClaimOutput, CliTaskCreateOutput,
    CliTaskDoneOutput, CliTaskHeartbeatOutput, CliTaskListOutput, CliTaskPromoteOutput,
    CliTaskReclaimOutput, CliTaskReleaseOutput, CliTaskReopenOutput, CliTaskReviewOutput,
    CliTaskShowOutput, CliTaskSpecifyOutput, CliTaskStepAddOutput, CliTaskStepDoneOutput,
    CliTaskStepListOutput, CliTaskStepNotRequiredOutput, CliTaskStepRemoveOutput,
    CliTaskStepReopenOutput, CliTaskStepSkipOutput, CliTaskStepUpdateOutput, CliTaskUnblockOutput,
    CliTaskUpdateOutput, LegacyImportResponse, MaintenanceRunResponse, VacuumResponse,
};

macro_rules! cli_contract {
    (
        $slug:literal,
        $command:literal,
        $schema_type:ty,
        $test_target:literal,
        $producer:literal,
        $consumer:literal
    ) => {{
        cli_contract_with_title!(
            $slug,
            $command,
            concat!("Kanban CLI ", $command, " output v1"),
            $schema_type,
            $test_target,
            $producer,
            $consumer
        )
    }};
}

macro_rules! cli_contract_with_title {
    (
        $slug:literal,
        $command:literal,
        $title:expr,
        $schema_type:ty,
        $test_target:literal,
        $producer:literal,
        $consumer:literal
    ) => {{
        let contract = ContractDeclaration::new(
            concat!("cli.", $slug, ".output"),
            concat!("kanban ", $command, " --json stdout"),
            ContractDirection::Serialize,
            None,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            concat!("urn:kanban-tool:schema:cli:", $slug, "-output:v1"),
            concat!("cli/", $slug, "-output.v1.schema.json"),
            $title,
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.valid.json"),
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.invalid.json"),
        )
        .with_adoption(
            AdoptionLocator {
                package: "kanban-cli",
                test_target: $test_target,
                exact_test: $producer,
            },
            AdoptionLocator {
                package: "kanban-cli",
                test_target: $test_target,
                exact_test: $consumer,
            },
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! cli_operation {
    (
        $slug:literal,
        $command:literal,
        $schema_type:ty,
        $test_target:literal,
        $producer:literal,
        $consumer:literal
    ) => {
        OperationDeclaration::new(
            concat!("cli.", $slug),
            ContractSurface::Cli,
            None,
            None,
            $command,
            $command,
            MigrationState::Adopted,
            &[const {
                cli_contract!(
                    $slug,
                    $command,
                    $schema_type,
                    $test_target,
                    $producer,
                    $consumer
                )
            }],
        )
    };
}

macro_rules! cli_operation_with_title {
    (
        $slug:literal,
        $command:literal,
        $title:expr,
        $schema_type:ty,
        $test_target:literal,
        $producer:literal,
        $consumer:literal
    ) => {
        OperationDeclaration::new(
            concat!("cli.", $slug),
            ContractSurface::Cli,
            None,
            None,
            $command,
            $command,
            MigrationState::Adopted,
            &[const {
                cli_contract_with_title!(
                    $slug,
                    $command,
                    $title,
                    $schema_type,
                    $test_target,
                    $producer,
                    $consumer
                )
            }],
        )
    };
}

const CLI_QUEUE_OPERATIONS: &[OperationDeclaration] = &[
    cli_operation_with_title!(
        "dep-add",
        "dep add",
        "Kanban CLI dependency add output v1",
        CliDependencyAddOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation_with_title!(
        "dep-list",
        "dep list",
        "Kanban CLI dependency list output v1",
        CliDependencyListOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation_with_title!(
        "dep-remove",
        "dep remove",
        "Kanban CLI dependency remove output v1",
        CliDependencyRemoveOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "stats",
        "stats",
        CliStatsOutput,
        "cli_admin_adoption",
        "maintenance_admin_commands_use_real_host_and_typed_json",
        "maintenance_admin_commands_use_real_host_and_typed_json"
    ),
    cli_operation!(
        "task-archive",
        "task archive",
        CliTaskArchiveOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-block",
        "task block",
        CliTaskBlockOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-claim",
        "task claim",
        CliTaskClaimOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-create",
        "task create",
        CliTaskCreateOutput,
        "cli_queue_adoption",
        "queue_cli_uses_real_host_for_config_board_and_task_commands",
        "queue_cli_uses_real_host_for_config_board_and_task_commands"
    ),
    cli_operation!(
        "task-done",
        "task done",
        CliTaskDoneOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-heartbeat",
        "task heartbeat",
        CliTaskHeartbeatOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-release",
        "task release",
        CliTaskReleaseOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-list",
        "task list",
        CliTaskListOutput,
        "cli_queue_adoption",
        "queue_cli_uses_real_host_for_config_board_and_task_commands",
        "queue_cli_uses_real_host_for_config_board_and_task_commands"
    ),
    cli_operation!(
        "task-promote",
        "task promote",
        CliTaskPromoteOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-reclaim",
        "task reclaim",
        CliTaskReclaimOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-reopen",
        "task reopen",
        CliTaskReopenOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-review",
        "task review",
        CliTaskReviewOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-show",
        "task show",
        CliTaskShowOutput,
        "cli_queue_adoption",
        "queue_cli_uses_real_host_for_config_board_and_task_commands",
        "queue_cli_uses_real_host_for_config_board_and_task_commands"
    ),
    cli_operation!(
        "task-specify",
        "task specify",
        CliTaskSpecifyOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-step-add",
        "task step add",
        CliTaskStepAddOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-done",
        "task step done",
        CliTaskStepDoneOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-list",
        "task step list",
        CliTaskStepListOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-not-required",
        "task step not-required",
        CliTaskStepNotRequiredOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-remove",
        "task step remove",
        CliTaskStepRemoveOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-reopen",
        "task step reopen",
        CliTaskStepReopenOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-skip",
        "task step skip",
        CliTaskStepSkipOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-step-update",
        "task step update",
        CliTaskStepUpdateOutput,
        "cli_steps_dependencies_adoption",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes"
    ),
    cli_operation!(
        "task-unblock",
        "task unblock",
        CliTaskUnblockOutput,
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
    ),
    cli_operation!(
        "task-update",
        "task update",
        CliTaskUpdateOutput,
        "cli_queue_adoption",
        "queue_cli_uses_real_host_for_config_board_and_task_commands",
        "queue_cli_uses_real_host_for_config_board_and_task_commands"
    ),
    cli_operation_with_title!(
        "maintenance-vacuum",
        "vacuum",
        "Kanban CLI maintenance vacuum output v1",
        VacuumResponse,
        "cli_admin_adoption",
        "maintenance_admin_commands_use_real_host_and_typed_json",
        "maintenance_admin_commands_use_real_host_and_typed_json"
    ),
    cli_operation!(
        "vector-configure",
        "vector configure",
        CliVectorConfigureOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "vector-query-chunks",
        "vector query-chunks",
        CliVectorQueryChunksOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "vector-query-label-atoms",
        "vector query-label-atoms",
        CliVectorQueryLabelAtomsOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "vector-rebuild",
        "vector rebuild",
        CliVectorRebuildOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "vector-status",
        "vector status",
        CliVectorStatusOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "vector-sync",
        "vector sync",
        CliVectorSyncOutput,
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    cli_operation!(
        "maintenance-cleanup",
        "maintenance cleanup",
        MaintenanceRunResponse,
        "cli_admin_adoption",
        "maintenance_admin_commands_use_real_host_and_typed_json",
        "maintenance_admin_commands_use_real_host_and_typed_json"
    ),
    cli_operation_with_title!(
        "import-v30",
        "import-v30",
        "Kanban CLI legacy SQLite v30 import output v1",
        LegacyImportResponse,
        "cli_admin_adoption",
        "maintenance_admin_commands_use_real_host_and_typed_json",
        "maintenance_admin_commands_use_real_host_and_typed_json"
    ),
];

/// 返回 queue CLI parent declaration source，保持与历史 surface key 相同的顺序。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    CLI_QUEUE_OPERATIONS
}

/// 返回该 family 的 output contract projection。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(CLI_QUEUE_OPERATIONS).contracts()
}

/// 返回 source 中的 CLI surface projection。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(CLI_QUEUE_OPERATIONS).surfaces()
}

/// 按 canonical CLI operation key 查找 source projection。
pub fn surface_operation(key: &str) -> SurfaceOperation {
    surface_catalog()
        .into_iter()
        .find(|operation| operation.key == key)
        .unwrap_or_else(|| panic!("missing queue CLI surface operation: {key}"))
}

/// 判断 contract 是否属于 queue CLI source。
pub fn owns_contract(id: &str) -> bool {
    CLI_QUEUE_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

/// 判断 operation 是否属于 queue CLI source。
pub fn owns_operation(id: &str) -> bool {
    CLI_QUEUE_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(feature = "schema")]
/// 返回 source 中显式 schema roots。
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(CLI_QUEUE_OPERATIONS).schemas()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_source_projects_unique_adopted_cli_outputs() {
        assert_eq!(CLI_QUEUE_OPERATIONS.len(), 37);
        let contracts = operation_contracts();
        assert_eq!(contracts.len(), 37);

        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(contracts.iter().all(|contract| {
            contract.surface == ContractSurface::Cli
                && contract.migration == MigrationState::Adopted
                && contract.schema_id.is_some()
                && contract.fixture.is_some()
                && contract.adoption.is_some()
        }));
    }

    #[test]
    fn queue_source_keeps_surface_keys_and_contract_ids_aligned() {
        for declaration in operation_declarations() {
            let surface = declaration.surface_operation();
            assert_eq!(surface.surface, ContractSurface::Cli);
            assert_eq!(surface.key, declaration.key);
            assert_eq!(surface.migration, MigrationState::Adopted);
            assert_eq!(surface.contracts, vec![declaration.contracts[0].id]);
        }
    }

    #[test]
    fn queue_hybrid_projection_replaces_each_legacy_row_once() {
        let inventory = crate::operation_inventory();
        for source in operation_contracts() {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Queue contract must be projected once: {}",
                source.id
            );
            assert_eq!(matches[0], &source, "Queue contract changed: {}", source.id);
        }
    }
}
