//! Browser-first Web contract 选择与生成。
//!
//! Web selection 是 canonical `kanban-protocol` catalog 的小型、可审计 projection。
//! selection 文件只包含 operation/contract ID；transport metadata 与 schema 继续由
//! protocol crate 持有。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::OpenOptions,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use fs4::fs_std::FileExt;
use kanban_protocol::{
    ContractDirection, ContractStrictness, ContractSurface, EndpointDescriptor, EndpointObligation,
    HttpMethod, OperationContract, SSE_HEARTBEAT_EVENT, STREAM_EVENT_ENVELOPE_FIELDS,
    TASK_SCOPED_EVENT_KINDS, endpoint_catalog, operation_inventory,
    schema::{DRAFT_2020_12, SchemaRoot, canonicalize, schema_document, schema_registry},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ToolResult, audit_inventory};

pub const SELECTION_RELATIVE_PATH: &str = "apps/web/web-contracts.json";
pub const OUTPUT_RELATIVE_PATH: &str = "apps/web/src/lib/api/generated";
const GENERATOR_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SelectionFile {
    version: u32,
    operations: Vec<String>,
    extra_contracts: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResolvedSelection {
    selection: SelectionFile,
    endpoints: Vec<EndpointDescriptor>,
    contracts: Vec<OperationContract>,
    roots: Vec<SchemaRoot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationManifest {
    id: String,
    method: String,
    path: String,
    obligations: OperationObligations,
    shared_components: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct OperationObligations {
    path: WebObligation,
    query: WebObligation,
    headers: WebObligation,
    body: WebObligation,
    success: WebObligation,
    sse: WebObligation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WebObligation {
    Contract {
        #[serde(rename = "contractId")]
        contract_id: String,
    },
    NotApplicable,
    Excluded {
        reason: String,
    },
    Todo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractManifest {
    id: String,
    surface: ContractSurface,
    direction: ContractDirection,
    strictness: ContractStrictness,
    schema_id: Option<String>,
    schema_path: Option<String>,
    valid_fixture: Option<String>,
    invalid_fixture: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceHashes {
    selection: String,
    protocol_operations: String,
    protocol_endpoints: String,
    protocol_schemas: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebManifest {
    generator_version: u32,
    schema_dialect: &'static str,
    numeric_policy: &'static str,
    selection_path: &'static str,
    source_hashes: SourceHashes,
    operations: Vec<OperationManifest>,
    contracts: Vec<ContractManifest>,
    generated_files: Vec<String>,
}

/// 分发 `xtask web-contracts generate|check`。
pub fn run(repo_root: &Path, command: &str) -> ToolResult<()> {
    match command {
        "generate" => generate(repo_root),
        "check" => check(repo_root),
        other => Err(failure(format!("未知 web-contracts command: {other}"))),
    }
}

/// 可恢复地生成精确的 Web output directory。
pub fn generate(repo_root: &Path) -> ToolResult<()> {
    let generated = expected_files(repo_root)?;
    let output = repo_relative_path(
        repo_root,
        OUTPUT_RELATIVE_PATH,
        RepoPathKind::Directory,
        true,
    )?;
    recoverable_replace_directory(&output, &generated)?;
    Ok(())
}

/// 在内存重新生成并逐字节比较，不写入任何文件。
pub fn check(repo_root: &Path) -> ToolResult<()> {
    let output = repo_relative_path(
        repo_root,
        OUTPUT_RELATIVE_PATH,
        RepoPathKind::Directory,
        true,
    )?;
    let parent = output.parent().ok_or_else(|| {
        failure(format!(
            "Web output path has no parent: {}",
            output.display()
        ))
    })?;
    let _lock = acquire_replace_lock(parent, ReplaceLockMode::Shared, false)?;
    let generated = expected_files(repo_root)?;
    ensure_replace_state_clean(&output)?;
    compare_output_directory(&output, &generated)
}

/// 返回确定性的 generated file set，供窄 tooling tests 使用。
pub fn expected_files(repo_root: &Path) -> ToolResult<BTreeMap<String, Vec<u8>>> {
    let resolved = resolve_selection(repo_root)?;
    let selection_bytes = canonical_json_bytes(&resolved.selection)?;
    let endpoint_bytes = canonical_json_bytes(&resolved.endpoints)?;

    let mut files = BTreeMap::new();
    let mut contract_modules = Vec::new();
    let mut schema_hash_material = Vec::new();
    let mut contract_manifests = Vec::new();

    for contract in &resolved.contracts {
        let (schema_path, valid_path, invalid_path) = match contract.schema_id {
            Some(schema_id) => {
                let root = resolved
                    .roots
                    .iter()
                    .find(|root| root.id == schema_id)
                    .ok_or_else(|| {
                        failure(format!(
                            "selected contract missing schema root: {schema_id}"
                        ))
                    })?;
                let slug = safe_slug(contract.id);
                let schema_path = format!("schemas/{slug}.schema.json");
                let valid_path = format!("fixtures/{slug}.valid.json");
                let invalid_path = format!("fixtures/{slug}.invalid.json");
                let schema = schema_document(root);
                let schema_bytes = pretty_json_bytes(&schema)?;
                let valid_fixture_path =
                    repo_relative_path(repo_root, root.valid_fixture, RepoPathKind::File, false)?;
                let valid_bytes = fs::read(&valid_fixture_path).map_err(|error| {
                    failure(format!(
                        "无法读取 Web fixture {}: {error}",
                        root.valid_fixture
                    ))
                })?;
                let invalid_fixture_path =
                    repo_relative_path(repo_root, root.invalid_fixture, RepoPathKind::File, false)?;
                let invalid_bytes = fs::read(&invalid_fixture_path).map_err(|error| {
                    failure(format!(
                        "无法读取 Web fixture {}: {error}",
                        root.invalid_fixture
                    ))
                })?;
                insert_generated_file(
                    &mut files,
                    schema_path.clone(),
                    schema_bytes.clone(),
                    "schema",
                )?;
                insert_generated_file(
                    &mut files,
                    valid_path.clone(),
                    valid_bytes.clone(),
                    "valid fixture",
                )?;
                insert_generated_file(
                    &mut files,
                    invalid_path.clone(),
                    invalid_bytes.clone(),
                    "invalid fixture",
                )?;
                schema_hash_material.extend_from_slice(contract.id.as_bytes());
                schema_hash_material.extend_from_slice(&schema_bytes);
                schema_hash_material.extend_from_slice(&valid_bytes);
                schema_hash_material.extend_from_slice(&invalid_bytes);

                let const_name = schema_const_name(contract.id);
                let slug = safe_slug(contract.id);
                let module_path = format!("contracts/{slug}.ts");
                insert_generated_file(
                    &mut files,
                    module_path,
                    contract_module(contract.id, &schema)?,
                    "contract module",
                )?;
                contract_modules.push(ContractModule {
                    id: contract.id.to_owned(),
                    const_name,
                    validator_name: validator_name(contract.id),
                    parser_name: format!("parse{}", pascal_identifier(contract.id)),
                });

                (Some(schema_path), Some(valid_path), Some(invalid_path))
            }
            None => (None, None, None),
        };

        contract_manifests.push(ContractManifest {
            id: contract.id.to_owned(),
            surface: contract.surface,
            direction: contract.direction,
            strictness: contract.strictness,
            schema_id: contract.schema_id.map(str::to_owned),
            schema_path,
            valid_fixture: valid_path,
            invalid_fixture: invalid_path,
        });
    }

    let operations = resolved
        .endpoints
        .iter()
        .map(|endpoint| OperationManifest {
            id: endpoint.operation_id.to_owned(),
            method: http_method_name(endpoint.method).to_owned(),
            path: endpoint.path.to_owned(),
            obligations: operation_obligations(endpoint),
            shared_components: endpoint
                .shared_components
                .iter()
                .map(|id| (*id).to_owned())
                .collect(),
        })
        .collect::<Vec<_>>();

    let protocol_schemas_hash = sha256(&schema_hash_material);
    let source_hashes = SourceHashes {
        selection: sha256(&selection_bytes),
        protocol_operations: sha256(&canonical_json_bytes(operation_inventory())?),
        protocol_endpoints: sha256(&endpoint_bytes),
        protocol_schemas: protocol_schemas_hash,
    };
    let generated_names = generated_source_names(&contract_manifests);
    let manifest = WebManifest {
        generator_version: GENERATOR_VERSION,
        schema_dialect: DRAFT_2020_12,
        numeric_policy: "reject_unsafe_json_numbers",
        selection_path: SELECTION_RELATIVE_PATH,
        source_hashes,
        operations: operations.clone(),
        contracts: contract_manifests.clone(),
        generated_files: generated_names,
    };

    let manifest_bytes = pretty_json_bytes(&canonicalize(serde_json::to_value(&manifest)?))?;
    insert_generated_file(
        &mut files,
        "manifest.json".to_owned(),
        manifest_bytes,
        "manifest",
    )?;
    insert_generated_file(
        &mut files,
        "operations.json".to_owned(),
        pretty_json_bytes(&canonicalize(serde_json::to_value(&operations)?))?,
        "operations manifest",
    )?;
    insert_generated_file(
        &mut files,
        "contracts.json".to_owned(),
        pretty_json_bytes(&canonicalize(serde_json::to_value(&contract_manifests)?))?,
        "contracts manifest",
    )?;
    insert_generated_file(
        &mut files,
        "runtime.ts".to_owned(),
        runtime_module(),
        "runtime module",
    )?;
    insert_generated_file(
        &mut files,
        "test-only.ts".to_owned(),
        test_only_module(&contract_modules),
        "test-only module",
    )?;
    insert_generated_file(
        &mut files,
        "operations.ts".to_owned(),
        operations_module(&operations),
        "operations module",
    )?;
    insert_generated_file(
        &mut files,
        "sse.ts".to_owned(),
        sse_module(&resolved)?,
        "SSE module",
    )?;
    insert_generated_file(
        &mut files,
        "index.ts".to_owned(),
        index_module(),
        "index module",
    )?;

    let hash_file = generated_hashes(&files);
    files.insert("generated.sha256".to_owned(), hash_file);
    Ok(files)
}

fn resolve_selection(repo_root: &Path) -> ToolResult<ResolvedSelection> {
    audit_inventory()?;
    let path = repo_relative_path(
        repo_root,
        SELECTION_RELATIVE_PATH,
        RepoPathKind::File,
        false,
    )?;
    let bytes = fs::read(&path).map_err(|error| {
        failure(format!(
            "无法读取 Web contract selection {}: {error}",
            path.display()
        ))
    })?;
    let selection: SelectionFile = serde_json::from_slice(&bytes)
        .map_err(|error| failure(format!("Web contract selection 不是合法 JSON: {error}")))?;
    if selection.version != 1 {
        return Err(failure(format!(
            "不支持的 Web contract selection version: {}",
            selection.version
        )));
    }

    let mut operation_ids = BTreeSet::new();
    for id in &selection.operations {
        if id.trim().is_empty() || !operation_ids.insert(id.clone()) {
            return Err(failure(format!(
                "Web contract selection 存在空值或重复 operation: {id}"
            )));
        }
    }
    let mut extra_ids = BTreeSet::new();
    for id in &selection.extra_contracts {
        if id.trim().is_empty() || !extra_ids.insert(id.clone()) {
            return Err(failure(format!(
                "Web contract selection 存在空值或重复 extra contract: {id}"
            )));
        }
    }

    let endpoints_by_id = endpoint_catalog()
        .iter()
        .map(|endpoint| (endpoint.operation_id, endpoint))
        .collect::<BTreeMap<_, _>>();
    let contracts_by_id = operation_inventory()
        .iter()
        .map(|contract| (contract.id, contract))
        .collect::<BTreeMap<_, _>>();

    for id in &operation_ids {
        let endpoint = endpoints_by_id
            .get(id.as_str())
            .ok_or_else(|| failure(format!("Web contract selection unknown operation: {id}")))?;
        if endpoint.exclusion.is_some() {
            return Err(failure(format!(
                "Web contract selection cannot select excluded operation: {id}"
            )));
        }
        if [
            endpoint.obligations.path,
            endpoint.obligations.query,
            endpoint.obligations.headers,
            endpoint.obligations.body,
            endpoint.obligations.success,
            endpoint.obligations.sse,
        ]
        .into_iter()
        .any(|obligation| matches!(obligation, EndpointObligation::Todo))
        {
            return Err(failure(format!(
                "Web contract selection cannot select operation with TODO obligation: {id}"
            )));
        }
    }

    let mut contract_ids = BTreeSet::new();
    for id in &operation_ids {
        let endpoint = endpoints_by_id[id.as_str()];
        for contract_id in endpoint_contract_ids(endpoint) {
            contract_ids.insert(contract_id.to_owned());
        }
    }
    for id in &extra_ids {
        if !contracts_by_id.contains_key(id.as_str()) {
            return Err(failure(format!(
                "Web contract selection unknown extra contract: {id}"
            )));
        }
        contract_ids.insert(id.clone());
    }

    let endpoints = endpoint_catalog()
        .iter()
        .filter(|endpoint| operation_ids.contains(endpoint.operation_id))
        .copied()
        .collect::<Vec<_>>();
    let contracts = operation_inventory()
        .iter()
        .filter(|contract| contract_ids.contains(contract.id))
        .copied()
        .collect::<Vec<_>>();
    let roots = schema_registry()
        .iter()
        .filter(|root| {
            contracts
                .iter()
                .any(|contract| contract.schema_id == Some(root.id))
        })
        .copied()
        .collect::<Vec<_>>();

    for contract in &contracts {
        if contract.schema_id.is_some()
            && !roots.iter().any(|root| Some(root.id) == contract.schema_id)
        {
            return Err(failure(format!(
                "selected contract has no schema root: {}",
                contract.id
            )));
        }
    }

    Ok(ResolvedSelection {
        selection,
        endpoints,
        contracts,
        roots,
    })
}

fn endpoint_contract_ids(endpoint: &EndpointDescriptor) -> Vec<&'static str> {
    let mut ids = Vec::new();
    for obligation in [
        endpoint.obligations.path,
        endpoint.obligations.query,
        endpoint.obligations.headers,
        endpoint.obligations.body,
        endpoint.obligations.success,
        endpoint.obligations.sse,
    ] {
        if let EndpointObligation::Contract(id) = obligation
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    for id in endpoint.shared_components {
        if !ids.contains(id) {
            ids.push(id);
        }
    }
    ids
}

fn operation_obligations(endpoint: &EndpointDescriptor) -> OperationObligations {
    OperationObligations {
        path: web_obligation(endpoint.obligations.path),
        query: web_obligation(endpoint.obligations.query),
        headers: web_obligation(endpoint.obligations.headers),
        body: web_obligation(endpoint.obligations.body),
        success: web_obligation(endpoint.obligations.success),
        sse: web_obligation(endpoint.obligations.sse),
    }
}

fn web_obligation(obligation: EndpointObligation) -> WebObligation {
    match obligation {
        EndpointObligation::Contract(id) => WebObligation::Contract {
            contract_id: id.to_owned(),
        },
        EndpointObligation::NotApplicable => WebObligation::NotApplicable,
        EndpointObligation::Excluded { reason } => WebObligation::Excluded {
            reason: reason.to_owned(),
        },
        EndpointObligation::Todo => WebObligation::Todo,
    }
}

fn generated_source_names(contracts: &[ContractManifest]) -> Vec<String> {
    let mut names = vec![
        "contracts.json".to_owned(),
        "generated.sha256".to_owned(),
        "index.ts".to_owned(),
        "manifest.json".to_owned(),
        "operations.json".to_owned(),
        "operations.ts".to_owned(),
        "runtime.ts".to_owned(),
        "sse.ts".to_owned(),
        "test-only.ts".to_owned(),
    ];
    for contract in contracts {
        if contract.schema_id.is_some() {
            names.push(format!("contracts/{}.ts", safe_slug(&contract.id)));
        }
        if let Some(path) = &contract.schema_path {
            names.push(path.clone());
        }
        if let Some(path) = &contract.valid_fixture {
            names.push(path.clone());
        }
        if let Some(path) = &contract.invalid_fixture {
            names.push(path.clone());
        }
    }
    names.sort();
    names
}

#[derive(Debug, Clone)]
struct ContractModule {
    id: String,
    const_name: String,
    validator_name: String,
    parser_name: String,
}

fn insert_generated_file(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    bytes: Vec<u8>,
    kind: &str,
) -> ToolResult<()> {
    if files.insert(path.clone(), bytes).is_some() {
        return Err(failure(format!(
            "selected contracts produce duplicate {kind} path: {path}"
        )));
    }
    Ok(())
}

fn contract_module(contract_id: &str, schema: &Value) -> ToolResult<Vec<u8>> {
    let const_name = schema_const_name(contract_id);
    let type_name = schema_type_name(contract_id);
    let validator = validator_name(contract_id);
    let parser = format!("parse{}", pascal_identifier(contract_id));
    let schema_text = json_to_typescript(schema)?;
    let mut output = String::from("// 由 `xtask web-contracts generate` 生成；请勿手工编辑。\n");
    if contains_unsafe_json_number(schema) {
        output.push_str(
            "/* eslint-disable no-loss-of-precision -- JSON Schema 的 int64 边界有意超过 JS 安全整数范围。 */\n",
        );
    }
    output.push_str(
        "import type { FromSchema } from \"json-schema-to-ts\";\nimport { ContractValidationError, createContractValidator } from \"../runtime\";\n\n",
    );
    output.push_str(&format!(
        "export const {const_name} = {schema_text} as const;\nexport type {type_name} = FromSchema<typeof {const_name}>;\n\n"
    ));
    output.push_str(&format!(
        "export const {validator}: ReturnType<typeof createContractValidator<{type_name}>> = createContractValidator<{type_name}>(\n  {contract_id:?},\n  {const_name},\n);\n\n"
    ));
    output.push_str(&format!(
        "export function {parser}(value: unknown): {type_name} {{\n  if (!{validator}(value)) throw new ContractValidationError({contract_id:?}, {validator}.errors);\n  return value;\n}}\n"
    ));
    Ok(output.into_bytes())
}

fn runtime_module() -> Vec<u8> {
    r###"// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import Ajv2020, { type ErrorObject, type ValidateFunction } from "ajv/dist/2020";

// `strict` 保持开启；协议沿用的 int64/uint format 只在这里关闭格式校验，整数类型仍由 JSON Schema 校验。
const ajv = new Ajv2020({ allErrors: true, strict: true, validateFormats: false });
const validatorCache = new WeakMap<object, ValidateFunction>();

export type ContractValidator<T> = ((value: unknown) => value is T) & {
  readonly errors: ErrorObject[] | null | undefined;
};

export class ContractValidationError extends Error {
  readonly contractId: string;
  readonly errors: ErrorObject[] | null | undefined;

  constructor(contractId: string, errors: ErrorObject[] | null | undefined) {
    super(`Invalid ${contractId} payload`);
    this.name = "ContractValidationError";
    this.contractId = contractId;
    this.errors = errors;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function unsafeNumberPath(value: unknown, path = ""): string | null {
  if (typeof value === "number") {
    return !Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value)) ? path || "/" : null;
  }
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const unsafe = unsafeNumberPath(value[index], `${path}/${index}`);
      if (unsafe !== null) return unsafe;
    }
    return null;
  }
  if (isRecord(value)) {
    for (const [key, child] of Object.entries(value)) {
      const escaped = key.replaceAll("~", "~0").replaceAll("/", "~1");
      const unsafe = unsafeNumberPath(child, `${path}/${escaped}`);
      if (unsafe !== null) return unsafe;
    }
  }
  return null;
}

function compileValidator(id: string, schema: object): ValidateFunction {
  const cached = validatorCache.get(schema);
  if (cached) return cached;
  const validator = ajv.compile(schema);
  validatorCache.set(schema, validator);
  return validator;
}

// 数字策略 `reject_unsafe_json_numbers`：先拒绝非有限数和非安全整数，再交给 AJV。
export function createContractValidator<T>(id: string, schema: object): ContractValidator<T> {
  let compiled: ValidateFunction | undefined;
  let errors: ErrorObject[] | null | undefined;
  const validate = Object.assign(
    (value: unknown): value is T => {
      const unsafePath = unsafeNumberPath(value);
      if (unsafePath !== null) {
        errors = [{ instancePath: unsafePath, schemaPath: "#/numericPolicy", keyword: "safeNumber", params: {}, message: "number must be finite and safe" }];
        validate.errors = errors;
        return false;
      }
      compiled ??= compileValidator(id, schema);
      const valid = compiled(value);
      errors = compiled.errors;
      validate.errors = errors;
      return valid;
    },
    { errors },
  );
  return validate;
}
"###
    .as_bytes()
    .to_vec()
}

fn operations_module(operations: &[OperationManifest]) -> Vec<u8> {
    let value = serde_json::to_string_pretty(operations).expect("operation manifest serializes");
    let mut output = String::from("// 由 `xtask web-contracts generate` 生成；请勿手工编辑。\n");
    output.push_str("export const operations = ");
    output.push_str(&value);
    output.push_str(" as const;\n\n");
    output.push_str("export type WebOperation = (typeof operations)[number];\nexport type WebOperationId = WebOperation[\"id\"];\n\n");
    output.push_str("export const operationById = {\n");
    for (index, operation) in operations.iter().enumerate() {
        output.push_str(&format!("  {:?}: operations[{index}],\n", operation.id));
    }
    output.push_str("} as const;\n\n");
    output.push_str("export function getOperation<K extends WebOperationId>(id: K): (typeof operationById)[K] {\n  return operationById[id];\n}\n");
    output.into_bytes()
}

fn test_only_module(contracts: &[ContractModule]) -> Vec<u8> {
    let mut output = String::from(
        "// 由 `xtask web-contracts generate` 生成；请勿手工编辑。\n// 仅供契约 fixtures/validator 测试使用；生产入口不得导入此聚合模块。\nimport { ContractValidationError } from \"./runtime\";\n",
    );
    for contract in contracts {
        output.push_str(&format!(
            "import {{ {}, {} }} from \"./contracts/{}\";\n",
            contract.const_name,
            contract.validator_name,
            safe_slug(&contract.id),
        ));
    }
    output.push_str("\nexport { ContractValidationError };\n\nexport const schemas = {\n");
    for contract in contracts {
        output.push_str(&format!("  {:?}: {},\n", contract.id, contract.const_name));
    }
    output.push_str("} as const;\n\nexport const validators = {\n");
    for contract in contracts {
        output.push_str(&format!(
            "  {:?}: {},\n",
            contract.id, contract.validator_name
        ));
    }
    output.push_str("} as const;\n\nexport type GeneratedContractId = keyof typeof validators;\n\nexport function isGeneratedContractId(value: unknown): value is GeneratedContractId {\n  return typeof value === \"string\" && Object.prototype.hasOwnProperty.call(validators, value);\n}\n\nexport function validateContract(id: GeneratedContractId, value: unknown): boolean {\n  if (!isGeneratedContractId(id)) throw new Error(`Unknown generated contract id: ${String(id)}`);\n  return validators[id](value);\n}\n\n");
    for contract in contracts {
        output.push_str(&format!(
            "export {{ {} }} from \"./contracts/{}\";\n",
            contract.parser_name,
            safe_slug(&contract.id)
        ));
    }
    output.into_bytes()
}

fn sse_module(selection: &ResolvedSelection) -> ToolResult<Vec<u8>> {
    let root = selection
        .roots
        .iter()
        .find(|root| root.contract_id == "sse.event.data")
        .ok_or_else(|| failure("Web selection missing sse.event.data schema"))?;
    let schema = schema_document(root);
    let kinds = known_sse_event_kinds(&schema);
    let _heartbeat_root = selection
        .roots
        .iter()
        .find(|root| root.contract_id == "sse.event.heartbeat")
        .ok_or_else(|| failure("Web selection missing sse.event.heartbeat schema"))?;
    let field_order = serde_json::to_string(STREAM_EVENT_ENVELOPE_FIELDS)?;
    let task_scoped_kinds = serde_json::to_string(TASK_SCOPED_EVENT_KINDS)?;
    let mut output = String::from(
        "// 由 `xtask web-contracts generate` 生成；请勿手工编辑。\nimport type { SseEventDataContract } from \"./contracts/sse-event-data\";\nimport { sseEventDataValidator } from \"./contracts/sse-event-data\";\nimport type { SseEventHeartbeatContract } from \"./contracts/sse-event-heartbeat\";\nimport { parseSseEventHeartbeat, sseEventHeartbeatValidator } from \"./contracts/sse-event-heartbeat\";\n\n",
    );
    output.push_str(&format!(
        "export const sseHeartbeatEventName = {SSE_HEARTBEAT_EVENT:?} as const;\nexport const sseEventEnvelopeFieldOrder = {field_order} as const;\nexport const taskScopedSseEventKinds = {task_scoped_kinds} as const;\n\nexport type SseEventEnvelopeField = (typeof sseEventEnvelopeFieldOrder)[number];\nexport type TaskScopedSseEventKind = (typeof taskScopedSseEventKinds)[number];\nexport type SseHeartbeatDataContract = SseEventHeartbeatContract;\nexport const sseHeartbeatDataValidator = sseEventHeartbeatValidator;\nexport const isSseHeartbeat = sseEventHeartbeatValidator;\nexport function parseSseHeartbeat(value: unknown): SseHeartbeatDataContract {{\n  return parseSseEventHeartbeat(value);\n}}\n\n",
    ));
    output.push_str("export const knownSseEventKinds = [\n");
    for kind in &kinds {
        output.push_str(&format!("  {kind:?},\n"));
    }
    output.push_str("] as const;\n\n");
    output.push_str("export type KnownSseEventKind = (typeof knownSseEventKinds)[number];\n");
    output.push_str("export type KnownSseEvent = { [K in KnownSseEventKind]: Extract<SseEventDataContract, { kind: K }> }[KnownSseEventKind];\n\n");
    output.push_str("export interface UnknownSseEvent {\n  readonly kind: string | null;\n  readonly raw: unknown;\n  readonly envelope: Record<string, unknown> | null;\n  readonly reason: \"unknown_kind\" | \"known_payload_invalid\" | \"invalid_envelope\";\n}\n\n");
    output.push_str("export type ParsedSseEvent = KnownSseEvent | UnknownSseEvent;\n\n");
    output.push_str("export function canonicalizeSseEventEnvelope(value: SseEventDataContract): Record<string, unknown> {\n  const result: Record<string, unknown> = {};\n  for (const field of sseEventEnvelopeFieldOrder) result[field] = canonicalizeSseValue(value[field]);\n  return result;\n}\n\nexport function canonicalSseEventFingerprint(value: SseEventDataContract): string {\n  return JSON.stringify(canonicalizeSseEventEnvelope(value));\n}\n\nfunction canonicalizeSseValue(value: unknown): unknown {\n  if (Array.isArray(value)) return value.map(canonicalizeSseValue);\n  if (!isRecord(value)) return value;\n  const result: Record<string, unknown> = {};\n  for (const key of Object.keys(value).sort()) result[key] = canonicalizeSseValue(value[key]);\n  return result;\n}\n\n");
    output.push_str("function isRecord(value: unknown): value is Record<string, unknown> {\n  return typeof value === \"object\" && value !== null && !Array.isArray(value);\n}\n\n");
    output.push_str("export function parseSseEvent(value: unknown): ParsedSseEvent {\n  if (!isRecord(value)) return invalidEnvelope(value, null);\n  const kind = typeof value.kind === \"string\" ? value.kind : null;\n  if (!sseEventDataValidator(value)) {\n    return kind !== null && isKnownKind(kind)\n      ? { kind, raw: value, envelope: value, reason: \"known_payload_invalid\" }\n      : invalidEnvelope(value, value);\n  }\n  if (kind !== null && isKnownSseEvent(value)) return value;\n  if (kind !== null) return { kind, raw: value, envelope: value, reason: \"unknown_kind\" };\n  return invalidEnvelope(value, value);\n}\n\nfunction isKnownSseEvent(value: SseEventDataContract): value is KnownSseEvent {\n  return typeof value.kind === \"string\" && isKnownKind(value.kind);\n}\n\nfunction isKnownKind(value: string): value is KnownSseEventKind {\n  return knownSseEventKinds.some((kind) => kind === value);\n}\n\nfunction invalidEnvelope(raw: unknown, envelope: Record<string, unknown> | null): UnknownSseEvent {\n  return { kind: envelope && typeof envelope.kind === \"string\" ? envelope.kind : null, raw, envelope, reason: \"invalid_envelope\" };\n}\n");
    Ok(output.into_bytes())
}

fn known_sse_event_kinds(schema: &Value) -> Vec<&str> {
    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|branch| {
            branch
                .pointer("/properties/kind/const")
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>()
}

fn index_module() -> Vec<u8> {
    "// 由 `xtask web-contracts generate` 生成；请勿手工编辑。\n// 生产入口保持轻量；按需直接导入 `sse.ts` 或 `contracts/<slug>.ts`。\nexport * from \"./operations\";\n".to_owned().into_bytes()
}

fn json_to_typescript(value: &Value) -> ToolResult<String> {
    // canonicalize 后的紧凑 JSON 是确定性的；TypeScript 可直接把 JSON object 当作
    // `as const` 声明。
    Ok(serde_json::to_string(value)?)
}

fn contains_unsafe_json_number(value: &Value) -> bool {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    match value {
        Value::Number(number) => {
            number
                .as_u64()
                .is_some_and(|value| value > MAX_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value.unsigned_abs() > MAX_SAFE_INTEGER)
        }
        Value::Array(values) => values.iter().any(contains_unsafe_json_number),
        Value::Object(values) => values.values().any(contains_unsafe_json_number),
        Value::Null | Value::Bool(_) | Value::String(_) => false,
    }
}

fn generated_hashes(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut output = String::new();
    for (path, bytes) in files {
        output.push_str(&format!("{}  {path}\n", sha256(bytes)));
    }
    output.into_bytes()
}

fn compare_output_directory(target: &Path, expected: &BTreeMap<String, Vec<u8>>) -> ToolResult<()> {
    let actual = collect_files(target)?;
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    let actual_paths = actual.keys().cloned().collect::<BTreeSet<_>>();
    if expected_paths != actual_paths {
        let missing = expected_paths
            .difference(&actual_paths)
            .cloned()
            .collect::<Vec<_>>();
        let stale = actual_paths
            .difference(&expected_paths)
            .cloned()
            .collect::<Vec<_>>();
        return Err(failure(format!(
            "Web generated tree 不完整：missing={missing:?}, stale_or_orphan={stale:?}"
        )));
    }
    for (path, expected_bytes) in expected {
        let actual_bytes = actual
            .get(path)
            .expect("path sets checked before byte comparison");
        if actual_bytes != expected_bytes {
            return Err(failure(format!("Web generated artifact 已漂移: {path}")));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum RepoPathKind {
    File,
    Directory,
}

fn repo_relative_path(
    repo_root: &Path,
    relative: &str,
    kind: RepoPathKind,
    allow_missing: bool,
) -> ToolResult<PathBuf> {
    validate_directory_chain(repo_root, false)?;
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(failure(format!(
            "repo-relative path 不得为 absolute path: {relative}"
        )));
    }
    let components = relative_path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(failure(format!(
            "repo-relative path 只允许 regular path components: {relative}"
        )));
    }

    let mut current = repo_root.to_owned();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let is_final = index + 1 == components.len();
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_missing => {
                return Ok(repo_root.join(relative_path));
            }
            Err(error) => {
                return Err(failure(format!(
                    "repo-relative path 不可访问: {}: {error}",
                    current.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(failure(format!(
                "repo-relative path 不得包含 symlink: {}",
                current.display()
            )));
        }
        if !is_final && !metadata.is_dir() {
            return Err(failure(format!(
                "repo-relative path ancestor 必须是 directory: {}",
                current.display()
            )));
        }
        if is_final {
            let valid = match kind {
                RepoPathKind::File => metadata.is_file(),
                RepoPathKind::Directory => metadata.is_dir(),
            };
            if !valid {
                return Err(failure(format!(
                    "repo-relative path 类型不符合预期: {}",
                    current.display()
                )));
            }
        }
    }
    Ok(current)
}

fn validate_directory_chain(path: &Path, allow_missing: bool) -> ToolResult<()> {
    let mut current = PathBuf::new();
    let mut missing = false;
    for component in path.components() {
        current.push(component.as_os_str());
        if missing {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_missing => {
                missing = true;
                continue;
            }
            Err(error) => {
                return Err(failure(format!(
                    "directory chain 不可访问: {}: {error}",
                    current.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(failure(format!(
                "directory chain 不得包含 symlink: {}",
                current.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(failure(format!(
                "directory chain ancestor 必须是 directory: {}",
                current.display()
            )));
        }
    }
    if missing && !allow_missing {
        return Err(failure(format!(
            "directory chain 不存在: {}",
            path.display()
        )));
    }
    Ok(())
}

fn remove_owned_directory_if_present(path: &Path) -> ToolResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(failure(format!(
            "owned temporary directory 不得为 symlink: {}",
            path.display()
        ))),
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path)?;
            Ok(())
        }
        Ok(_) => Err(failure(format!(
            "owned temporary path 必须是 directory: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(format!(
            "owned temporary path 不可访问: {}: {error}",
            path.display()
        ))),
    }
}

const GENERATED_TEMP_PREFIX: &str = ".generated.tmp-";
const GENERATED_BACKUP_PREFIX: &str = ".generated.old-";
const GENERATED_LOCK_FILE: &str = ".generated.lock";

#[derive(Debug, Clone, Copy)]
enum ReplaceLockMode {
    Shared,
    Exclusive,
}

#[derive(Debug)]
struct ReplaceLock {
    file: fs::File,
}

impl Drop for ReplaceLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn acquire_replace_lock(
    parent: &Path,
    mode: ReplaceLockMode,
    create: bool,
) -> ToolResult<ReplaceLock> {
    validate_directory_chain(parent, false)?;
    let path = parent.join(GENERATED_LOCK_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(failure(format!(
                "generated lock file must not be a symlink: {}",
                path.display()
            )));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(failure(format!(
                "generated lock file must be regular: {}",
                path.display()
            )));
        }
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(failure(format!(
                "generated lock file 不可访问: {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.is_none() && !create {
        return Err(failure(format!(
            "generated lock file missing: {}; run `kanban web-contracts generate` first",
            path.display()
        )));
    }
    let file = if create {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| failure(format!("无法打开 generated lock file: {path:?}: {error}")))?
    } else {
        OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|error| failure(format!("无法读取 generated lock file: {path:?}: {error}")))?
    };
    match mode {
        ReplaceLockMode::Shared => file.lock_shared(),
        ReplaceLockMode::Exclusive => file.lock_exclusive(),
    }
    .map_err(|error| {
        failure(format!(
            "无法锁定 generated output: {}: {error}",
            path.display()
        ))
    })?;
    Ok(ReplaceLock { file })
}

fn recoverable_replace_directory(
    target: &Path,
    files: &BTreeMap<String, Vec<u8>>,
) -> ToolResult<()> {
    let parent = target.parent().ok_or_else(|| {
        failure(format!(
            "Web output path has no parent: {}",
            target.display()
        ))
    })?;
    validate_directory_chain(parent, true)?;
    fs::create_dir_all(parent)?;
    validate_directory_chain(parent, false)?;
    let _lock = acquire_replace_lock(parent, ReplaceLockMode::Exclusive, true)?;
    recover_replace_state(target)?;
    let target_exists = directory_exists(target, "Web output path")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp = parent.join(format!(
        "{GENERATED_TEMP_PREFIX}{}-{nonce}",
        std::process::id()
    ));
    // 上面的 recovery scan 已接管所有 generated prefix 路径；创建新目录前仍显式检查
    // 同 nonce 冲突，避免误删未知路径。
    ensure_absent(&temp, "generated temporary path")?;
    fs::create_dir(&temp)?;
    for (relative, bytes) in files {
        let path = temp.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    }
    let backup = parent.join(format!(
        "{GENERATED_BACKUP_PREFIX}{}-{nonce}",
        std::process::id()
    ));
    ensure_absent(&backup, "generated backup path")?;
    if target_exists {
        fs::rename(target, &backup)?;
    }
    if let Err(error) = fs::rename(&temp, target) {
        if directory_exists(&backup, "generated backup path")? {
            let _ = fs::rename(&backup, target);
        }
        let _ = remove_owned_directory_if_present(&temp);
        return Err(error.into());
    }
    if directory_exists(&backup, "generated backup path")? {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}

/// 恢复在两个目录 rename 之间中断的 replace。
///
/// 该 rename sequence 明确设计为可恢复，而不是声称跨目录 atomic：进程可能在把
/// `target` 移到 `backup` 后、把 `temp` 放入目标前停止。每次调用都会扫描 parent，
/// 只接受至多一组 owned temp/backup，并且只处理含义明确的状态；未知名称、symlink、
/// 非目录和多候选状态都会 fail closed，不删除任何内容。
fn recover_replace_state(target: &Path) -> ToolResult<()> {
    let state = inspect_replace_state(target, true)?;

    match (
        state.target_exists,
        state.backup.is_some(),
        state.temp.is_some(),
    ) {
        (false, true, true) => {
            let backup = state.backup.expect("backup presence checked");
            let temp = state.temp.expect("temp presence checked");
            fs::rename(&backup, target).map_err(|error| {
                failure(format!(
                    "无法从 owned backup 恢复 Web output {}: {error}",
                    target.display()
                ))
            })?;
            remove_owned_directory_if_present(&temp)?;
        }
        (false, true, false) => {
            let backup = state.backup.expect("backup presence checked");
            fs::rename(&backup, target).map_err(|error| {
                failure(format!(
                    "无法从 owned backup 恢复 Web output {}: {error}",
                    target.display()
                ))
            })?;
        }
        (true, true, false) => {
            remove_owned_directory_if_present(&state.backup.expect("backup presence checked"))?;
        }
        (false, false, true) | (true, false, true) => {
            remove_owned_directory_if_present(&state.temp.expect("temp presence checked"))?;
        }
        (true, true, true) => {
            return Err(failure(
                "ambiguous generated replace state: target, backup and temp all exist",
            ));
        }
        (false, false, false) | (true, false, false) => {}
    }
    Ok(())
}

#[derive(Debug)]
struct ReplaceState {
    target_exists: bool,
    temp: Option<PathBuf>,
    backup: Option<PathBuf>,
}

fn inspect_replace_state(target: &Path, create_parent: bool) -> ToolResult<ReplaceState> {
    let parent = target.parent().ok_or_else(|| {
        failure(format!(
            "Web output path has no parent: {}",
            target.display()
        ))
    })?;
    if create_parent {
        validate_directory_chain(parent, true)?;
        fs::create_dir_all(parent)?;
    }
    validate_directory_chain(parent, false)?;
    Ok(ReplaceState {
        target_exists: directory_exists(target, "Web output path")?,
        temp: discover_owned_directory(parent, GENERATED_TEMP_PREFIX)?,
        backup: discover_owned_directory(parent, GENERATED_BACKUP_PREFIX)?,
    })
}

fn ensure_replace_state_clean(target: &Path) -> ToolResult<()> {
    let state = inspect_replace_state(target, false)?;
    if state.temp.is_none() && state.backup.is_none() {
        return Ok(());
    }
    Err(failure(format!(
        "Web output has pending recoverable replace state (target_exists={}, temp={:?}, backup={:?}); run `kanban web-contracts generate` to recover",
        state.target_exists, state.temp, state.backup
    )))
}

fn directory_exists(path: &Path, label: &str) -> ToolResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(failure(format!(
            "{label} must not be a symlink: {}",
            path.display()
        ))),
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(failure(format!(
            "{label} must be a directory: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(failure(format!(
            "{label} 不可访问: {}: {error}",
            path.display()
        ))),
    }
}

fn ensure_absent(path: &Path, label: &str) -> ToolResult<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(failure(format!("{label} collision: {}", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(format!(
            "{label} 不可访问: {}: {error}",
            path.display()
        ))),
    }
}

fn discover_owned_directory(parent: &Path, prefix: &str) -> ToolResult<Option<PathBuf>> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(prefix) {
            continue;
        }
        let suffix = &name[prefix.len()..];
        let valid_name = suffix.split_once('-').is_some_and(|(pid, nonce)| {
            !pid.is_empty()
                && !nonce.is_empty()
                && pid.bytes().all(|byte| byte.is_ascii_digit())
                && nonce.bytes().all(|byte| byte.is_ascii_digit())
        });
        if !valid_name {
            return Err(failure(format!(
                "owned generated path name is invalid: {}",
                path.display()
            )));
        }
        // 不跟随 symlink 解析 metadata；匹配到 symlink 或 regular file 都是不安全/歧义状态，
        // 不得删除。
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(failure(format!(
                    "owned generated path must not be a symlink: {}",
                    path.display()
                )));
            }
            Ok(metadata) if metadata.is_dir() => candidates.push(path),
            Ok(_) => {
                return Err(failure(format!(
                    "owned generated path must be a directory: {}",
                    path.display()
                )));
            }
            Err(error) => {
                return Err(failure(format!(
                    "owned generated path 不可访问: {}: {error}",
                    path.display()
                )));
            }
        }
    }
    if candidates.len() > 1 {
        return Err(failure(format!(
            "ambiguous owned generated paths with prefix {prefix:?}: {candidates:?}"
        )));
    }
    Ok(candidates.pop())
}

fn collect_files(root: &Path) -> ToolResult<BTreeMap<String, Vec<u8>>> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        failure(format!(
            "Web generated directory 不可访问: {}: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(failure(format!(
            "Web generated directory 不得为 symlink: {}",
            root.display()
        )));
    }
    validate_directory_chain(root, false)?;
    if !metadata.is_dir() {
        return Err(failure(format!(
            "Web generated directory 必须是 directory: {}",
            root.display()
        )));
    }
    let mut files = BTreeMap::new();
    collect_files_recursive(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> ToolResult<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_unstable_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(failure(format!(
                "Web generated tree 不得包含 symlink: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, fs::read(path)?);
        } else {
            return Err(failure(format!(
                "Web generated tree 不得包含非 regular file: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn pretty_json_bytes(value: &Value) -> ToolResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonical_json_bytes<T: Serialize + ?Sized>(value: &T) -> ToolResult<Vec<u8>> {
    pretty_json_bytes(&canonicalize(serde_json::to_value(value)?))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn safe_slug(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
}

fn pascal_identifier(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars.next().unwrap_or_default().to_ascii_uppercase();
            format!("{first}{}", chars.as_str())
        })
        .collect()
}

fn schema_const_name(contract_id: &str) -> String {
    format!("{}Schema", pascal_identifier(contract_id))
}

fn schema_type_name(contract_id: &str) -> String {
    format!("{}Contract", pascal_identifier(contract_id))
}

fn validator_name(contract_id: &str) -> String {
    let identifier = pascal_identifier(contract_id);
    let mut chars = identifier.chars();
    match chars.next() {
        Some(first) => format!("{}{}Validator", first.to_ascii_lowercase(), chars.as_str()),
        None => "contractValidator".to_owned(),
    }
}

fn http_method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn failure(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask must live below repository root")
            .to_owned()
    }

    #[test]
    fn selection_is_rendered_only_and_isolated_from_endpoint_catalog() {
        let resolved = resolve_selection(&repository_root()).expect("selection resolves");
        assert!(
            resolved
                .endpoints
                .iter()
                .any(|endpoint| endpoint.operation_id == "api.list-tasks")
        );
        assert!(
            !resolved
                .endpoints
                .iter()
                .any(|endpoint| endpoint.operation_id == "api.create-board")
        );
        assert!(
            !resolved
                .endpoints
                .iter()
                .any(|endpoint| endpoint.operation_id == "api.update-step")
        );
        assert!(
            !resolved
                .endpoints
                .iter()
                .any(|endpoint| endpoint.operation_id == "api.list-task-labels")
        );
        assert!(
            resolved
                .contracts
                .iter()
                .any(|contract| contract.id == "runtime.web-config.output")
        );
        assert!(
            endpoint_catalog()
                .iter()
                .all(|endpoint| endpoint.operation_id != "runtime.web-config.output")
        );
    }

    #[test]
    fn generated_file_set_is_byte_deterministic() {
        let first = expected_files(&repository_root()).expect("first generation");
        let second = expected_files(&repository_root()).expect("second generation");
        assert_eq!(first, second);
        assert!(first.contains_key("generated.sha256"));
        assert!(first.contains_key("runtime.ts"));
        assert!(first.contains_key("test-only.ts"));
        assert!(first.contains_key("contracts/api-get-task-path.ts"));
        assert!(first.contains_key("sse.ts"));
    }

    #[test]
    fn sse_taxonomy_keeps_unknown_and_invalid_payloads_conservative() {
        let files = expected_files(&repository_root()).expect("expected files");
        let sse = String::from_utf8(files["sse.ts"].clone()).expect("generated SSE is UTF-8");
        assert_eq!(
            sse.matches("  \"").count(),
            40,
            "protocol known SSE kind count must stay 40"
        );
        assert!(sse.contains("sseEventDataValidator(value)"));
        assert!(sse.contains("./contracts/sse-event-data"));
        assert!(!sse.contains("./schemas"));
        assert!(!sse.contains("./validators"));
        assert!(sse.contains("reason: \"unknown_kind\""));
        assert!(sse.contains("reason: \"known_payload_invalid\""));
        assert!(sse.contains("reason: \"invalid_envelope\""));
        assert!(sse.contains("Extract<SseEventDataContract, { kind: K }>"));
        assert!(!sse.contains("parseSseEventData(value)"));
    }

    #[test]
    fn operation_manifest_exposes_role_mapping_and_typed_lookup() {
        let files = expected_files(&repository_root()).expect("expected files");
        let operations: Value = serde_json::from_slice(&files["operations.json"])
            .expect("operation manifest should be JSON");
        let operation = |id: &str| {
            operations
                .as_array()
                .expect("operation manifest array")
                .iter()
                .find(|entry| entry["id"] == id)
                .unwrap_or_else(|| panic!("missing operation {id}"))
        };
        assert_eq!(
            operation("api.get-task")["obligations"]["path"]["kind"],
            "contract"
        );
        assert_eq!(
            operation("api.get-task")["obligations"]["path"]["contractId"],
            "api.get-task.path"
        );
        assert_eq!(
            operation("sse.stream-events")["obligations"]["query"]["contractId"],
            "sse.stream-events.query"
        );
        assert_eq!(
            operation("sse.stream-events")["obligations"]["sse"]["contractId"],
            "sse.event.data"
        );
        assert!(
            operation("api.list-tasks")["sharedComponents"]
                .as_array()
                .expect("shared components array")
                .iter()
                .any(|id| id == "api.error.response")
        );
        assert!(
            !operations
                .as_array()
                .expect("operation manifest array")
                .iter()
                .any(|entry| entry["obligations"].to_string().contains("todo"))
        );
        let operations_ts =
            String::from_utf8(files["operations.ts"].clone()).expect("operations TS");
        assert!(operations_ts.contains("operationById"));
        assert!(operations_ts.contains("getOperation<K extends WebOperationId>"));
    }

    #[test]
    fn numeric_policy_preserves_protocol_schema_identity_and_rejects_unsafe_numbers() {
        let root = repository_root();
        let files = expected_files(&root).expect("expected files");
        let manifest: Value = serde_json::from_slice(&files["manifest.json"]).expect("manifest");
        assert_eq!(manifest["numericPolicy"], "reject_unsafe_json_numbers");
        let resolved = resolve_selection(&root).expect("selection resolves");
        for schema_root in &resolved.roots {
            let contract = resolved
                .contracts
                .iter()
                .find(|contract| contract.schema_id == Some(schema_root.id))
                .expect("schema root has selected contract");
            let path = format!("schemas/{}.schema.json", safe_slug(contract.id));
            let generated: Value = serde_json::from_slice(&files[&path]).expect("generated schema");
            let canonical = schema_document(schema_root);
            assert_eq!(generated["$id"], canonical["$id"]);
            assert_eq!(
                generated, canonical,
                "Web projection must not rewrite protocol schema bytes"
            );
        }
        let contract_ts = String::from_utf8(files["contracts/api-list-tasks-query.ts"].clone())
            .expect("contract TS");
        assert!(contract_ts.contains("9223372036854775807"));
        let runtime_ts = String::from_utf8(files["runtime.ts"].clone()).expect("runtime TS");
        assert!(runtime_ts.contains("Number.isSafeInteger"));
        assert!(runtime_ts.contains("safeNumber"));
    }

    #[test]
    fn production_entrypoint_does_not_reach_aggregate_contract_modules() {
        let files = expected_files(&repository_root()).expect("expected files");
        let index = String::from_utf8(files["index.ts"].clone()).expect("index TS");
        assert!(index.contains("export * from \"./operations\""));
        assert!(!index.contains("./schemas"));
        assert!(!index.contains("./validators"));
        assert!(!index.contains("./contracts/"));
        assert!(!index.contains("./runtime"));
        assert!(!index.contains("./sse"));

        let contract = String::from_utf8(files["contracts/api-get-task-path.ts"].clone())
            .expect("contract module");
        assert!(contract.contains("from \"../runtime\""));
        assert!(!contract.contains("from \"../schemas\""));
        assert!(!contract.contains("from \"../validators\""));
        let test_only = String::from_utf8(files["test-only.ts"].clone()).expect("test-only TS");
        assert!(test_only.contains("仅供契约 fixtures/validator 测试使用"));
        assert!(test_only.contains("./contracts/api-get-task-path"));
    }

    #[test]
    fn generated_path_collisions_fail_closed_for_all_artifact_kinds() {
        assert_eq!(safe_slug("contract.one"), safe_slug("contract-one"));
        for kind in [
            "schema",
            "valid fixture",
            "invalid fixture",
            "contract module",
        ] {
            let mut files = BTreeMap::new();
            insert_generated_file(
                &mut files,
                "collision/path".to_owned(),
                b"first".to_vec(),
                kind,
            )
            .expect("first artifact insertion");
            let error = insert_generated_file(
                &mut files,
                "collision/path".to_owned(),
                b"second".to_vec(),
                kind,
            )
            .expect_err("duplicate artifact insertion must fail closed");
            assert!(error.to_string().contains("duplicate"));
            assert!(error.to_string().contains(kind));
        }
    }

    #[test]
    fn stale_and_orphan_files_are_rejected_without_writing() {
        let root = repository_root();
        let expected = expected_files(&root).expect("expected files");
        let temp =
            std::env::temp_dir().join(format!("kanban-web-contracts-test-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old test directory");
        }
        fs::create_dir_all(&temp).expect("create test directory");
        for (path, bytes) in &expected {
            let destination = temp.join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).expect("create nested output directory");
            }
            fs::write(destination, bytes).expect("write expected output");
        }
        compare_output_directory(&temp, &expected).expect("fresh output should pass");
        fs::write(temp.join("manifest.json"), b"drift").expect("write drift");
        let error = compare_output_directory(&temp, &expected).expect_err("byte drift must fail");
        assert!(error.to_string().contains("artifact 已漂移"));
        let manifest = &expected["manifest.json"];
        fs::write(temp.join("manifest.json"), manifest).expect("restore manifest");
        fs::write(temp.join("orphan.txt"), b"stale").expect("write stale file");
        let error = compare_output_directory(&temp, &expected).expect_err("stale must fail");
        assert!(error.to_string().contains("stale_or_orphan"));
        fs::remove_dir_all(temp).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn collect_files_rejects_symlink_root() {
        use std::os::unix::fs::symlink;

        let temp = std::env::temp_dir().join(format!(
            "kanban-web-contracts-root-symlink-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old root symlink test directory");
        }
        fs::create_dir_all(&temp).expect("create root symlink test directory");
        let target = temp.join("target");
        fs::create_dir(&target).expect("create target directory");
        fs::write(target.join("artifact"), b"artifact").expect("write target artifact");
        let link = temp.join("generated");
        symlink(&target, &link).expect("create root symlink");
        let error = collect_files(&link).expect_err("symlink root must fail");
        assert!(error.to_string().contains("symlink"));
        fs::remove_file(link).expect("remove root symlink");
        fs::remove_dir_all(temp).expect("remove root symlink test directory");
    }

    #[cfg(unix)]
    #[test]
    fn repo_relative_source_rejects_parent_and_symlink_components() {
        use std::os::unix::fs::symlink;

        let temp = std::env::temp_dir().join(format!(
            "kanban-web-contracts-source-chain-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old source chain test directory");
        }
        fs::create_dir_all(&temp).expect("create source chain test directory");
        let repo = temp.join("repo");
        let external = temp.join("external");
        fs::create_dir(&repo).expect("create repo directory");
        fs::create_dir(&external).expect("create external directory");
        fs::write(external.join("selection.json"), b"{}").expect("write external selection");
        symlink(&external, repo.join("source")).expect("create source ancestor symlink");
        let error = repo_relative_path(&repo, "source/selection.json", RepoPathKind::File, false)
            .expect_err("source ancestor symlink must fail");
        assert!(error.to_string().contains("symlink"));
        fs::remove_file(repo.join("source")).expect("remove source ancestor symlink");
        fs::create_dir(repo.join("source")).expect("create source directory");
        symlink(
            external.join("selection.json"),
            repo.join("source/selection.json"),
        )
        .expect("create source file symlink");
        let error = repo_relative_path(&repo, "source/selection.json", RepoPathKind::File, false)
            .expect_err("source file symlink must fail");
        assert!(error.to_string().contains("symlink"));
        let error = repo_relative_path(
            &repo,
            "../external/selection.json",
            RepoPathKind::File,
            false,
        )
        .expect_err("parent component must fail");
        assert!(error.to_string().contains("regular path components"));
        let absolute = external.join("selection.json");
        let error = repo_relative_path(
            &repo,
            &absolute.to_string_lossy(),
            RepoPathKind::File,
            false,
        )
        .expect_err("absolute path must fail");
        assert!(error.to_string().contains("absolute"));
        fs::remove_dir_all(temp).expect("remove source chain test directory");
    }

    #[test]
    fn recoverable_state_restores_missing_target_and_removes_stale_temp() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-recovery-missing-target-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old recovery directory");
        }
        fs::create_dir_all(&root).expect("create recovery directory");
        let target = root.join("generated");
        let backup = root.join(".generated.old-123-1");
        let temp = root.join(".generated.tmp-123-1");
        fs::create_dir(&backup).expect("create owned backup");
        fs::write(backup.join("sentinel"), b"old").expect("write old sentinel");
        fs::create_dir(&temp).expect("create owned temp");
        fs::write(temp.join("sentinel"), b"new").expect("write new sentinel");

        recover_replace_state(&target).expect("missing target should recover from backup");

        assert_eq!(
            fs::read(target.join("sentinel")).expect("read restored target"),
            b"old"
        );
        assert!(!backup.exists());
        assert!(!temp.exists());
        fs::remove_dir_all(root).expect("remove recovery directory");
    }

    #[test]
    fn recoverable_state_cleans_new_target_backup_after_temp_rename() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-recovery-new-target-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old recovery directory");
        }
        fs::create_dir_all(&root).expect("create recovery directory");
        let target = root.join("generated");
        let backup = root.join(".generated.old-123-2");
        fs::create_dir(&target).expect("create new target");
        fs::write(target.join("sentinel"), b"new").expect("write new sentinel");
        fs::create_dir(&backup).expect("create owned backup");
        fs::write(backup.join("sentinel"), b"old").expect("write old sentinel");

        recover_replace_state(&target).expect("new target should win and stale backup clean");

        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target"),
            b"new"
        );
        assert!(!backup.exists());
        fs::remove_dir_all(root).expect("remove recovery directory");
    }

    #[test]
    fn recoverable_state_cleans_temp_before_target_rename() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-recovery-temp-only-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old recovery directory");
        }
        fs::create_dir_all(&root).expect("create recovery directory");
        let target = root.join("generated");
        let temp = root.join(".generated.tmp-123-3");
        fs::create_dir(&temp).expect("create owned temp");

        recover_replace_state(&target).expect("stale temp should be cleaned");

        assert!(!target.exists());
        assert!(!temp.exists());
        fs::remove_dir_all(root).expect("remove recovery directory");
    }

    #[test]
    fn recoverable_state_rejects_ambiguous_target_backup_temp_without_mutation() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-recovery-ambiguous-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old recovery directory");
        }
        fs::create_dir_all(&root).expect("create recovery directory");
        let target = root.join("generated");
        let backup = root.join(".generated.old-123-4");
        let temp = root.join(".generated.tmp-123-4");
        fs::create_dir(&target).expect("create target");
        fs::write(target.join("sentinel"), b"target").expect("write target sentinel");
        fs::create_dir(&backup).expect("create backup");
        fs::write(backup.join("sentinel"), b"backup").expect("write backup sentinel");
        fs::create_dir(&temp).expect("create temp");
        fs::write(temp.join("sentinel"), b"temp").expect("write temp sentinel");

        let error = recover_replace_state(&target).expect_err("ambiguous state must fail closed");

        assert!(error.to_string().contains("ambiguous"));
        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target"),
            b"target"
        );
        assert_eq!(
            fs::read(backup.join("sentinel")).expect("read backup"),
            b"backup"
        );
        assert_eq!(fs::read(temp.join("sentinel")).expect("read temp"), b"temp");
        fs::remove_dir_all(root).expect("remove recovery directory");
    }

    #[cfg(unix)]
    #[test]
    fn recoverable_state_rejects_owned_symlink_without_removing_it() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-recovery-symlink-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old recovery directory");
        }
        fs::create_dir_all(&root).expect("create recovery directory");
        let external = root.join("external");
        fs::create_dir(&external).expect("create external directory");
        let target = root.join("generated");
        let temp = root.join(".generated.tmp-123-6");
        symlink(&external, &temp).expect("create owned temp symlink");

        let error = recover_replace_state(&target).expect_err("owned symlink must fail closed");

        assert!(error.to_string().contains("symlink"));
        assert!(temp.exists());
        assert!(!target.exists());
        fs::remove_file(temp).expect("remove owned temp symlink");
        fs::remove_dir_all(root).expect("remove recovery directory");
    }

    #[test]
    fn check_pending_replace_state_is_read_only() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-check-read-only-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old check directory");
        }
        fs::create_dir_all(&root).expect("create check directory");
        let target = root.join("generated");
        let backup = root.join(".generated.old-123-5");
        fs::create_dir(&target).expect("create target");
        fs::write(target.join("sentinel"), b"target").expect("write target sentinel");
        fs::create_dir(&backup).expect("create backup");
        fs::write(backup.join("sentinel"), b"backup").expect("write backup sentinel");
        let mut before = fs::read_dir(&root)
            .expect("read before state")
            .map(|entry| {
                let entry = entry.expect("read before entry");
                let metadata = entry.metadata().expect("read before metadata");
                (entry.file_name(), metadata.len(), metadata.is_dir())
            })
            .collect::<Vec<_>>();
        before.sort_by(|left, right| left.0.cmp(&right.0));

        let error =
            ensure_replace_state_clean(&target).expect_err("check must report pending state");
        assert!(
            error
                .to_string()
                .contains("run `kanban web-contracts generate`")
        );

        let mut after = fs::read_dir(&root)
            .expect("read after state")
            .map(|entry| {
                let entry = entry.expect("read after entry");
                let metadata = entry.metadata().expect("read after metadata");
                (entry.file_name(), metadata.len(), metadata.is_dir())
            })
            .collect::<Vec<_>>();
        after.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(before, after);
        fs::remove_dir_all(root).expect("remove check directory");
    }

    #[test]
    fn check_missing_lock_is_actionable_and_read_only() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-check-missing-lock-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old missing-lock directory");
        }
        fs::create_dir_all(&root).expect("create missing-lock directory");
        let before = fs::read_dir(&root)
            .expect("read before missing-lock state")
            .count();

        let error = acquire_replace_lock(&root, ReplaceLockMode::Shared, false)
            .expect_err("missing shared lock must fail without creating it");

        assert!(error.to_string().contains("generated lock file missing"));
        assert_eq!(
            fs::read_dir(&root)
                .expect("read after missing-lock state")
                .count(),
            before
        );
        assert!(!root.join(GENERATED_LOCK_FILE).exists());
        fs::remove_dir_all(root).expect("remove missing-lock directory");
    }

    #[test]
    fn replace_lock_serializes_exclusive_generators() {
        let root = std::env::temp_dir().join(format!(
            "kanban-web-contracts-lock-test-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old lock directory");
        }
        fs::create_dir_all(&root).expect("create lock directory");
        let first = acquire_replace_lock(&root, ReplaceLockMode::Exclusive, true)
            .expect("acquire first exclusive lock");
        let lock_path = root.join(GENERATED_LOCK_FILE);
        let contender = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .expect("open lock contender");
        assert!(!contender.try_lock_exclusive().expect("try lock contender"));
        drop(first);
        assert!(
            contender
                .try_lock_exclusive()
                .expect("try lock after release")
        );
        contender.unlock().expect("unlock contender");
        fs::remove_dir_all(root).expect("remove lock directory");
    }

    #[cfg(unix)]
    #[test]
    fn recoverable_generation_rejects_symlink_output_parent() {
        use std::os::unix::fs::symlink;

        let temp = std::env::temp_dir().join(format!(
            "kanban-web-contracts-parent-symlink-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old parent symlink test directory");
        }
        fs::create_dir_all(&temp).expect("create parent symlink test directory");
        let external = temp.join("external");
        fs::create_dir(&external).expect("create external directory");
        let parent = temp.join("parent");
        symlink(&external, &parent).expect("create parent symlink");
        let target = parent.join("generated");
        let mut files = BTreeMap::new();
        files.insert("manifest.json".to_owned(), b"{}\n".to_vec());
        let error =
            recoverable_replace_directory(&target, &files).expect_err("parent symlink must fail");
        assert!(error.to_string().contains("symlink"));
        assert!(!external.join("manifest.json").exists());
        fs::remove_file(parent).expect("remove parent symlink");
        fs::remove_dir_all(temp).expect("remove parent symlink test directory");
    }

    #[cfg(unix)]
    #[test]
    fn recoverable_generation_rejects_symlink_output_without_touching_target() {
        use std::os::unix::fs::symlink;

        let root = repository_root();
        let expected = expected_files(&root).expect("expected files");
        let temp = std::env::temp_dir().join(format!(
            "kanban-web-contracts-symlink-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old symlink test directory");
        }
        fs::create_dir_all(&temp).expect("create symlink test directory");
        let target = temp.join("target");
        fs::create_dir(&target).expect("create target directory");
        fs::write(target.join("sentinel"), b"keep").expect("write target sentinel");
        let link = temp.join("generated");
        symlink(&target, &link).expect("create output symlink");
        let error = recoverable_replace_directory(&link, &expected).expect_err("symlink must fail");
        assert!(error.to_string().contains("symlink"));
        assert_eq!(
            fs::read(target.join("sentinel")).expect("read target sentinel"),
            b"keep"
        );
        fs::remove_file(link).expect("remove output symlink");
        fs::remove_dir_all(temp).expect("remove symlink test directory");
    }

    #[cfg(unix)]
    #[test]
    fn check_rejects_nested_symlink_in_generated_tree() {
        use std::os::unix::fs::symlink;

        let root = repository_root();
        let expected = expected_files(&root).expect("expected files");
        let temp = std::env::temp_dir().join(format!(
            "kanban-web-contracts-nested-symlink-test-{}",
            std::process::id()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).expect("remove old nested symlink test directory");
        }
        fs::create_dir_all(&temp).expect("create nested symlink test directory");
        for (path, bytes) in &expected {
            let destination = temp.join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).expect("create nested output directory");
            }
            fs::write(destination, bytes).expect("write expected output");
        }
        let external = temp.join("external");
        fs::create_dir(&external).expect("create external directory");
        let link = temp.join("schemas/nested-link.json");
        symlink(&external, &link).expect("create nested output symlink");
        let error = compare_output_directory(&temp, &expected).expect_err("symlink must fail");
        assert!(error.to_string().contains("symlink"));
        fs::remove_file(link).expect("remove nested output symlink");
        fs::remove_dir_all(temp).expect("remove nested symlink test directory");
    }
}
