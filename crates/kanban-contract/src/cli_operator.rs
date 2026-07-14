//! CLI signal、hook、dispatch、export 与 import machine contracts。
//!
//! 这些 DTO 只描述公开 machine output；service、状态机与 JSONL record body 仍由原 owner 管理。
//! 每个 surface 仍由中央 inventory 分别注册 exact root 与 adoption witnesses。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{ApiComment, DataEnvelope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliSignalObservation {
    pub id: String,
    pub board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub task_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub task_ref_snapshot: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub run_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub comment_id: Option<String>,
    pub actor: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub agent_type: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub source: Option<String>,
    pub evidence: crate::structured_metadata::SignalEvidence,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliSignalStatus {
    Open,
    Confirmed,
    Rejected,
    Superseded,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliSignal {
    pub id: String,
    pub board_id: String,
    pub observation_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub status: CliSignalStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub dedupe_key: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub superseded_by_signal_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub reviewed_by: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i64_schema")
    )]
    pub reviewed_at: Option<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub review_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub observation: CliSignalObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliSignalRecordResult {
    pub signal: CliSignal,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_api_comment_schema")
    )]
    pub backlink_comment: Option<ApiComment>,
}

pub type CliSignalRecordOutput = DataEnvelope<CliSignalRecordResult>;
pub type CliSignalListOutput = DataEnvelope<Vec<CliSignal>>;
pub type CliSignalShowOutput = DataEnvelope<CliSignal>;
pub type CliSignalReviewOutput = DataEnvelope<Vec<CliSignal>>;
pub type CliSignalConfirmOutput = DataEnvelope<Vec<CliSignal>>;
pub type CliSignalRejectOutput = DataEnvelope<Vec<CliSignal>>;
pub type CliSignalResolveOutput = DataEnvelope<Vec<CliSignal>>;
pub type CliSignalSupersedeOutput = DataEnvelope<Vec<CliSignal>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliHookPromptBindings {
    pub failure: String,
    pub task_create: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliHookPromptConfigStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub valid: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub error: Option<String>,
    pub bindings: CliHookPromptBindings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliHookCodexInstallResult {
    pub path: PathBuf,
    pub installed: bool,
    pub matcher: String,
    pub handler_commands: Vec<String>,
    pub managed_hook_count: usize,
    pub prompt_config_created: bool,
    pub prompt_config: CliHookPromptConfigStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliHookCodexStatusResult {
    pub path: PathBuf,
    pub installed: bool,
    pub matcher: String,
    pub managed_hook_count: usize,
    pub post_tool_use_group_count: usize,
    pub managed_commands: Vec<String>,
    pub prompt_config: CliHookPromptConfigStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliHookCodexUninstallResult {
    pub path: PathBuf,
    pub removed_hook_count: usize,
    pub installed: bool,
}

pub type CliHookCodexInstallOutput = DataEnvelope<CliHookCodexInstallResult>;
pub type CliHookCodexStatusOutput = DataEnvelope<CliHookCodexStatusResult>;
pub type CliHookCodexUninstallOutput = DataEnvelope<CliHookCodexUninstallResult>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDispatchRunResult {
    pub claimed: usize,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub task_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_string_schema")
    )]
    pub run_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "required_nullable_i32_schema")
    )]
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliDispatchLoopResult {
    pub iterations: usize,
    pub claimed: usize,
    pub runs: Vec<CliDispatchRunResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<CliDispatchStopReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CliDispatchStopReason {
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum CliDispatchResult {
    Once(CliDispatchRunResult),
    Loop(CliDispatchLoopResult),
}

pub type CliDispatchOutput = DataEnvelope<CliDispatchResult>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliExportResult {
    pub out_path: PathBuf,
    pub records: usize,
}

pub type CliExportOutput = DataEnvelope<CliExportResult>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CliImportResult {
    pub input_path: PathBuf,
    pub records: usize,
    pub dry_run: bool,
}

pub type CliImportOutput = DataEnvelope<CliImportResult>;

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[cfg(feature = "schema")]
fn required_nullable_i64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i64>>()
}

#[cfg(feature = "schema")]
fn required_nullable_i32_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i32>>()
}

#[cfg(feature = "schema")]
fn required_nullable_api_comment_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<ApiComment>>()
}
