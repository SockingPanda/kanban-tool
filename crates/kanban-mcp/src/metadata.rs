//! MCP envelope 元数据；不混入领域 DTO。

use rmcp::model::{Implementation, MetaObject, ProtocolVersion};
use serde_json::json;

pub(crate) const PROTOCOL: &str = "2026-07-28";
pub(crate) static SUPPORTED: [ProtocolVersion; 1] = [ProtocolVersion::V_2026_07_28];
pub(crate) const ERROR_KEY: &str = "io.github.sockingpanda.kanban-tool/error";
pub(crate) const LIST_TTL_MS: u64 = 60_000;

pub(crate) const INSTRUCTIONS: &str = include_str!("../docs/instructions.md");

pub(crate) fn implementation() -> Implementation {
    Implementation::new("kanban-mcp", env!("CARGO_PKG_VERSION"))
}

pub(crate) fn response_meta() -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert(
        "io.modelcontextprotocol/serverInfo".to_owned(),
        json!({
            "name": "kanban-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }),
    );
    meta
}
