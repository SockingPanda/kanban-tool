//! `/app/runtime.json` 的 browser-first host metadata wire contract。
//!
//! 该 DTO 描述同源 Web host 装配的运行时元数据，不是 `/api/v1` operation。它仍由
//! `kanban-protocol` 作为唯一 wire 事实源提供，以便浏览器与 Tauri 共享同一份生成类型。

use serde::{Deserialize, Serialize};

/// 浏览器与 Tauri 加载 `/app/runtime.json` 时消费的 host metadata。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebRuntimeConfig {
    pub api_base_url: String,
    pub web_base_path: String,
    pub actor: String,
    pub default_board: String,
    pub server_version: String,
    pub protocol_version: String,
    pub web_build_id: String,
}
