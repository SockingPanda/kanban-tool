//! MCP tool 与 canonical HTTP operation 的机器可读绑定。
//!
//! 这里记录的是 MCP adapter 的能力边界，不复制 rmcp handler 的参数 schema，也不把
//! service/domain 规则搬进 protocol。每个公开 tool 必须只绑定到当前
//! [`crate::endpoint_catalog`] 中真实存在的领域 HTTP operation；selector adapter
//! 需要先解析任务时，可以在同一个条目中声明多个 operation。

use std::{collections::BTreeMap, fmt};

use serde::Serialize;

use crate::{
    contract_catalog::McpExposure, endpoint_catalog, ContractSurface, OperationDeclaration,
};

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

/// declaration source 投影时的冲突。
///
/// 一个 tool 可以由多个 parent declaration 提供 operation；只有同名 tool 的
/// exposure 与 invariants 完全一致时，projection 才会合并其 operation。这样新增
/// family 不需要复制 MCP registry，但错误的跨边界绑定仍会在 source 层 fail closed。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpProjectionError {
    /// tool 名称为空，无法形成稳定的 adapter binding。
    EmptyToolName { operation_id: &'static str },
    /// tool 没有任何 HTTP operation，无法形成可调用 binding。
    EmptyToolOperations {
        operation_id: &'static str,
        tool_name: &'static str,
    },
    /// binding 中出现空 operation id。
    EmptyOperationId {
        operation_id: &'static str,
        tool_name: &'static str,
    },
    /// 同名 tool 的 exposure 不一致。
    ToolExposureConflict {
        tool_name: &'static str,
        first: McpOperationClass,
        second: McpOperationClass,
    },
    /// 同名 tool 的边界 invariants 不一致。
    ToolInvariantConflict { tool_name: &'static str },
    /// 同一个 operation 被 declaration source 同时归入 Domain 与 HostAdmin。
    OperationExposureConflict { operation_id: &'static str },
}

impl fmt::Display for McpProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyToolName { operation_id } => {
                write!(
                    formatter,
                    "MCP operation {operation_id} 的 tool name 不能为空"
                )
            }
            Self::EmptyToolOperations {
                operation_id,
                tool_name,
            } => write!(
                formatter,
                "MCP operation {operation_id} 的 tool {tool_name} 缺少 HTTP operation binding"
            ),
            Self::EmptyOperationId {
                operation_id,
                tool_name,
            } => write!(
                formatter,
                "MCP operation {operation_id} 的 tool {tool_name} 含空 HTTP operation id"
            ),
            Self::ToolExposureConflict {
                tool_name,
                first,
                second,
            } => write!(
                formatter,
                "MCP tool {tool_name} 的 exposure 冲突：{first:?} 与 {second:?}"
            ),
            Self::ToolInvariantConflict { tool_name } => {
                write!(formatter, "MCP tool {tool_name} 的 invariants 冲突")
            }
            Self::OperationExposureConflict { operation_id } => write!(
                formatter,
                "MCP operation {operation_id} 同时属于 Domain 与 HostAdmin"
            ),
        }
    }
}

impl std::error::Error for McpProjectionError {}

/// 从一个或多个 declaration parent 投影出的 MCP tool binding。
///
/// `http_operations` 是 owned projection：同名 tool 可以跨 family 声明多个 operation，
/// projection 会按第一次出现的 tool 顺序合并并去重。字段使用静态字符串是因为
/// `OperationDeclaration`/`McpToolBinding` 的 wire literal 本身就是静态 source。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpToolBindingProjection {
    /// rmcp adapter 对外暴露的 canonical tool name。
    pub tool_name: &'static str,
    /// 该 tool 可能调用的 operation，按 source 首次出现顺序排列。
    pub http_operations: Vec<&'static str>,
    /// declaration policy 归属的能力分类。
    pub class: McpOperationClass,
    /// declaration policy 声明的边界不变量，按首次出现顺序去重。
    pub invariants: Vec<McpOperationInvariant>,
}

/// MCP policy 的纯 declaration projection。
///
/// 该 projection 不访问 endpoint registry、handler 或数据库；它只消费输入的
/// `OperationDeclaration` 数组，因此可在任意 family source、测试 fixture 或最终
/// canonical catalog 上重复计算。所有集合均按 declaration/binding source 顺序去重。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct McpPolicyProjection {
    /// 完整 tool binding，第一次出现的 tool 决定结果顺序。
    pub tool_bindings: Vec<McpToolBindingProjection>,
    /// 归入 Domain 的 operation id（含 policy parent 与其 tool binding 引用）。
    pub domain_operations: Vec<&'static str>,
    /// 归入 HostAdmin 的 operation id（含 policy parent 与其 tool binding 引用）。
    pub host_admin_operations: Vec<&'static str>,
    /// Domain policy 使用过的 invariants，按 source 顺序去重。
    pub domain_invariants: Vec<McpOperationInvariant>,
    /// HostAdmin policy 使用过的 invariants，按 source 顺序去重。
    pub host_admin_invariants: Vec<McpOperationInvariant>,
}

impl McpPolicyProjection {
    /// 返回按 source 顺序投影的 tool binding。
    pub fn tool_bindings(&self) -> &[McpToolBindingProjection] {
        &self.tool_bindings
    }

    /// 返回指定能力分类的 operation 集合。
    pub fn operations(&self, class: McpOperationClass) -> &[&'static str] {
        match class {
            McpOperationClass::Domain => &self.domain_operations,
            McpOperationClass::HostAdmin => &self.host_admin_operations,
        }
    }

    /// 返回指定能力分类的不变量集合。
    pub fn invariants(&self, class: McpOperationClass) -> &[McpOperationInvariant] {
        match class {
            McpOperationClass::Domain => &self.domain_invariants,
            McpOperationClass::HostAdmin => &self.host_admin_invariants,
        }
    }
}

/// 将任意 operation declaration source 投影为 MCP policy。
///
/// projection 是纯函数：不排序、不依赖全局 registry，也不假设输入来自某个特定
/// family。重复 operation/tool 会以输入中第一次出现的位置为准；同名 tool 的
/// operation 会合并，exposure 或 invariants 冲突则返回错误。
pub fn project_mcp_policy(
    declarations: &[OperationDeclaration],
) -> Result<McpPolicyProjection, McpProjectionError> {
    let mut projection = McpPolicyProjection::default();
    let mut tool_indexes: BTreeMap<&'static str, usize> = BTreeMap::new();

    for declaration in declarations {
        let Some(policy) = declaration.mcp_policy else {
            continue;
        };
        let class = match policy.exposure {
            McpExposure::Domain => McpOperationClass::Domain,
            McpExposure::HostAdmin => McpOperationClass::HostAdmin,
        };

        push_operation(&mut projection, class, declaration.operation_id)?;
        for invariant in policy.invariants {
            push_invariant(&mut projection, class, *invariant);
        }

        for binding in policy.tool_bindings {
            if binding.tool_name.is_empty() {
                return Err(McpProjectionError::EmptyToolName {
                    operation_id: declaration.operation_id,
                });
            }
            if binding.http_operations.is_empty() {
                return Err(McpProjectionError::EmptyToolOperations {
                    operation_id: declaration.operation_id,
                    tool_name: binding.tool_name,
                });
            }
            for operation_id in binding.http_operations {
                if operation_id.is_empty() {
                    return Err(McpProjectionError::EmptyOperationId {
                        operation_id: declaration.operation_id,
                        tool_name: binding.tool_name,
                    });
                }
                push_operation(&mut projection, class, operation_id)?;
            }

            if let Some(&tool_index) = tool_indexes.get(binding.tool_name) {
                let tool = &mut projection.tool_bindings[tool_index];
                if tool.class != class {
                    return Err(McpProjectionError::ToolExposureConflict {
                        tool_name: binding.tool_name,
                        first: tool.class,
                        second: class,
                    });
                }
                if !same_invariants(&tool.invariants, policy.invariants) {
                    return Err(McpProjectionError::ToolInvariantConflict {
                        tool_name: binding.tool_name,
                    });
                }
                for operation_id in binding.http_operations {
                    if !tool.http_operations.contains(operation_id) {
                        tool.http_operations.push(operation_id);
                    }
                }
            } else {
                let mut http_operations = Vec::new();
                for operation_id in binding.http_operations {
                    if !http_operations.contains(operation_id) {
                        http_operations.push(*operation_id);
                    }
                }
                let mut invariants = Vec::new();
                for invariant in policy.invariants {
                    if !invariants.contains(invariant) {
                        invariants.push(*invariant);
                    }
                }
                tool_indexes.insert(binding.tool_name, projection.tool_bindings.len());
                projection.tool_bindings.push(McpToolBindingProjection {
                    tool_name: binding.tool_name,
                    http_operations,
                    class,
                    invariants,
                });
            }
        }
    }

    Ok(projection)
}

fn push_operation(
    projection: &mut McpPolicyProjection,
    class: McpOperationClass,
    operation_id: &'static str,
) -> Result<(), McpProjectionError> {
    let (operations, opposite) = match class {
        McpOperationClass::Domain => (
            &mut projection.domain_operations,
            &projection.host_admin_operations,
        ),
        McpOperationClass::HostAdmin => (
            &mut projection.host_admin_operations,
            &projection.domain_operations,
        ),
    };
    if opposite.contains(&operation_id) {
        return Err(McpProjectionError::OperationExposureConflict { operation_id });
    }
    if !operations.contains(&operation_id) {
        operations.push(operation_id);
    }
    Ok(())
}

fn push_invariant(
    projection: &mut McpPolicyProjection,
    class: McpOperationClass,
    invariant: McpOperationInvariant,
) {
    let invariants = match class {
        McpOperationClass::Domain => &mut projection.domain_invariants,
        McpOperationClass::HostAdmin => &mut projection.host_admin_invariants,
    };
    if !invariants.contains(&invariant) {
        invariants.push(invariant);
    }
}

fn same_invariants(first: &[McpOperationInvariant], second: &[McpOperationInvariant]) -> bool {
    first.len() == second.len() && first.iter().all(|invariant| second.contains(invariant))
}

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
    domain_tool!("label_proposals_list" => ["api.list-task-label-proposals", "api.list-board-label-proposals"]),
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
    use crate::{ContractSurface, McpPolicy, McpToolBinding, MigrationState, OperationDeclaration};

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

    const TEST_DOMAIN_INVARIANTS: &[McpOperationInvariant] = &[
        McpOperationInvariant::CanonicalHostOnly,
        McpOperationInvariant::SharedApplicationService,
        McpOperationInvariant::NoHostAdminSurface,
    ];
    const TEST_HOST_ADMIN_INVARIANTS: &[McpOperationInvariant] = &[];
    const MERGED_FIRST_BINDING: &[McpToolBinding] = &[McpToolBinding {
        tool_name: "merged",
        http_operations: &["api.example-a", "api.shared"],
    }];
    const MERGED_SECOND_BINDING: &[McpToolBinding] = &[McpToolBinding {
        tool_name: "merged",
        http_operations: &["api.shared", "api.example-b"],
    }];
    const STATS_BINDING: &[McpToolBinding] = &[McpToolBinding {
        tool_name: "stats",
        http_operations: &["api.get-stats"],
    }];
    const DOMAIN_POLICY: McpPolicy = McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: MERGED_FIRST_BINDING,
        invariants: TEST_DOMAIN_INVARIANTS,
    };
    const MERGED_POLICY: McpPolicy = McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: MERGED_SECOND_BINDING,
        invariants: TEST_DOMAIN_INVARIANTS,
    };
    const STATS_POLICY: McpPolicy = McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: STATS_BINDING,
        invariants: TEST_DOMAIN_INVARIANTS,
    };
    const HOST_ADMIN_POLICY: McpPolicy = McpPolicy {
        exposure: McpExposure::HostAdmin,
        tool_bindings: &[],
        invariants: TEST_HOST_ADMIN_INVARIANTS,
    };
    const TEST_DOMAIN_FIRST: OperationDeclaration = OperationDeclaration::new(
        "api.example-a",
        ContractSurface::Api,
        None,
        None,
        "example-a",
        "example-a",
        MigrationState::Adopted,
        &[],
    )
    .with_mcp_policy(DOMAIN_POLICY);
    const TEST_DOMAIN_SECOND: OperationDeclaration = OperationDeclaration::new(
        "api.example-b",
        ContractSurface::Api,
        None,
        None,
        "example-b",
        "example-b",
        MigrationState::Adopted,
        &[],
    )
    .with_mcp_policy(MERGED_POLICY);
    const TEST_STATS: OperationDeclaration = OperationDeclaration::new(
        "api.get-stats",
        ContractSurface::Api,
        None,
        None,
        "stats",
        "stats",
        MigrationState::Adopted,
        &[],
    )
    .with_mcp_policy(STATS_POLICY);
    const TEST_HOST_ADMIN: OperationDeclaration = OperationDeclaration::new(
        "api.example-admin",
        ContractSurface::Api,
        None,
        None,
        "example-admin",
        "example-admin",
        MigrationState::Adopted,
        &[],
    )
    .with_mcp_policy(HOST_ADMIN_POLICY);
    const TEST_HOST_ADMIN_COLLISION: OperationDeclaration = OperationDeclaration::new(
        "api.example-a",
        ContractSurface::Api,
        None,
        None,
        "example-a-admin",
        "example-a-admin",
        MigrationState::Adopted,
        &[],
    )
    .with_mcp_policy(HOST_ADMIN_POLICY);

    #[test]
    fn policy_projection_merges_tools_and_deduplicates_in_source_order() {
        let projection = project_mcp_policy(&[
            TEST_DOMAIN_FIRST,
            TEST_DOMAIN_SECOND,
            TEST_STATS,
            TEST_HOST_ADMIN,
        ])
        .expect("声明 policy 应能投影");

        assert_eq!(
            projection
                .tool_bindings()
                .iter()
                .map(|binding| binding.tool_name)
                .collect::<Vec<_>>(),
            ["merged", "stats"]
        );
        assert_eq!(
            projection.tool_bindings()[0].http_operations,
            ["api.example-a", "api.shared", "api.example-b"]
        );
        assert_eq!(
            projection.domain_operations,
            [
                "api.example-a",
                "api.shared",
                "api.example-b",
                "api.get-stats"
            ]
        );
        assert_eq!(projection.host_admin_operations, ["api.example-admin"]);
        assert_eq!(
            projection.invariants(McpOperationClass::Domain),
            TEST_DOMAIN_INVARIANTS
        );
        assert_eq!(
            projection.invariants(McpOperationClass::HostAdmin),
            TEST_HOST_ADMIN_INVARIANTS
        );
        assert_eq!(
            projection.tool_bindings()[1].class,
            McpOperationClass::Domain
        );
        assert_eq!(projection.tool_bindings()[1].tool_name, "stats");
    }

    #[test]
    fn policy_projection_rejects_cross_boundary_and_policy_conflicts() {
        const HOST_TOOL_BINDING: &[McpToolBinding] = &[McpToolBinding {
            tool_name: "merged",
            http_operations: &["api.example-admin"],
        }];
        const HOST_TOOL_POLICY: McpPolicy = McpPolicy {
            exposure: McpExposure::HostAdmin,
            tool_bindings: HOST_TOOL_BINDING,
            invariants: TEST_HOST_ADMIN_INVARIANTS,
        };
        const HOST_TOOL: OperationDeclaration = OperationDeclaration::new(
            "api.example-admin-tool",
            ContractSurface::Api,
            None,
            None,
            "example-admin-tool",
            "example-admin-tool",
            MigrationState::Adopted,
            &[],
        )
        .with_mcp_policy(HOST_TOOL_POLICY);

        assert!(matches!(
            project_mcp_policy(&[TEST_DOMAIN_FIRST, HOST_TOOL]),
            Err(McpProjectionError::ToolExposureConflict {
                tool_name: "merged",
                ..
            })
        ));

        const OTHER_DOMAIN_INVARIANTS: &[McpOperationInvariant] =
            &[McpOperationInvariant::CanonicalHostOnly];
        const OTHER_DOMAIN_POLICY: McpPolicy = McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: MERGED_SECOND_BINDING,
            invariants: OTHER_DOMAIN_INVARIANTS,
        };
        const OTHER_DOMAIN: OperationDeclaration = OperationDeclaration::new(
            "api.example-c",
            ContractSurface::Api,
            None,
            None,
            "example-c",
            "example-c",
            MigrationState::Adopted,
            &[],
        )
        .with_mcp_policy(OTHER_DOMAIN_POLICY);
        assert!(matches!(
            project_mcp_policy(&[TEST_DOMAIN_FIRST, OTHER_DOMAIN]),
            Err(McpProjectionError::ToolInvariantConflict {
                tool_name: "merged"
            })
        ));

        assert!(matches!(
            project_mcp_policy(&[TEST_DOMAIN_FIRST, TEST_HOST_ADMIN_COLLISION]),
            Err(McpProjectionError::OperationExposureConflict {
                operation_id: "api.example-a"
            })
        ));

        const EMPTY_TOOL_BINDING: &[McpToolBinding] = &[McpToolBinding {
            tool_name: "empty",
            http_operations: &[],
        }];
        const EMPTY_TOOL_POLICY: McpPolicy = McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: EMPTY_TOOL_BINDING,
            invariants: TEST_DOMAIN_INVARIANTS,
        };
        const EMPTY_TOOL: OperationDeclaration = OperationDeclaration::new(
            "api.empty",
            ContractSurface::Api,
            None,
            None,
            "empty",
            "empty",
            MigrationState::Adopted,
            &[],
        )
        .with_mcp_policy(EMPTY_TOOL_POLICY);
        assert!(matches!(
            project_mcp_policy(&[EMPTY_TOOL]),
            Err(McpProjectionError::EmptyToolOperations {
                operation_id: "api.empty",
                tool_name: "empty"
            })
        ));
    }

    #[test]
    fn policy_projection_consumes_current_declaration_source_without_static_inventory() {
        let projection = project_mcp_policy(crate::operation_catalog())
            .expect("当前 declaration family 应可投影");
        assert!(!projection.tool_bindings().is_empty());
        assert!(!projection.domain_operations.is_empty());
        assert!(projection
            .tool_bindings()
            .windows(2)
            .all(|window| window[0].tool_name != window[1].tool_name));
    }
}
