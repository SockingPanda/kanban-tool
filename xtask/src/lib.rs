#![doc = include_str!("../README.md")]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
};

use jsonschema::Draft;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use kanban_protocol::{
    ContractBinding, ContractGranularity, ContractTransport, OperationContract, SurfaceOperation,
    endpoint_catalog, generated_artifacts, generated_schema_ids, operation_inventory,
    schema::{DRAFT_2020_12, SchemaRoot, canonicalize, schema_document, schema_registry},
    surface_operation_catalog, validate_endpoint_catalog, validate_operation_contracts,
};

pub type ToolResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub mod web_contracts;

pub const ARTIFACT_DIRECTORY: &str = "schemas/json-schema/draft-2020-12";

#[derive(Debug, Serialize)]
struct ArtifactManifest<'a> {
    schema_dialect: &'static str,
    contract_inventory: ArtifactIndex,
    surface_catalog: ArtifactIndex,
    roots: Vec<RootManifest<'a>>,
}

#[derive(Debug, Serialize)]
struct ArtifactIndex {
    path: &'static str,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct RootManifest<'a> {
    id: &'a str,
    path: &'a str,
    contract_id: &'a str,
    direction: kanban_protocol::ContractDirection,
    strictness: kanban_protocol::ContractStrictness,
    schema_fixture: &'a str,
    invalid_fixture: &'a str,
    sha256: String,
}

pub fn audit_inventory() -> ToolResult<()> {
    validate_endpoint_catalog(endpoint_catalog()).map_err(failure)?;
    let operations = audit_operation_entries(operation_inventory())?;
    let surfaces = surface_operation_catalog();
    audit_surface_entries(&surfaces, &operations)?;
    audit_shared_component_links(&surfaces, &operations)?;
    audit_schema_registry(&operations)
}

fn audit_operation_entries(
    entries: &[OperationContract],
) -> ToolResult<BTreeMap<&str, &OperationContract>> {
    let mut operations = BTreeMap::new();

    for operation in entries {
        if operation.id.trim().is_empty()
            || operation.path.trim().is_empty()
            || operation.operation.trim().is_empty()
        {
            return Err(failure(format!(
                "contract inventory 存在空 id/path/operation: {operation:?}"
            )));
        }
        if operations.insert(operation.id, operation).is_some() {
            return Err(failure(format!(
                "contract inventory 存在重复 id: {}",
                operation.id
            )));
        }
        if operation.granularity == ContractGranularity::Exact && contains_wildcard(operation.path)
        {
            return Err(failure(format!(
                "exact contract 不得包含 wildcard path: {} ({})",
                operation.id, operation.path
            )));
        }
        if operation.binding == ContractBinding::ExactSurface
            && operation.granularity != ContractGranularity::Exact
        {
            return Err(failure(format!(
                "ExactSurface contract requires exact granularity: contract={} binding={} expected=exact actual={}",
                operation.id,
                contract_binding_name(operation.binding),
                contract_granularity_name(operation.granularity)
            )));
        }
        if operation.schema_id.is_some() != operation.fixture.is_some() {
            return Err(failure(format!(
                "contract 的 schema_id 与 fixture 必须同时存在或同时缺省: {}",
                operation.id
            )));
        }
        if operation
            .exclusion
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(failure(format!(
                "contract exclusion 理由不能为空: {}",
                operation.id
            )));
        }
        if operation.exclusion.is_some()
            && (operation.schema_id.is_some() || operation.fixture.is_some())
        {
            return Err(failure(format!(
                "excluded contract 不得同时声明 schema/fixture: {}",
                operation.id
            )));
        }
    }

    validate_operation_contracts(entries).map_err(failure)?;
    Ok(operations)
}

fn audit_surface_entries(
    entries: &[SurfaceOperation],
    operations: &BTreeMap<&str, &OperationContract>,
) -> ToolResult<()> {
    let mut keys = BTreeSet::new();
    let mut exact_bindings = BTreeMap::new();

    for entry in entries {
        if entry.key.trim().is_empty() {
            return Err(failure("surface catalog 存在空 operation key"));
        }
        if contains_wildcard(&entry.key) {
            return Err(failure(format!(
                "surface operation key 必须精确，禁止 wildcard: {:?} {}",
                entry.surface, entry.key
            )));
        }
        if !keys.insert((entry.surface, entry.key.clone())) {
            return Err(failure(format!(
                "surface catalog 存在重复 key: {:?} {}",
                entry.surface, entry.key
            )));
        }

        if entry.exclusion.is_some() {
            if !entry.contracts.is_empty()
                || entry
                    .exclusion
                    .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(failure(format!(
                    "excluded surface 必须只有明确 exclusion 理由: {}",
                    entry.key
                )));
            }
            continue;
        }

        let mut exact_contracts = Vec::new();
        let mut linked_contracts = BTreeSet::new();
        for contract_id in &entry.contracts {
            if !linked_contracts.insert(*contract_id) {
                return Err(failure(format!(
                    "surface 存在重复 contract linkage: {} -> {}",
                    entry.key, contract_id
                )));
            }
            let contract = operations.get(contract_id).ok_or_else(|| {
                failure(format!(
                    "surface {} 链接未知 contract: {}",
                    entry.key, contract_id
                ))
            })?;
            if contract.surface != entry.surface {
                return Err(failure(format!(
                    "contract 与 surface 漂移: {:?} {} -> {:?} {}",
                    entry.surface, entry.key, contract.surface, contract.id
                )));
            }
            match contract.binding {
                ContractBinding::ExactSurface => exact_contracts.push(*contract),
                ContractBinding::SharedComponent => {}
            }
        }

        if exact_contracts.is_empty() {
            return Err(failure(format!(
                "非排除 surface 必须链接至少一个 ExactSurface contract: {}",
                entry.key
            )));
        }

        for contract in exact_contracts {
            audit_exact_surface_binding(entry, contract, &mut exact_bindings)?;
        }
    }

    let expected_exact = operations
        .values()
        .filter(|contract| contract.binding == ContractBinding::ExactSurface)
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();
    let bound_exact = exact_bindings.keys().copied().collect::<BTreeSet<_>>();
    if bound_exact != expected_exact {
        let missing = expected_exact
            .difference(&bound_exact)
            .copied()
            .collect::<Vec<_>>();
        return Err(failure(format!(
            "ExactSurface contract 缺少 surface operation: {missing:?}"
        )));
    }
    Ok(())
}

fn audit_exact_surface_binding<'a>(
    entry: &SurfaceOperation,
    contract: &'a OperationContract,
    exact_bindings: &mut BTreeMap<&'a str, String>,
) -> ToolResult<()> {
    if contract.binding != ContractBinding::ExactSurface {
        return Err(failure(format!(
            "exact surface binding 收到 shared component contract: {}",
            contract.id
        )));
    }
    if contract.granularity != ContractGranularity::Exact {
        return Err(failure(format!(
            "exact surface binding requires exact granularity: surface={} contract={} binding={} expected=exact actual={}",
            entry.key,
            contract.id,
            contract_binding_name(contract.binding),
            contract_granularity_name(contract.granularity)
        )));
    }
    if contract.surface != entry.surface {
        return Err(failure(format!(
            "exact contract 与 surface 漂移: {:?} {} -> {:?} {}",
            entry.surface, entry.key, contract.surface, contract.id
        )));
    }

    match contract.transport {
        ContractTransport::Http { operation_key, .. } => {
            if operation_key != Some(entry.key.as_str()) {
                return Err(failure(format!(
                    "exact contract operation 与 surface operation 漂移: {} -> {}",
                    contract.id, entry.key
                )));
            }
        }
        ContractTransport::NoTransport => {
            if contract.operation != entry.key {
                return Err(failure(format!(
                    "exact contract operation 与 surface operation 漂移: {} -> {}",
                    contract.id, entry.key
                )));
            }
        }
    }

    if let Some(first_surface) = exact_bindings.insert(contract.id, entry.key.clone()) {
        return Err(failure(format!(
            "exact contract duplicate surface binding: contract={} first={} second={}",
            contract.id, first_surface, entry.key
        )));
    }
    Ok(())
}

fn audit_shared_component_links(
    entries: &[SurfaceOperation],
    operations: &BTreeMap<&str, &OperationContract>,
) -> ToolResult<()> {
    let linked_shared = entries
        .iter()
        .flat_map(|entry| {
            entry.contracts.iter().filter_map(|contract_id| {
                operations.get(contract_id).and_then(|contract| {
                    (contract.binding == ContractBinding::SharedComponent
                        && contract.surface == entry.surface)
                        .then_some(contract.id)
                })
            })
        })
        .collect::<BTreeSet<_>>();

    let expected_shared = operations
        .values()
        .filter(|contract| contract.binding == ContractBinding::SharedComponent)
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();
    if linked_shared != expected_shared {
        let missing = expected_shared
            .difference(&linked_shared)
            .copied()
            .collect::<Vec<_>>();
        return Err(failure(format!(
            "SharedComponent contract 缺少显式 surface linkage: {missing:?}"
        )));
    }
    Ok(())
}

fn contract_binding_name(binding: kanban_protocol::ContractBinding) -> &'static str {
    match binding {
        kanban_protocol::ContractBinding::ExactSurface => "exact_surface",
        kanban_protocol::ContractBinding::SharedComponent => "shared_component",
    }
}

fn contract_granularity_name(granularity: ContractGranularity) -> &'static str {
    match granularity {
        ContractGranularity::Exact => "exact",
        ContractGranularity::Family => "family",
    }
}

fn audit_schema_registry(operations: &BTreeMap<&str, &OperationContract>) -> ToolResult<()> {
    let schema_operations = operations
        .values()
        .filter(|operation| operation.schema_id.is_some() && operation.fixture.is_some())
        .map(|operation| (operation.id, *operation))
        .collect::<BTreeMap<_, _>>();

    let mut root_ids = BTreeSet::new();
    let mut artifact_paths = BTreeSet::new();
    for root in schema_registry() {
        if !root_ids.insert(root.id) {
            return Err(failure(format!("schema registry 存在重复 id: {}", root.id)));
        }
        if !artifact_paths.insert(root.artifact_path) {
            return Err(failure(format!(
                "schema registry 存在重复 artifact path: {}",
                root.artifact_path
            )));
        }
        let operation = schema_operations.get(root.contract_id).ok_or_else(|| {
            failure(format!(
                "schema root {} 未指向声明 schema/fixture 的 contract {}",
                root.id, root.contract_id
            ))
        })?;
        audit_root_mapping(root, operation)?;
    }

    let declared_ids = generated_schema_ids()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if declared_ids != root_ids {
        return Err(failure(format!(
            "generated_schema_ids 与 registry 不一致: declared={declared_ids:?}, roots={root_ids:?}"
        )));
    }
    if schema_operations.len() != schema_registry().len() {
        return Err(failure(format!(
            "schema contract 与 schema root 数量不一致: {} != {}",
            schema_operations.len(),
            schema_registry().len()
        )));
    }
    Ok(())
}

fn audit_root_mapping(root: &SchemaRoot, operation: &OperationContract) -> ToolResult<()> {
    let expected_id = operation
        .schema_id
        .expect("schema contract schema_id was checked before root mapping");
    if expected_id != root.id {
        return Err(failure(format!(
            "contract {} 的 schema_id 与 root 不一致: {} != {}",
            root.contract_id, expected_id, root.id
        )));
    }
    if operation.direction != root.direction {
        return Err(failure(format!(
            "contract {} 的 direction 与 root 不一致: {:?} != {:?}",
            root.contract_id, operation.direction, root.direction
        )));
    }
    if operation.strictness != root.strictness {
        return Err(failure(format!(
            "contract {} 的 strictness 与 root 不一致: {:?} != {:?}",
            root.contract_id, operation.strictness, root.strictness
        )));
    }
    let expected_fixture = operation
        .fixture
        .expect("schema contract fixture was checked before root mapping");
    if expected_fixture != root.valid_fixture {
        return Err(failure(format!(
            "contract {} 的 schema fixture 与 root 不一致: {} != {}",
            root.contract_id, expected_fixture, root.valid_fixture
        )));
    }
    Ok(())
}

pub fn write_generated(repo_root: &Path) -> ToolResult<()> {
    audit_inventory()?;
    validate_generated_schemas()?;
    validate_fixtures(repo_root)?;

    let target = repo_root.join(ARTIFACT_DIRECTORY);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::create_dir_all(&target)?;
    for (relative, bytes) in expected_artifacts()? {
        let path = target.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    }
    Ok(())
}

pub fn check_contract(repo_root: &Path) -> ToolResult<()> {
    audit_inventory()?;
    validate_generated_schemas()?;
    validate_fixtures(repo_root)?;
    check_fixture_tree(repo_root)?;
    check_committed_artifacts(repo_root)
}

pub fn check_committed_artifacts(repo_root: &Path) -> ToolResult<()> {
    let target = repo_root.join(ARTIFACT_DIRECTORY);
    let expected = expected_artifacts()?;
    let actual = collect_files(&target)?;

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
            "committed schema tree 不完整：missing={missing:?}, stale_or_orphan={stale:?}"
        )));
    }

    for (path, expected_bytes) in expected {
        let actual_bytes = actual
            .get(&path)
            .expect("path sets were checked before byte comparison");
        if actual_bytes != &expected_bytes {
            return Err(failure(format!(
                "committed schema artifact 已漂移: {}/{}",
                ARTIFACT_DIRECTORY, path
            )));
        }
    }
    Ok(())
}

pub fn validate_generated_schemas() -> ToolResult<()> {
    for root in schema_registry() {
        let schema = schema_document(root);
        jsonschema::meta::validate(&schema)
            .map_err(|error| failure(format!("{} metaschema 校验失败: {error}", root.id)))?;
        jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(&schema)
            .map_err(|error| failure(format!("{} validator 编译失败: {error}", root.id)))?;
    }
    Ok(())
}

pub fn validate_fixtures(repo_root: &Path) -> ToolResult<()> {
    for root in schema_registry() {
        let schema = schema_document(root);
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(&schema)
            .map_err(|error| failure(format!("{} validator 编译失败: {error}", root.id)))?;

        let valid = read_json(&repo_root.join(root.valid_fixture))?;
        if let Err(error) = validator.validate(&valid) {
            return Err(failure(format!(
                "{} schema fixture 校验失败（instance {}）: {error}",
                root.valid_fixture,
                error.instance_path()
            )));
        }

        let invalid = read_json(&repo_root.join(root.invalid_fixture))?;
        if validator.is_valid(&invalid) {
            return Err(failure(format!(
                "{} 负例 fixture 未被拒绝（root {}）",
                root.invalid_fixture, root.id
            )));
        }
    }
    Ok(())
}

pub fn expected_artifacts() -> ToolResult<BTreeMap<String, Vec<u8>>> {
    let mut artifacts = generated_artifacts();

    let contract_inventory_bytes = canonical_json_bytes(operation_inventory())?;
    let contract_inventory_hash = sha256(&contract_inventory_bytes);
    artifacts.insert("operations.json".to_owned(), contract_inventory_bytes);

    let surface_catalog_bytes = canonical_json_bytes(&surface_operation_catalog())?;
    let surface_catalog_hash = sha256(&surface_catalog_bytes);
    artifacts.insert("surface-operations.json".to_owned(), surface_catalog_bytes);

    let mut seen_hashes = BTreeSet::new();
    let mut roots = Vec::new();
    for root in schema_registry() {
        let bytes = artifacts.get(root.artifact_path).ok_or_else(|| {
            failure(format!(
                "generator 未生成 registry path: {}",
                root.artifact_path
            ))
        })?;
        let hash = sha256(bytes);
        if !seen_hashes.insert(hash.clone()) {
            return Err(failure(format!(
                "schema roots 产生重复 hash，需确认不是重复 contract: {}",
                root.id
            )));
        }
        roots.push(root_manifest(root, hash));
    }

    let manifest = ArtifactManifest {
        schema_dialect: DRAFT_2020_12,
        contract_inventory: ArtifactIndex {
            path: "operations.json",
            sha256: contract_inventory_hash,
        },
        surface_catalog: ArtifactIndex {
            path: "surface-operations.json",
            sha256: surface_catalog_hash,
        },
        roots,
    };
    let manifest = canonicalize(serde_json::to_value(manifest)?);
    artifacts.insert("manifest.json".to_owned(), pretty_json_bytes(&manifest)?);
    Ok(artifacts)
}

fn root_manifest(root: &SchemaRoot, sha256: String) -> RootManifest<'_> {
    RootManifest {
        id: root.id,
        path: root.artifact_path,
        contract_id: root.contract_id,
        direction: root.direction,
        strictness: root.strictness,
        schema_fixture: root.valid_fixture,
        invalid_fixture: root.invalid_fixture,
        sha256,
    }
}

fn canonical_json_bytes<T: Serialize + ?Sized>(value: &T) -> ToolResult<Vec<u8>> {
    let value = canonicalize(serde_json::to_value(value)?);
    pretty_json_bytes(&value)
}

fn pretty_json_bytes(value: &Value) -> ToolResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn read_json(path: &Path) -> ToolResult<Value> {
    let bytes = fs::read(path)
        .map_err(|error| failure(format!("无法读取 fixture {}: {error}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| failure(format!("fixture 不是合法 JSON {}: {error}", path.display())))
}

fn check_fixture_tree(repo_root: &Path) -> ToolResult<()> {
    let fixture_root = repo_root.join("schemas/fixtures");
    let actual = collect_files(&fixture_root)?;
    let actual_paths = actual.keys().cloned().collect::<BTreeSet<_>>();
    let expected_paths = schema_registry()
        .iter()
        .flat_map(|root| [root.valid_fixture, root.invalid_fixture])
        .map(|path| {
            path.strip_prefix("schemas/fixtures/")
                .expect("registry fixture path must live below schemas/fixtures")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        let missing = expected_paths
            .difference(&actual_paths)
            .cloned()
            .collect::<Vec<_>>();
        let orphan = actual_paths
            .difference(&expected_paths)
            .cloned()
            .collect::<Vec<_>>();
        return Err(failure(format!(
            "fixture tree 不完整：missing={missing:?}, orphan={orphan:?}"
        )));
    }
    Ok(())
}

fn collect_files(root: &Path) -> ToolResult<BTreeMap<String, Vec<u8>>> {
    if !root.is_dir() {
        return Err(failure(format!("目录不存在: {}", root.display())));
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
        if path.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, fs::read(path)?);
        }
    }
    Ok(())
}

fn contains_wildcard(value: &str) -> bool {
    value.contains('*') || value.contains("...")
}

fn failure(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_protocol::{
        ContractDirection, ContractStrictness, ContractSurface, HttpTransportLocation,
    };

    fn exact_operation() -> OperationContract {
        OperationContract {
            id: "test.exact.output",
            path: "GET /test response",
            surface: ContractSurface::Api,
            operation: "GET /test",
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:test:exact-output"),
            fixture: Some("schemas/fixtures/test-exact-output.json"),
            exclusion: None,
            transport: ContractTransport::Http {
                operation_key: Some("GET /test"),
                location: HttpTransportLocation::Success,
                parameters: &[],
            },
            binding: ContractBinding::ExactSurface,
        }
    }

    fn shared_operation() -> OperationContract {
        OperationContract {
            id: "test.error.output",
            path: "HTTP JSON error envelope",
            surface: ContractSurface::Api,
            operation: "HTTP JSON error envelope",
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:test:error-output"),
            fixture: Some("schemas/fixtures/test-error-output.json"),
            exclusion: None,
            transport: ContractTransport::Http {
                operation_key: None,
                location: HttpTransportLocation::Error,
                parameters: &[],
            },
            binding: ContractBinding::SharedComponent,
        }
    }

    fn surface(contracts: Vec<&'static str>) -> SurfaceOperation {
        SurfaceOperation {
            key: "GET /test".to_owned(),
            surface: ContractSurface::Api,
            contracts,
            exclusion: None,
        }
    }

    #[test]
    fn current_inventory_satisfies_stable_contract_audit() {
        audit_inventory().expect("当前 contract 与 surface catalog 必须满足稳定审计");
    }

    #[test]
    fn root_mapping_rejects_inventory_contract_drift() {
        let root = &schema_registry()[0];
        let operation = operation_inventory()
            .iter()
            .find(|operation| operation.id == root.contract_id)
            .expect("schema root 必须有对应 contract");

        let mut direction_drift = *operation;
        direction_drift.direction = match operation.direction {
            ContractDirection::Serialize => ContractDirection::Deserialize,
            ContractDirection::Deserialize => ContractDirection::Serialize,
            ContractDirection::Bidirectional => ContractDirection::Serialize,
        };
        let error =
            audit_root_mapping(root, &direction_drift).expect_err("direction 漂移必须被拒绝");
        assert!(error.to_string().contains("direction"));

        let mut fixture_drift = *operation;
        fixture_drift.fixture = Some("schemas/fixtures/api/not-the-root.json");
        let error = audit_root_mapping(root, &fixture_drift).expect_err("fixture 漂移必须被拒绝");
        assert!(error.to_string().contains("fixture"));
    }

    #[test]
    fn operation_audit_rejects_ambiguous_or_incomplete_contracts() {
        let exact = exact_operation();
        let error =
            audit_operation_entries(&[exact, exact]).expect_err("重复 contract id 必须被拒绝");
        assert!(error.to_string().contains("重复 id"), "{error}");

        let mut wildcard = exact_operation();
        wildcard.path = "/api/v1/**";
        let error = audit_operation_entries(&[wildcard]).expect_err("exact wildcard 必须被拒绝");
        assert!(error.to_string().contains("wildcard"), "{error}");

        let mut missing_fixture = exact_operation();
        missing_fixture.fixture = None;
        let error =
            audit_operation_entries(&[missing_fixture]).expect_err("schema_id 与 fixture 必须成对");
        assert!(error.to_string().contains("同时存在"), "{error}");

        let mut family = exact_operation();
        family.granularity = ContractGranularity::Family;
        let error = audit_operation_entries(&[family]).expect_err("ExactSurface family 必须被拒绝");
        assert!(error.to_string().contains("expected=exact"), "{error}");
    }

    #[test]
    fn operation_audit_requires_explicit_exclusion_boundary() {
        let mut empty_reason = exact_operation();
        empty_reason.schema_id = None;
        empty_reason.fixture = None;
        empty_reason.exclusion = Some(" ");
        let error = audit_operation_entries(&[empty_reason]).expect_err("空 exclusion 必须被拒绝");
        assert!(error.to_string().contains("不能为空"), "{error}");

        let mut mixed = exact_operation();
        mixed.exclusion = Some("测试排除");
        let error =
            audit_operation_entries(&[mixed]).expect_err("exclusion 不得和 schema/fixture 混用");
        assert!(error.to_string().contains("不得同时"), "{error}");
    }

    #[test]
    fn surface_audit_requires_unique_exact_ownership() {
        let exact = exact_operation();
        let operations = BTreeMap::from([(exact.id, &exact)]);

        audit_surface_entries(&[surface(vec![exact.id])], &operations)
            .expect("单一精确 ownership 必须有效");

        let error = audit_surface_entries(&[surface(vec![exact.id, exact.id])], &operations)
            .expect_err("重复 linkage 必须被拒绝");
        assert!(
            error.to_string().contains("重复 contract linkage"),
            "{error}"
        );

        let error = audit_surface_entries(&[], &operations)
            .expect_err("未绑定的 ExactSurface contract 必须被拒绝");
        assert!(error.to_string().contains("缺少 surface"), "{error}");
    }

    #[test]
    fn surface_audit_rejects_unknown_cross_surface_and_shared_only_links() {
        let exact = exact_operation();
        let operations = BTreeMap::from([(exact.id, &exact)]);

        let error = audit_surface_entries(&[surface(vec!["test.missing"])], &operations)
            .expect_err("未知 contract 必须被拒绝");
        assert!(error.to_string().contains("未知 contract"), "{error}");

        let mut wrong_surface = surface(vec![exact.id]);
        wrong_surface.surface = ContractSurface::Sse;
        let error = audit_surface_entries(&[wrong_surface], &operations)
            .expect_err("跨 surface linkage 必须被拒绝");
        assert!(error.to_string().contains("漂移"), "{error}");

        let shared = shared_operation();
        let shared_operations = BTreeMap::from([(shared.id, &shared)]);
        let error = audit_surface_entries(&[surface(vec![shared.id])], &shared_operations)
            .expect_err("非排除 surface 不能只有共享组件");
        assert!(error.to_string().contains("ExactSurface"), "{error}");
    }

    #[test]
    fn shared_components_require_explicit_surface_linkage() {
        let shared = shared_operation();
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let entry = surface(vec![shared.id]);

        audit_shared_component_links(std::slice::from_ref(&entry), &operations)
            .expect("显式 shared linkage 必须有效");

        let error = audit_shared_component_links(&[], &operations)
            .expect_err("孤立 shared component 必须被拒绝");
        assert!(error.to_string().contains("缺少显式"), "{error}");
    }

    #[test]
    fn excluded_surface_requires_reason_and_no_contracts() {
        let operations = BTreeMap::new();
        let excluded = SurfaceOperation {
            key: "GET /excluded".to_owned(),
            surface: ContractSurface::Api,
            contracts: Vec::new(),
            exclusion: Some("主机管理能力不进入领域协议"),
        };
        audit_surface_entries(&[excluded], &operations).expect("明确排除必须有效");

        let invalid = SurfaceOperation {
            key: "GET /excluded".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec!["test.exact.output"],
            exclusion: Some("测试排除"),
        };
        let error = audit_surface_entries(&[invalid], &operations)
            .expect_err("排除 surface 不得链接 contract");
        assert!(error.to_string().contains("excluded surface"), "{error}");
    }
}
