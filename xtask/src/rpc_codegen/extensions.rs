//! 扩展 RPC 直接以正式 proto 为源，生成统一 catalog 消费的清单。
use std::{fs, path::Path};

use crate::ToolResult;

pub(super) fn run(root: &Path, check: bool) -> ToolResult<()> {
    let owner = root.join("crates/kanban-protocol");
    let source = fs::read_to_string(owner.join("proto/kanban/extensions/v1/workspace.proto"))?;
    let mut service = None;
    let mut methods = Vec::new();
    for line in source.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("service ") {
            service = Some(name.trim_end_matches(" {").to_owned());
        } else if line == "}" {
            service = None;
        } else if let Some(declaration) = line.strip_prefix("rpc ") {
            let (method, tail) = declaration.split_once('(').ok_or("RPC 缺少输入类型")?;
            let (request, tail) = tail.split_once(") returns (").ok_or("RPC 缺少输出类型")?;
            let output = tail.strip_suffix(");").ok_or("RPC 声明未结束")?;
            let (response, streaming) = output
                .strip_prefix("stream ")
                .map_or((output, false), |v| (v, true));
            let unqualified = |value: &str| value.rsplit('.').next().unwrap_or(value).to_owned();
            methods.push(kanban_protocol::rpc::catalog::RpcMethodDescriptor {
                operation_id: format!(
                    "rpc.extension.{}",
                    super::model::snake(method).replace('_', "-")
                ),
                service: format!(
                    "kanban.extensions.v1.{}",
                    service.as_ref().ok_or("RPC 不属于 service")?
                ),
                method: method.into(),
                request: unqualified(request),
                response: unqualified(response),
                server_streaming: streaming,
            });
        }
    }
    if methods.is_empty() {
        return Err("对象/文件 RPC 清单为空".into());
    }
    super::write(
        &owner.join("proto/rpc-extensions.json"),
        &(serde_json::to_string_pretty(&methods)? + "\n"),
        check,
    )
}
