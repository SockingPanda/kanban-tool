//! MCP tool 与 canonical HTTP operation 的机器可读绑定。
//!
//! 这里记录的是 MCP adapter 的能力边界，不复制 rmcp handler 的参数 schema，也不把
//! service/domain 规则搬进 protocol。每个公开 tool 必须只绑定到当前
//! [`crate::endpoint_catalog`] 中真实存在的领域 HTTP operation；selector adapter
//! 需要先解析任务时，可以在同一个条目中声明多个 operation。

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{ContractSurface, endpoint_catalog};

/// MCP tool 的能力分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOperationClass {
    /// 通过 typed localhost client 访问 canonical application/domain path。
    Domain,
    /// host-owned migration、maintenance 或 database 管理能力。
    ///
    /// 该分类保留给边界校验；MCP catalog 不允许暴露此类 operation。
    HostAdmin,
}

/// 所有公开 MCP domain tool 共同遵守的边界不变量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOperationInvariant {
    /// 只能通过 canonical host 提供服务，不打开本地数据库。
    CanonicalHostOnly,
    /// mutation/query 都走共享 application service 与 typed HTTP client。
    SharedApplicationService,
    /// MCP 不暴露 host-admin、migration 或 database replace。
    NoHostAdminSurface,
}

const DOMAIN_INVARIANTS: &[McpOperationInvariant] = &[
    McpOperationInvariant::CanonicalHostOnly,
    McpOperationInvariant::SharedApplicationService,
    McpOperationInvariant::NoHostAdminSurface,
];

/// MCP 明确禁止绑定的 host-admin HTTP operation。
///
/// 领域 projection 的 `label atom-index/rebuild`、search/graph/vector configure/rebuild/sync
/// 都是现有 domain capability，因而不在这里列为 host-admin；数据库、迁移、portable
/// replace 和通用 maintenance 则始终由 host/CLI/Desktop 管理，不得新增 MCP tool。
pub const MCP_HOST_ADMIN_OPERATION_IDS: &[&str] = &[
    "api.health",
    "api.doctor",
    "api.checkpoint",
    "api.maintenance-backup",
    "api.maintenance-export",
    "api.maintenance-import",
    "api.maintenance-vacuum",
    "api.maintenance-status",
    "api.maintenance-run",
    "api.maintenance-rebuild",
    "api.maintenance-cleanup",
    "api.maintenance-import-v30",
];

/// 一个 MCP tool 的 canonical name、HTTP operation 绑定和边界不变量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct McpOperationDescriptor {
    /// rmcp router 对外暴露的 canonical tool name。
    pub tool_name: &'static str,
    /// 该 tool 可能调用的完整领域 HTTP operation 集合（按调用顺序声明）。
    pub http_operations: &'static [&'static str],
    /// 当前 MCP surface 的能力分类；公开 catalog 只能是 `Domain`。
    pub class: McpOperationClass,
    /// 该 tool 必须保持的 adapter/application 边界不变量。
    pub invariants: &'static [McpOperationInvariant],
}

macro_rules! domain_tool {
    ($tool:literal => [$($operation:literal),+ $(,)?]) => {
        McpOperationDescriptor {
            tool_name: $tool,
            http_operations: &[$($operation),+],
            class: McpOperationClass::Domain,
            invariants: DOMAIN_INVARIANTS,
        }
    };
}

/// 当前 MCP domain tool 的唯一 machine-readable catalog。
///
/// 顺序与 rmcp `ToolRouter::list_all()` 的 canonical name 顺序一致，便于 adapter exact
/// test 在新增、删除或重命名时立即失败。
pub const MCP_OPERATION_CATALOG: &[McpOperationDescriptor] = &[
    domain_tool!("attachment_create" => ["api.list-tasks", "api.create-attachment"]),
    domain_tool!("attachment_download" => ["api.list-tasks", "api.download-attachment"]),
    domain_tool!("attachment_list" => ["api.list-tasks", "api.list-attachments"]),
    domain_tool!("attachment_remove" => ["api.list-tasks", "api.delete-attachment"]),
    domain_tool!("board_archive" => ["api.archive-board"]),
    domain_tool!("board_columns" => ["api.list-board-columns"]),
    domain_tool!("board_create" => ["api.create-board"]),
    domain_tool!("board_list" => ["api.list-boards"]),
    domain_tool!("board_show" => ["api.get-board"]),
    domain_tool!("board_task_map" => ["api.board-task-map"]),
    domain_tool!("comment_create" => ["api.list-tasks", "api.create-comment"]),
    domain_tool!("comment_list" => ["api.list-tasks", "api.list-comments"]),
    domain_tool!("context_build" => ["api.build-context"]),
    domain_tool!("dependency_create" => ["api.list-tasks", "api.add-dependency"]),
    domain_tool!("dependency_list" => ["api.list-tasks", "api.list-dependencies"]),
    domain_tool!("dependency_remove" => ["api.list-tasks", "api.remove-dependency"]),
    domain_tool!("entity_list" => ["api.list-entities"]),
    domain_tool!("entity_show" => ["api.get-entity"]),
    domain_tool!("entity_upsert" => ["api.upsert-entity"]),
    domain_tool!("event_list" => ["api.list-tasks", "api.list-events"]),
    domain_tool!("graph_neighbors" => ["api.graph-neighbors"]),
    domain_tool!("graph_query" => ["api.graph-query"]),
    domain_tool!("graph_rebuild" => ["api.graph-rebuild"]),
    domain_tool!("graph_status" => ["api.graph-status"]),
    domain_tool!("graph_sync" => ["api.graph-sync"]),
    domain_tool!("label_atom_explain" => ["api.explain-label-atom"]),
    domain_tool!("label_atom_index_query" => ["api.query-label-atom-index"]),
    domain_tool!("label_atom_index_rebuild" => ["api.rebuild-label-atom-index"]),
    domain_tool!("label_atom_index_status" => ["api.label-atom-index-status"]),
    domain_tool!("label_atoms_list" => ["api.list-label-atoms"]),
    domain_tool!("label_create" => ["api.create-board-label"]),
    domain_tool!("label_list" => ["api.list-board-labels"]),
    domain_tool!("label_ontology_action" => ["api.create-label-ontology-action"]),
    domain_tool!("label_ontology_apply_atom" => ["api.apply-label-ontology-atom"]),
    domain_tool!("label_ontology_observe" => ["api.record-label-ontology-observation"]),
    domain_tool!("label_ontology_quality" => ["api.review-label-ontology"]),
    domain_tool!("label_ontology_revert" => ["api.revert-label-ontology-mutation"]),
    domain_tool!("label_ontology_review" => ["api.review-label-ontology"]),
    domain_tool!("label_ontology_signal_show" => ["api.get-label-ontology-signal"]),
    domain_tool!("label_ontology_signals" => ["api.list-label-ontology-signals"]),
    domain_tool!("label_ontology_validate" => ["api.validate-label-ontology-action"]),
    domain_tool!("label_proposal_accept" => ["api.accept-label-proposal"]),
    domain_tool!("label_proposal_reject" => ["api.reject-label-proposal"]),
    domain_tool!("label_proposal_show" => ["api.get-label-proposal"]),
    domain_tool!("label_proposals_list" => ["api.list-task-label-proposals"]),
    domain_tool!("label_propose" => ["api.propose-task-label"]),
    domain_tool!("label_semantics_delete" => ["api.delete-label-semantics"]),
    domain_tool!("label_semantics_list" => ["api.list-label-semantics"]),
    domain_tool!("label_semantics_show" => ["api.get-label-semantics"]),
    domain_tool!("label_semantics_upsert" => ["api.upsert-label-semantics"]),
    domain_tool!("label_suggest" => ["api.suggest-task-labels"]),
    domain_tool!("run_list" => ["api.list-tasks", "api.list-runs"]),
    domain_tool!("run_log" => ["api.get-run-log"]),
    domain_tool!("run_show" => ["api.get-run"]),
    domain_tool!("search_index_rebuild" => ["api.rebuild-search-index"]),
    domain_tool!("search_index_sync" => ["api.sync-search-index"]),
    domain_tool!("search_status" => ["api.search-status"]),
    domain_tool!("search_tasks" => ["api.search-tasks"]),
    domain_tool!("search_tasks_by_status" => ["api.search-tasks-by-status"]),
    domain_tool!("signal_confirm" => ["api.confirm-signals"]),
    domain_tool!("signal_list" => ["api.list-signals"]),
    domain_tool!("signal_record" => ["api.record-signal"]),
    domain_tool!("signal_reject" => ["api.reject-signals"]),
    domain_tool!("signal_resolve" => ["api.resolve-signals"]),
    domain_tool!("signal_review" => ["api.review-signals"]),
    domain_tool!("signal_show" => ["api.get-signal"]),
    domain_tool!("signal_supersede" => ["api.supersede-signals"]),
    domain_tool!("stats" => ["api.get-stats"]),
    domain_tool!("step_create" => ["api.list-tasks", "api.create-step"]),
    domain_tool!("step_done" => ["api.list-tasks", "api.list-steps", "api.complete-step"]),
    domain_tool!("step_list" => ["api.list-tasks", "api.list-steps"]),
    domain_tool!("step_remove" => ["api.list-tasks", "api.list-steps", "api.remove-step"]),
    domain_tool!("step_reopen" => ["api.list-tasks", "api.list-steps", "api.reopen-step"]),
    domain_tool!("step_skip" => ["api.list-tasks", "api.list-steps", "api.skip-step"]),
    domain_tool!("step_update" => ["api.list-tasks", "api.list-steps", "api.update-step"]),
    domain_tool!("task_archive" => ["api.list-tasks", "api.archive-task"]),
    domain_tool!("task_block" => ["api.list-tasks", "api.block-task"]),
    domain_tool!("task_claim" => ["api.list-tasks", "api.claim-task"]),
    domain_tool!("task_create" => ["api.create-task"]),
    domain_tool!("task_done" => ["api.list-tasks", "api.complete-task"]),
    domain_tool!("task_heartbeat" => ["api.list-tasks", "api.heartbeat-task"]),
    domain_tool!("task_label_add" => ["api.list-tasks", "api.add-task-label"]),
    domain_tool!("task_label_list" => ["api.list-tasks", "api.list-task-labels"]),
    domain_tool!("task_label_remove" => ["api.list-tasks", "api.remove-task-label"]),
    domain_tool!("task_list" => ["api.list-tasks"]),
    domain_tool!("task_list_by_status" => ["api.list-tasks-by-status"]),
    domain_tool!("task_neighborhood" => ["api.task-neighborhood"]),
    domain_tool!(
        "task_plan_not_required" => ["api.list-tasks", "api.mark-execution-plan-not-required"]
    ),
    domain_tool!("task_promote" => ["api.list-tasks", "api.promote-task"]),
    domain_tool!("task_reclaim" => ["api.list-tasks", "api.reclaim-task"]),
    domain_tool!("task_release" => ["api.list-tasks", "api.release-task"]),
    domain_tool!("task_reopen" => ["api.list-tasks", "api.reopen-task"]),
    domain_tool!("task_review" => ["api.list-tasks", "api.submit-review-task"]),
    domain_tool!("task_show" => ["api.list-tasks", "api.get-task"]),
    domain_tool!("task_specify" => ["api.list-tasks", "api.specify-task"]),
    domain_tool!("task_unblock" => ["api.list-tasks", "api.unblock-task"]),
    domain_tool!("task_update" => ["api.list-tasks", "api.update-task"]),
    domain_tool!("vector_configure" => ["api.vector-configure"]),
    domain_tool!("vector_query_chunks" => ["api.vector-query-chunks"]),
    domain_tool!("vector_query_label_atoms" => ["api.vector-query-label-atoms"]),
    domain_tool!("vector_rebuild" => ["api.vector-rebuild"]),
    domain_tool!("vector_status" => ["api.vector-status"]),
    domain_tool!("vector_sync" => ["api.vector-sync"]),
];

/// 返回 canonical MCP operation catalog。
pub fn mcp_operation_catalog() -> &'static [McpOperationDescriptor] {
    MCP_OPERATION_CATALOG
}

/// 按 canonical tool name 查找 MCP operation descriptor。
pub fn mcp_operation_descriptor(tool_name: &str) -> Option<&'static McpOperationDescriptor> {
    MCP_OPERATION_CATALOG
        .iter()
        .find(|descriptor| descriptor.tool_name == tool_name)
}

/// 验证 MCP catalog 的唯一性、endpoint 存在性和 host-admin 隔离。
pub fn validate_mcp_operation_catalog(catalog: &[McpOperationDescriptor]) -> Result<(), String> {
    let endpoint_ids = endpoint_catalog()
        .iter()
        .map(|endpoint| endpoint.operation_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut tool_names = BTreeMap::new();

    for (index, descriptor) in catalog.iter().enumerate() {
        if descriptor.tool_name.is_empty() {
            return Err(format!("MCP catalog 第 {index} 项缺少 canonical tool name"));
        }
        if let Some(first_index) = tool_names.insert(descriptor.tool_name, index) {
            return Err(format!(
                "MCP catalog 存在重复 tool name：{}（第 {first_index}、{index} 项）",
                descriptor.tool_name
            ));
        }
        if descriptor.class != McpOperationClass::Domain {
            return Err(format!(
                "MCP catalog 不得暴露 host-admin tool：{}",
                descriptor.tool_name
            ));
        }
        if descriptor.invariants != DOMAIN_INVARIANTS {
            return Err(format!(
                "MCP domain tool 不变量不完整：{}",
                descriptor.tool_name
            ));
        }
        if descriptor.http_operations.is_empty() {
            return Err(format!(
                "MCP tool 缺少 HTTP operation 绑定：{}",
                descriptor.tool_name
            ));
        }
        for operation_id in descriptor.http_operations {
            if !endpoint_ids.contains(operation_id) {
                return Err(format!(
                    "MCP tool {} 引用了不存在的 endpoint operation：{}",
                    descriptor.tool_name, operation_id
                ));
            }
            let endpoint =
                crate::endpoint_descriptor(operation_id).expect("endpoint ID 已验证存在");
            if endpoint.surface != ContractSurface::Api {
                return Err(format!(
                    "MCP tool {} 只能绑定 API endpoint，实际为 {:?}：{}",
                    descriptor.tool_name, endpoint.surface, operation_id
                ));
            }
            if MCP_HOST_ADMIN_OPERATION_IDS.contains(operation_id) {
                return Err(format!(
                    "MCP tool {} 绑定了禁止的 host-admin operation：{}",
                    descriptor.tool_name, operation_id
                ));
            }
        }
    }

    let bound_operations = catalog
        .iter()
        .flat_map(|descriptor| descriptor.http_operations)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let missing_operations = endpoint_catalog()
        .iter()
        .filter(|endpoint| {
            endpoint.surface == ContractSurface::Api
                && !MCP_HOST_ADMIN_OPERATION_IDS.contains(&endpoint.operation_id)
        })
        .map(|endpoint| endpoint.operation_id)
        .filter(|operation_id| !bound_operations.contains(operation_id))
        .collect::<Vec<_>>();
    if !missing_operations.is_empty() {
        return Err(format!(
            "MCP catalog 缺少非 host-admin API operation：{}",
            missing_operations.join(", ")
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_valid_and_has_unique_sorted_tool_names() {
        validate_mcp_operation_catalog(MCP_OPERATION_CATALOG).unwrap();
        assert_eq!(MCP_OPERATION_CATALOG.len(), 103);
        let names = MCP_OPERATION_CATALOG
            .iter()
            .map(|descriptor| descriptor.tool_name)
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn host_admin_operations_are_never_bound() {
        for descriptor in MCP_OPERATION_CATALOG {
            assert_eq!(descriptor.class, McpOperationClass::Domain);
            for operation_id in descriptor.http_operations {
                assert!(!MCP_HOST_ADMIN_OPERATION_IDS.contains(operation_id));
            }
        }
    }

    #[test]
    fn invalid_endpoint_and_host_admin_bindings_fail_closed() {
        const INVARIANTS: &[McpOperationInvariant] = &[
            McpOperationInvariant::CanonicalHostOnly,
            McpOperationInvariant::SharedApplicationService,
            McpOperationInvariant::NoHostAdminSurface,
        ];
        let missing = [McpOperationDescriptor {
            tool_name: "broken",
            http_operations: &["api.not-real"],
            class: McpOperationClass::Domain,
            invariants: INVARIANTS,
        }];
        assert!(
            validate_mcp_operation_catalog(&missing)
                .expect_err("缺失 endpoint 时必须失败")
                .contains("api.not-real")
        );

        let host_admin = [McpOperationDescriptor {
            tool_name: "backup",
            http_operations: &["api.maintenance-backup"],
            class: McpOperationClass::Domain,
            invariants: INVARIANTS,
        }];
        assert!(
            validate_mcp_operation_catalog(&host_admin)
                .expect_err("绑定 host-admin endpoint 时必须失败")
                .contains("host-admin")
        );
    }
}
