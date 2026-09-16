//! 从已有 router/schema 和 protocol declaration 投影工具目录，避免维护第二份全集。

use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    hash::Hasher,
};

use anyhow::ensure;
use kanban_protocol::{
    HttpMethod, McpOperationClass, operation_catalog, project_mcp_policy,
    validate_mcp_policy_projection,
};
use rmcp::model::{Tool, ToolAnnotations};

use crate::{
    bounded::json_size,
    config::{Config, Profile, valid_tool_name},
    metadata::PROTOCOL,
    pagination::ENVELOPE_RESERVE,
};

#[derive(Clone)]
pub(crate) struct ToolPolicy {
    pub(crate) tools: Vec<Tool>,
    pub(crate) stamp: String,
    allowed: BTreeSet<String>,
    read_only: BTreeSet<String>,
}

impl ToolPolicy {
    pub(crate) fn build(
        config: &Config,
        mut tools: Vec<Tool>,
        work: BTreeSet<String>,
    ) -> anyhow::Result<Self> {
        let projection = project_mcp_policy(operation_catalog())?;
        validate_mcp_policy_projection(&projection).map_err(anyhow::Error::msg)?;
        let declared: BTreeSet<_> = projection
            .tool_bindings()
            .iter()
            .map(|binding| binding.tool_name.to_owned())
            .collect();
        let actual: BTreeSet<_> = tools.iter().map(|tool| tool.name.to_string()).collect();
        ensure!(
            declared == actual && tools.len() == actual.len(),
            "MCP router 与 protocol catalog 不一致"
        );
        ensure!(work.is_subset(&actual), "work profile 含未注册工具");
        ensure!(
            config.disabled_tools.is_subset(&actual),
            "disabled_tools 含未注册工具，请使用 --inspect 核对名称"
        );

        // 旧字段名仍叫 http_operations；这里消费的是契约语义，业务传输仍为 gRPC。
        // 未证明为 GET 的 operation 保守视为可写，包括 POST query。
        let read_operations: BTreeMap<_, _> = operation_catalog()
            .iter()
            .map(|operation| {
                (
                    operation.operation_id,
                    matches!(operation.method, Some(HttpMethod::Get)),
                )
            })
            .collect();
        let mut read_only = BTreeSet::new();
        for binding in projection.tool_bindings() {
            ensure!(
                binding.class == McpOperationClass::Domain,
                "MCP 不能暴露 host-admin operation"
            );
            if !binding.http_operations.is_empty()
                && binding
                    .http_operations
                    .iter()
                    .all(|operation| read_operations.get(operation).copied().unwrap_or(false))
            {
                read_only.insert(binding.tool_name.to_owned());
            }
        }
        tools.retain(|tool| {
            let name = tool.name.as_ref();
            !config.disabled_tools.contains(name)
                && match config.profile {
                    Profile::All => true,
                    Profile::Work => work.contains(name),
                    Profile::ReadOnly => read_only.contains(name),
                }
        });
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        for tool in &mut tools {
            ensure!(valid_tool_name(&tool.name), "MCP 工具名不符合规范");
            let read = read_only.contains(tool.name.as_ref());
            // annotations 只给客户端提示；真正的执行限制见 allowed/read_only。
            tool.annotations = Some(
                ToolAnnotations::new()
                    .read_only(read)
                    .destructive(!read)
                    .idempotent(read),
            );
            ensure!(
                json_size(tool, config.limits.max_result_bytes - ENVELOPE_RESERVE)?.is_some(),
                "单个 MCP 工具 schema 超过配置的响应限额：{}",
                tool.name
            );
        }
        let allowed = tools.iter().map(|tool| tool.name.to_string()).collect();
        // 游标陈旧检测，不用于认证、完整性保护或访问控制。
        let mut hasher = DefaultHasher::new();
        hasher.write(PROTOCOL.as_bytes());
        hasher.write(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.write(&serde_json::to_vec(&(config, &tools))?);
        let stamp = format!("{:016x}", hasher.finish());
        Ok(Self {
            tools,
            stamp,
            allowed,
            read_only,
        })
    }

    pub(crate) fn allows(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }

    pub(crate) fn is_read_only(&self, name: &str) -> bool {
        self.read_only.contains(name)
    }

    pub(crate) fn allows_resource_tool(&self, name: &str) -> bool {
        self.allows(name) && self.is_read_only(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::KanbanMcp;

    fn policy(config: &Config) -> anyhow::Result<ToolPolicy> {
        ToolPolicy::build(
            config,
            KanbanMcp::tool_router().list_all(),
            KanbanMcp::work_tool_router()
                .list_all()
                .into_iter()
                .map(|tool| tool.name.to_string())
                .collect(),
        )
    }

    #[test]
    fn read_only_profile_excludes_mutations() {
        let policy = policy(&Config {
            profile: Profile::ReadOnly,
            ..Config::default()
        })
        .unwrap();
        assert!(policy.allows_resource_tool("board_show"));
        assert!(policy.allows_resource_tool("task_show"));
        assert!(!policy.allows("task_create"));
        assert!(!policy.allows("attachment_create"));
        assert!(
            policy
                .tools
                .iter()
                .all(|tool| policy.is_read_only(&tool.name))
        );
    }

    #[test]
    fn disabled_tools_affect_the_execution_guard() {
        let config = Config {
            disabled_tools: BTreeSet::from(["task_show".into()]),
            ..Config::default()
        };
        let policy = policy(&config).unwrap();
        assert!(!policy.allows("task_show"));
        assert!(!policy.allows_resource_tool("task_show"));
        assert!(policy.tools.iter().all(|tool| tool.name != "task_show"));
    }

    #[test]
    fn misspelled_tool_configuration_fails_closed() {
        let config = Config {
            disabled_tools: BTreeSet::from(["task_shwo".into()]),
            ..Config::default()
        };
        assert!(policy(&config).is_err());
    }

    #[test]
    fn unchanged_configuration_has_stable_order_and_cursor_stamp() {
        let config = Config::default();
        let left = policy(&config).unwrap();
        let right = policy(&config).unwrap();
        assert_eq!(left.stamp, right.stamp);
        assert!(
            left.tools
                .windows(2)
                .all(|pair| pair[0].name < pair[1].name)
        );
        let changed = policy(&Config {
            profile: Profile::All,
            ..config
        })
        .unwrap();
        assert_ne!(left.stamp, changed.stamp);
    }
}
