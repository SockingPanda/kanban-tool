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

use kanban_contract::{
    AdoptionWitness, ContractDirection, ContractGranularity, MigrationState, OperationContract,
    SurfaceOperation, endpoint_catalog, endpoint_obligation_todo_count, generated_artifacts,
    generated_schema_ids, operation_inventory,
    schema::{DRAFT_2020_12, SchemaRoot, canonicalize, schema_document, schema_registry},
    surface_operation_catalog, validate_endpoint_catalog, validate_operation_contracts,
};

pub type ToolResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
    direction: kanban_contract::ContractDirection,
    strictness: kanban_contract::ContractStrictness,
    schema_fixture: &'a str,
    invalid_fixture: &'a str,
    sha256: String,
}

pub fn audit_inventory(require_closed: bool) -> ToolResult<()> {
    validate_endpoint_catalog(endpoint_catalog(), require_closed).map_err(failure)?;
    let operations = audit_operation_entries(operation_inventory(), require_closed)?;
    let surfaces = surface_operation_catalog();
    audit_surface_entries(&surfaces, &operations, require_closed)?;
    audit_shared_component_witnesses(&surfaces, &operations)?;
    audit_schema_registry(&operations)
}

fn audit_operation_entries(
    entries: &[OperationContract],
    require_closed: bool,
) -> ToolResult<BTreeMap<&str, &OperationContract>> {
    let mut operations = BTreeMap::new();
    let mut unfinished = Vec::new();

    for operation in entries {
        if operation.id.is_empty() || operation.path.is_empty() || operation.operation.is_empty() {
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
        if require_closed && operation.granularity == ContractGranularity::Family {
            return Err(failure(format!(
                "closure 拒绝 family contract: {}",
                operation.id
            )));
        }
        if require_closed && operation.direction == ContractDirection::Bidirectional {
            return Err(failure(format!(
                "closure 拒绝 bidirectional contract，必须拆成 input/output: {}",
                operation.id
            )));
        }

        match operation.migration {
            MigrationState::Planned => {
                if operation.schema_id.is_some()
                    || operation.fixture.is_some()
                    || operation.adoption.is_some()
                    || operation.exclusion.is_some()
                {
                    return Err(failure(format!(
                        "planned contract 只能描述精确待迁移边界: {}",
                        operation.id
                    )));
                }
                unfinished.push(format!("planned:{}", operation.id));
            }
            MigrationState::Generated => {
                if operation.schema_id.is_none()
                    || operation.fixture.is_none()
                    || operation.adoption.is_some()
                    || operation.exclusion.is_some()
                {
                    return Err(failure(format!(
                        "generated contract 必须有 schema/fixture，且不能伪装 runtime adoption: {}",
                        operation.id
                    )));
                }
                unfinished.push(format!("generated:{}", operation.id));
            }
            MigrationState::Adopted => {
                if operation.schema_id.is_none()
                    || operation.fixture.is_none()
                    || operation.exclusion.is_some()
                {
                    return Err(failure(format!(
                        "adopted contract 必须有 schema/fixture 且不能有 exclusion: {}",
                        operation.id
                    )));
                }
                let evidence = operation.adoption.ok_or_else(|| {
                    failure(format!(
                        "adopted contract 缺少 producer/consumer evidence: {}",
                        operation.id
                    ))
                })?;
                if evidence.producer_fixture.trim().is_empty() {
                    return Err(failure(format!(
                        "adopted contract producer fixture 不能为空: {}",
                        operation.id
                    )));
                }
                audit_adoption_witness(operation, "producer", &evidence.producer)?;
                audit_adoption_witness(operation, "consumer", &evidence.consumer)?;
            }
            MigrationState::Excluded => {
                if operation.schema_id.is_some()
                    || operation.fixture.is_some()
                    || operation.adoption.is_some()
                    || operation
                        .exclusion
                        .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(failure(format!(
                        "excluded contract 必须只有明确 exclusion 理由: {}",
                        operation.id
                    )));
                }
            }
        }
    }

    validate_operation_contracts(entries).map_err(failure)?;

    if require_closed && !unfinished.is_empty() {
        return Err(failure(format!(
            "contract train 尚未闭合，只允许 adopted/excluded: {}",
            unfinished.join(", ")
        )));
    }

    Ok(operations)
}

fn audit_adoption_witness(
    contract: &OperationContract,
    role: &str,
    witness: &AdoptionWitness,
) -> ToolResult<()> {
    if witness.operation.trim().is_empty()
        || witness.contract_id.trim().is_empty()
        || witness.package.trim().is_empty()
        || witness.test_target.trim().is_empty()
        || witness.exact_test.trim().is_empty()
    {
        return Err(failure(format!(
            "adopted contract {role} witness 不完整: {}",
            contract.id
        )));
    }
    if witness.contract_id != contract.id {
        return Err(failure(format!(
            "adopted contract {role} witness contract_id 漂移: {} != {}",
            witness.contract_id, contract.id
        )));
    }
    if witness.surface != contract.surface {
        return Err(failure(format!(
            "adopted contract {role} witness surface 漂移: {:?} != {:?}",
            witness.surface, contract.surface
        )));
    }
    if witness.direction != contract.direction {
        return Err(failure(format!(
            "adopted contract {role} witness direction 漂移: {:?} != {:?}",
            witness.direction, contract.direction
        )));
    }
    Ok(())
}

fn audit_surface_entries(
    entries: &[SurfaceOperation],
    operations: &BTreeMap<&str, &OperationContract>,
    require_closed: bool,
) -> ToolResult<()> {
    let mut keys = BTreeSet::new();
    let mut exact_bindings = BTreeMap::new();
    let mut adopted_contracts = BTreeSet::new();
    let mut unfinished = Vec::new();

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

        if entry.migration == MigrationState::Excluded {
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
        if entry.exclusion.is_some() {
            return Err(failure(format!(
                "非 excluded surface 不得声明 exclusion: {}",
                entry.key
            )));
        }

        let mut exact_contracts = Vec::new();
        let mut shared_contracts = Vec::new();
        for contract_id in &entry.contracts {
            let contract = operations.get(contract_id).ok_or_else(|| {
                failure(format!(
                    "surface {} 链接未知 contract: {}",
                    entry.key, contract_id
                ))
            })?;
            if !matches!(
                contract.migration,
                MigrationState::Generated | MigrationState::Adopted
            ) {
                return Err(failure(format!(
                    "surface {} 链接非 generated/adopted contract: {}",
                    entry.key, contract_id
                )));
            }
            if contract.surface != entry.surface {
                return Err(failure(format!(
                    "contract 与 surface 漂移: {:?} {} -> {:?} {}",
                    entry.surface, entry.key, contract.surface, contract.id
                )));
            }
            match contract.binding {
                kanban_contract::ContractBinding::ExactSurface => exact_contracts.push(*contract),
                kanban_contract::ContractBinding::SharedComponent => {
                    shared_contracts.push(*contract)
                }
            }
        }

        match entry.migration {
            MigrationState::Planned => {
                if !exact_contracts.is_empty() {
                    return Err(failure(format!(
                        "planned surface 不能声明 ExactSurface schema coverage: {}",
                        entry.key
                    )));
                }
                unfinished.push(format!("planned:{:?}:{}", entry.surface, entry.key));
            }
            MigrationState::Generated => {
                if exact_contracts.is_empty() {
                    return Err(failure(format!(
                        "generated surface 必须链接至少一个 ExactSurface generated/adopted contract: {}",
                        entry.key
                    )));
                }
                unfinished.push(format!("generated:{:?}:{}", entry.surface, entry.key));
            }
            MigrationState::Adopted => {
                if exact_contracts.is_empty() {
                    return Err(failure(format!(
                        "adopted surface requires at least one ExactSurface contract: {}",
                        entry.key
                    )));
                }
                if exact_contracts
                    .iter()
                    .any(|contract| contract.migration != MigrationState::Adopted)
                {
                    return Err(failure(format!(
                        "adopted surface {} 链接的 exact contract 尚未 adopted",
                        entry.key
                    )));
                }
            }
            MigrationState::Excluded => unreachable!("excluded surface 已提前处理"),
        }

        for contract in exact_contracts {
            audit_exact_surface_binding(
                entry,
                contract,
                &mut exact_bindings,
                &mut adopted_contracts,
            )?;
        }

        let mut linked_shared = BTreeSet::new();
        for contract in shared_contracts {
            if !linked_shared.insert(contract.id) {
                return Err(failure(format!(
                    "surface 存在重复 shared component linkage: {} -> {}",
                    entry.key, contract.id
                )));
            }
        }
    }

    let expected_adopted = operations
        .values()
        .filter(|contract| {
            contract.migration == MigrationState::Adopted
                && contract.binding == kanban_contract::ContractBinding::ExactSurface
        })
        .map(|contract| contract.id)
        .collect::<BTreeSet<_>>();
    if adopted_contracts != expected_adopted {
        let missing = expected_adopted
            .difference(&adopted_contracts)
            .copied()
            .collect::<Vec<_>>();
        return Err(failure(format!(
            "adopted contract 缺少 adopted surface operation: {missing:?}"
        )));
    }

    if require_closed && !unfinished.is_empty() {
        return Err(failure(format!(
            "surface catalog 尚未闭合，只允许 adopted/excluded: {}",
            unfinished.join(", ")
        )));
    }
    Ok(())
}

fn audit_exact_surface_binding<'a>(
    entry: &SurfaceOperation,
    contract: &'a OperationContract,
    exact_bindings: &mut BTreeMap<&'a str, String>,
    adopted_contracts: &mut BTreeSet<&'a str>,
) -> ToolResult<()> {
    if contract.binding != kanban_contract::ContractBinding::ExactSurface {
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
        kanban_contract::ContractTransport::Http { operation_key, .. } => {
            if operation_key != Some(entry.key.as_str()) {
                return Err(failure(format!(
                    "exact contract operation 与 surface operation 漂移: {} -> {}",
                    contract.id, entry.key
                )));
            }
        }
        kanban_contract::ContractTransport::NoTransport => {
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

    if contract.migration == MigrationState::Adopted {
        let evidence = contract
            .adoption
            .expect("adopted contract evidence was checked before surface audit");
        for (role, witness) in [
            ("producer", &evidence.producer),
            ("consumer", &evidence.consumer),
        ] {
            if witness.operation != entry.key {
                return Err(failure(format!(
                    "adopted surface {role} witness operation 漂移: {} != {}",
                    witness.operation, entry.key
                )));
            }
        }
        adopted_contracts.insert(contract.id);
    }
    Ok(())
}

fn audit_shared_component_witnesses(
    entries: &[SurfaceOperation],
    operations: &BTreeMap<&str, &OperationContract>,
) -> ToolResult<()> {
    let surface_keys = entries
        .iter()
        .map(|entry| (entry.surface, entry.key.clone()))
        .collect::<BTreeSet<_>>();
    let linked_shared = entries
        .iter()
        .flat_map(|entry| {
            entry.contracts.iter().filter_map(|contract_id| {
                operations.get(contract_id).and_then(|contract| {
                    (contract.binding == kanban_contract::ContractBinding::SharedComponent
                        && contract.surface == entry.surface)
                        .then_some(contract.id)
                })
            })
        })
        .collect::<BTreeSet<_>>();

    for contract in operations.values().filter(|contract| {
        matches!(
            contract.migration,
            MigrationState::Generated | MigrationState::Adopted
        ) && contract.binding == kanban_contract::ContractBinding::SharedComponent
    }) {
        // orphan policy 是 OR：显式 linkage 已充分；否则 Adopted component 的两项
        // witness 都必须指向自身 surface 的真实 operation。
        if linked_shared.contains(contract.id) {
            continue;
        }
        if contract.migration == MigrationState::Adopted {
            let evidence = contract.adoption.ok_or_else(|| {
                failure(format!(
                    "shared component 缺少 adoption evidence: {}",
                    contract.id
                ))
            })?;
            for (role, witness) in [
                ("producer", &evidence.producer),
                ("consumer", &evidence.consumer),
            ] {
                if witness.surface != contract.surface
                    || !surface_keys.contains(&(contract.surface, witness.operation.to_owned()))
                {
                    return Err(failure(format!(
                        "shared component {role} witness operation 不是同 surface 的真实 catalog key: contract={} surface={:?} operation={}",
                        contract.id, contract.surface, witness.operation
                    )));
                }
            }
            continue;
        }

        return Err(failure(format!("orphan shared component: {}", contract.id)));
    }
    Ok(())
}

fn contract_binding_name(binding: kanban_contract::ContractBinding) -> &'static str {
    match binding {
        kanban_contract::ContractBinding::ExactSurface => "exact_surface",
        kanban_contract::ContractBinding::SharedComponent => "shared_component",
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
        .filter(|operation| {
            matches!(
                operation.migration,
                MigrationState::Generated | MigrationState::Adopted
            )
        })
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
                "schema root {} 未指向 generated/adopted contract {}",
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
            "generated/adopted contract 与 schema root 数量不一致: {} != {}",
            schema_operations.len(),
            schema_registry().len()
        )));
    }
    Ok(())
}

fn audit_root_mapping(root: &SchemaRoot, operation: &OperationContract) -> ToolResult<()> {
    let expected_id = operation
        .schema_id
        .expect("generated/adopted contract schema_id was checked before root mapping");
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
        .expect("generated/adopted contract fixture was checked before root mapping");
    if expected_fixture != root.valid_fixture {
        return Err(failure(format!(
            "contract {} 的 schema fixture 与 root 不一致: {} != {}",
            root.contract_id, expected_fixture, root.valid_fixture
        )));
    }
    Ok(())
}

pub fn write_generated(repo_root: &Path) -> ToolResult<()> {
    audit_inventory(false)?;
    validate_generated_schemas()?;
    validate_fixtures(repo_root)?;
    validate_adoption_evidence(repo_root)?;

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

pub fn check_contract(repo_root: &Path, require_closed: bool) -> ToolResult<()> {
    audit_inventory(require_closed)?;
    validate_generated_schemas()?;
    validate_fixtures(repo_root)?;
    validate_adoption_evidence(repo_root)?;
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

fn validate_adoption_evidence(repo_root: &Path) -> ToolResult<()> {
    for operation in operation_inventory()
        .iter()
        .filter(|operation| operation.migration == MigrationState::Adopted)
    {
        let evidence = operation
            .adoption
            .expect("adopted evidence was checked by inventory audit");
        let producer_fixture = repo_root.join(evidence.producer_fixture);
        if !producer_fixture.is_file() {
            return Err(failure(format!(
                "adopted contract 的真实 producer fixture 不存在: {} ({})",
                operation.id,
                producer_fixture.display()
            )));
        }
        let root = schema_registry()
            .iter()
            .find(|root| root.contract_id == operation.id)
            .ok_or_else(|| {
                failure(format!(
                    "adopted contract 没有 schema root: {}",
                    operation.id
                ))
            })?;
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(&schema_document(root))
            .map_err(|error| failure(format!("{} validator 编译失败: {error}", root.id)))?;
        let fixture = read_json(&producer_fixture)?;
        if let Err(error) = validator.validate(&fixture) {
            return Err(failure(format!(
                "真实 producer fixture 不符合 adopted schema {}: {error}",
                operation.id
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

pub fn unfinished_contract_count() -> usize {
    operation_inventory()
        .iter()
        .map(|operation| operation.migration)
        .chain(
            surface_operation_catalog()
                .iter()
                .map(|operation| operation.migration),
        )
        .filter(|state| matches!(state, MigrationState::Planned | MigrationState::Generated))
        .count()
        + endpoint_obligation_todo_count(endpoint_catalog())
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
    use kanban_contract::{
        AdoptionEvidence, AdoptionWitness, ContractStrictness, ContractSurface, ContractTransport,
        HttpTransportLocation, MigrationState,
    };

    fn exact_operation() -> OperationContract {
        OperationContract {
            id: "test.exact.output",
            path: "GET /test response",
            surface: ContractSurface::Api,
            operation: "test exact output",
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: None,
            fixture: None,
            adoption: None,
            exclusion: None,
            migration: MigrationState::Planned,
            transport: ContractTransport::Http {
                operation_key: Some("test exact output"),
                location: HttpTransportLocation::Success,
                parameters: &[],
            },
            binding: kanban_contract::ContractBinding::ExactSurface,
        }
    }

    fn adoption_witness(operation: &'static str) -> AdoptionWitness {
        AdoptionWitness {
            operation,
            contract_id: "test.exact.output",
            surface: ContractSurface::Api,
            direction: ContractDirection::Serialize,
            package: "kanban-server",
            test_target: "lib",
            exact_test: "tests::contract_witness",
        }
    }

    #[test]
    fn root_mapping_rejects_inventory_contract_drift() {
        let root = &schema_registry()[0];
        let operation = operation_inventory()
            .iter()
            .find(|operation| operation.id == root.contract_id)
            .expect("generated root must have an inventory operation");

        let mut direction_drift = *operation;
        direction_drift.direction = match operation.direction {
            ContractDirection::Serialize => ContractDirection::Deserialize,
            ContractDirection::Deserialize => ContractDirection::Serialize,
            ContractDirection::Bidirectional => ContractDirection::Serialize,
        };
        let error = audit_root_mapping(root, &direction_drift)
            .expect_err("direction drift must be rejected");
        assert!(error.to_string().contains("direction"));

        let mut fixture_drift = *operation;
        fixture_drift.fixture = Some("schemas/fixtures/api/not-the-root.json");
        let error =
            audit_root_mapping(root, &fixture_drift).expect_err("fixture drift must be rejected");
        assert!(error.to_string().contains("fixture"));
    }

    #[test]
    fn closure_rejects_generated_contracts() {
        let mut generated = operation_inventory()[0];
        generated.migration = MigrationState::Generated;
        generated.adoption = None;
        let error = audit_operation_entries(&[generated], true)
            .expect_err("generated is not equivalent to runtime adopted");
        assert!(error.to_string().contains("generated:"), "{error}");
    }

    #[test]
    fn closure_rejects_family_bidirectional_and_wildcard_shortcuts() {
        let mut family = exact_operation();
        family.granularity = ContractGranularity::Family;
        family.migration = MigrationState::Excluded;
        family.exclusion = Some("test-only exclusion");
        let error = audit_operation_entries(&[family], true)
            .expect_err("family shortcut must fail closure");
        assert!(error.to_string().contains("family"), "{error}");

        let mut bidirectional = exact_operation();
        bidirectional.direction = ContractDirection::Bidirectional;
        bidirectional.migration = MigrationState::Excluded;
        bidirectional.exclusion = Some("test-only exclusion");
        let error = audit_operation_entries(&[bidirectional], true)
            .expect_err("bidirectional shortcut must fail closure");
        assert!(error.to_string().contains("bidirectional"), "{error}");

        let mut wildcard = exact_operation();
        wildcard.path = "/api/v1/**";
        let error = audit_operation_entries(&[wildcard], false)
            .expect_err("exact wildcard must always fail");
        assert!(error.to_string().contains("wildcard"), "{error}");
    }

    #[test]
    fn adopted_contract_requires_complete_producer_and_consumer_evidence() {
        let mut adopted = exact_operation();
        adopted.migration = MigrationState::Adopted;
        adopted.schema_id = Some("urn:test");
        adopted.fixture = Some("schemas/fixtures/test.json");
        let error = audit_operation_entries(&[adopted], false)
            .expect_err("adopted without evidence must fail");
        assert!(error.to_string().contains("evidence"), "{error}");

        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "",
            producer: adoption_witness("test exact output"),
            consumer: adoption_witness("test exact output"),
        });
        let error = audit_operation_entries(&[adopted], false)
            .expect_err("empty producer fixture must fail");
        assert!(error.to_string().contains("producer fixture"), "{error}");
    }

    #[test]
    fn adopted_surface_rejects_contract_surface_drift() {
        let mut adopted = exact_operation();
        adopted.migration = MigrationState::Adopted;
        adopted.schema_id = Some("urn:test");
        adopted.fixture = Some("schemas/fixtures/test.json");
        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test.producer.json",
            producer: adoption_witness("test exact output"),
            consumer: adoption_witness("test exact output"),
        });

        let operations = BTreeMap::from([(adopted.id, &adopted)]);
        let surface = SurfaceOperation {
            key: "test exact output".to_owned(),
            surface: ContractSurface::Cli,
            contracts: vec!["test.exact.output"],
            migration: MigrationState::Adopted,
            exclusion: None,
        };

        let error = audit_surface_entries(&[surface], &operations, false)
            .expect_err("adopted surface 与 contract 的 surface 漂移必须失败");
        assert!(error.to_string().contains("surface"), "{error}");
    }

    #[test]
    fn adopted_contract_rejects_witness_identity_drift() {
        let mut adopted = exact_operation();
        adopted.migration = MigrationState::Adopted;
        adopted.schema_id = Some("urn:test");
        adopted.fixture = Some("schemas/fixtures/test.json");

        let mut wrong_contract = adoption_witness("test exact output");
        wrong_contract.contract_id = "test.other.output";
        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test.producer.json",
            producer: wrong_contract,
            consumer: adoption_witness("test exact output"),
        });
        let error = audit_operation_entries(&[adopted], false)
            .expect_err("witness contract_id 漂移必须失败");
        assert!(error.to_string().contains("contract_id"), "{error}");

        let mut wrong_direction = adoption_witness("test exact output");
        wrong_direction.direction = ContractDirection::Deserialize;
        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test.producer.json",
            producer: wrong_direction,
            consumer: adoption_witness("test exact output"),
        });
        let error =
            audit_operation_entries(&[adopted], false).expect_err("witness direction 漂移必须失败");
        assert!(error.to_string().contains("direction"), "{error}");

        let mut wrong_surface = adoption_witness("test exact output");
        wrong_surface.surface = ContractSurface::Cli;
        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test.producer.json",
            producer: wrong_surface,
            consumer: adoption_witness("test exact output"),
        });
        let error =
            audit_operation_entries(&[adopted], false).expect_err("witness surface 漂移必须失败");
        assert!(error.to_string().contains("surface"), "{error}");
    }

    #[test]
    fn adopted_surface_rejects_witness_operation_drift() {
        let mut adopted = exact_operation();
        adopted.migration = MigrationState::Adopted;
        adopted.schema_id = Some("urn:test");
        adopted.fixture = Some("schemas/fixtures/test.json");
        adopted.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test.producer.json",
            producer: adoption_witness("different operation"),
            consumer: adoption_witness("test exact output"),
        });
        let operations = BTreeMap::from([(adopted.id, &adopted)]);
        let surface = SurfaceOperation {
            key: "test exact output".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec!["test.exact.output"],
            migration: MigrationState::Adopted,
            exclusion: None,
        };

        let error = audit_surface_entries(&[surface], &operations, false)
            .expect_err("witness operation 漂移必须失败");
        assert!(error.to_string().contains("operation"), "{error}");
    }
    fn adopted_shared_operation(operation: &'static str) -> OperationContract {
        let mut c = exact_operation();
        c.id = "test.shared.output";
        c.operation = "shared output";
        c.binding = kanban_contract::ContractBinding::SharedComponent;
        c.transport = ContractTransport::Http {
            operation_key: None,
            location: HttpTransportLocation::Error,
            parameters: &[],
        };
        c.migration = MigrationState::Adopted;
        c.schema_id = Some("urn:test:shared");
        c.fixture = Some("schemas/fixtures/test-shared.json");
        c.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test-shared.json",
            producer: AdoptionWitness {
                operation,
                contract_id: c.id,
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
                exact_test: "tests::producer",
            },
            consumer: AdoptionWitness {
                operation,
                contract_id: c.id,
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
                exact_test: "tests::consumer",
            },
        });
        c
    }

    #[test]
    fn adopted_shared_component_with_real_witness_is_valid_without_explicit_linkage() {
        let shared = adopted_shared_operation("GET /api/v1/events");
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let surface = SurfaceOperation {
            key: "GET /api/v1/events".to_owned(),
            surface: ContractSurface::Api,
            contracts: Vec::new(),
            migration: MigrationState::Planned,
            exclusion: None,
        };

        audit_surface_entries(std::slice::from_ref(&surface), &operations, false)
            .expect("witness-only shared component 不进入 exact coverage");
        audit_shared_component_witnesses(&[surface], &operations)
            .expect("同 surface 的真实 adoption witness 足以避免 orphan");
    }

    #[test]
    fn generated_shared_component_with_explicit_linkage_is_valid_without_witness() {
        let mut shared = adopted_shared_operation("not-a-catalog-key");
        shared.migration = MigrationState::Generated;
        shared.adoption = None;
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let surface = SurfaceOperation {
            key: "GET /api/v1/events".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec!["test.shared.output"],
            migration: MigrationState::Planned,
            exclusion: None,
        };

        audit_surface_entries(std::slice::from_ref(&surface), &operations, false)
            .expect("合法 shared reference 不能被 exact-only audit 拒绝");
        audit_shared_component_witnesses(&[surface], &operations)
            .expect("显式 linkage 足以让无 witness 的 generated shared component 非 orphan");
    }

    #[test]
    fn orphan_generated_shared_component_is_rejected() {
        let mut shared = adopted_shared_operation("GET /api/v1/events");
        shared.migration = MigrationState::Generated;
        shared.adoption = None;
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let error = audit_shared_component_witnesses(&[], &operations)
            .expect_err("没有 linkage 或 witness 的 shared component 必须失败");
        assert!(
            error.to_string().contains("orphan shared component"),
            "{error}"
        );
    }

    #[test]
    fn shared_reference_does_not_enter_exact_adoption_set() {
        let exact = adopted_partial_input("POST /test");
        let shared = adopted_shared_operation("POST /test");
        let operations = BTreeMap::from([(exact.id, &exact), (shared.id, &shared)]);
        let surface = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![exact.id, shared.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };

        audit_surface_entries(std::slice::from_ref(&surface), &operations, false)
            .expect("shared component 不得被 exact-only adoption 集合误拒");
        audit_shared_component_witnesses(&[surface], &operations)
            .expect("shared component 显式 linkage 必须有效");
    }

    #[test]
    fn shared_component_cannot_impersonate_whole_adopted_surface() {
        let shared = adopted_shared_operation("GET /api/v1/events");
        assert!(matches!(
            shared.transport,
            ContractTransport::Http {
                location: HttpTransportLocation::Error,
                ..
            }
        ));
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let surface = SurfaceOperation {
            key: "GET /api/v1/events".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![shared.id],
            migration: MigrationState::Adopted,
            exclusion: None,
        };
        let error = audit_surface_entries(&[surface], &operations, false)
            .expect_err("shared component 不能把 endpoint 整体伪装为 Adopted");
        assert!(
            error
                .to_string()
                .contains("adopted surface requires at least one ExactSurface contract"),
            "{error}"
        );
    }

    #[test]
    fn exact_contract_second_surface_binding_is_rejected() {
        let exact = adopted_partial_input("POST /test");
        let operations = BTreeMap::from([(exact.id, &exact)]);
        let surface = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![exact.id, exact.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };
        let error = audit_surface_entries(&[surface], &operations, false)
            .expect_err("ExactSurface contract 第二次绑定必须失败");
        assert!(
            error.to_string().contains("contract=test.partial.input")
                && error.to_string().contains("first=POST /test")
                && error.to_string().contains("second=POST /test"),
            "{error}"
        );
    }

    #[test]
    fn shared_component_bad_witness_is_rejected_without_explicit_linkage() {
        let shared = adopted_shared_operation("HTTP JSON error envelope");
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let error = audit_shared_component_witnesses(&[], &operations)
            .expect_err("shared witness must name real surface key");
        assert!(error.to_string().contains("真实 catalog key"));
    }

    #[test]
    fn exact_adopted_contract_missing_surface_is_rejected() {
        let mut exact = exact_operation();
        exact.migration = MigrationState::Adopted;
        exact.schema_id = Some("urn:test:exact");
        exact.fixture = Some("schemas/fixtures/test-exact.json");
        exact.adoption = Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/test-exact.json",
            producer: adoption_witness("GET /test"),
            consumer: adoption_witness("GET /test"),
        });
        let operations = BTreeMap::from([(exact.id, &exact)]);
        let error = audit_surface_entries(&[], &operations, false)
            .expect_err("exact adopted contract requires a surface");
        assert!(error.to_string().contains("缺少 adopted surface"));
    }

    fn adopted_partial_input(operation: &'static str) -> OperationContract {
        OperationContract {
            id: "test.partial.input",
            path: "POST /test request",
            surface: ContractSurface::Api,
            operation: "test partial input",
            direction: ContractDirection::Deserialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:test:partial-input"),
            fixture: Some("schemas/fixtures/test-partial-input.json"),
            adoption: Some(AdoptionEvidence {
                producer_fixture: "schemas/fixtures/test-partial-input.json",
                producer: AdoptionWitness {
                    operation,
                    contract_id: "test.partial.input",
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: "tests::partial_input_producer",
                },
                consumer: AdoptionWitness {
                    operation,
                    contract_id: "test.partial.input",
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: "tests::partial_input_consumer",
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some(operation),
                location: HttpTransportLocation::Body,
                parameters: &[],
            },
            binding: kanban_contract::ContractBinding::ExactSurface,
        }
    }

    #[test]
    fn partial_generated_surface_accepts_adopted_body_contract() {
        let contract = adopted_partial_input("POST /test");
        let operations = BTreeMap::from([(contract.id, &contract)]);
        let surface = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![contract.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };

        audit_surface_entries(&[surface], &operations, false)
            .expect("partial endpoint 可以独立 adopted 已迁移的 body obligation");
    }

    #[test]
    fn partial_generated_surface_rejects_witness_operation_drift_and_orphan_contract() {
        let wrong_operation = adopted_partial_input("POST /other");
        let wrong_operations = BTreeMap::from([(wrong_operation.id, &wrong_operation)]);
        let surface = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![wrong_operation.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };
        let error = audit_surface_entries(&[surface], &wrong_operations, false)
            .expect_err("partial adoption witness operation 漂移必须失败");
        assert!(error.to_string().contains("operation"), "{error}");

        let orphan = adopted_partial_input("POST /test");
        let generated = OperationContract {
            id: "test.generated.output",
            path: "POST /test response",
            surface: ContractSurface::Api,
            operation: "test generated output",
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:test:generated-output"),
            fixture: Some("schemas/fixtures/test-generated-output.json"),
            adoption: None,
            exclusion: None,
            migration: MigrationState::Generated,
            transport: ContractTransport::Http {
                operation_key: Some("POST /test"),
                location: HttpTransportLocation::Success,
                parameters: &[],
            },
            binding: kanban_contract::ContractBinding::ExactSurface,
        };
        let orphan_operations = BTreeMap::from([(generated.id, &generated), (orphan.id, &orphan)]);
        let missing_reference = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![generated.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };
        let error = audit_surface_entries(&[missing_reference], &orphan_operations, false)
            .expect_err("descriptor 缺少 adopted body ref 时必须拒绝 orphan witness");
        let message = error.to_string();
        assert!(
            message.contains("adopted contract 缺少 adopted surface operation"),
            "{error}"
        );
        assert!(!message.contains("未知 contract"), "{error}");
    }

    #[test]
    fn generated_surface_error_names_generated_or_adopted_contracts() {
        let surface = SurfaceOperation {
            key: "POST /test".to_owned(),
            surface: ContractSurface::Api,
            contracts: Vec::new(),
            migration: MigrationState::Generated,
            exclusion: None,
        };
        let error = audit_surface_entries(&[surface], &BTreeMap::new(), false)
            .expect_err("generated surface 缺少 contract 必须失败");
        assert!(
            error.to_string().contains("generated/adopted contract"),
            "{error}"
        );
    }

    #[test]
    fn adopted_endpoint_body_contract_mutations_fail_closed() {
        let baseline = *kanban_contract::endpoint_descriptor("api.claim-task")
            .expect("claim endpoint descriptor");
        assert_eq!(baseline.migration, MigrationState::Adopted);
        assert!(matches!(
            baseline.obligations.body,
            kanban_contract::EndpointObligation::Contract("api.claim-task.request")
        ));

        for (mutation, contract_id, expected) in [
            ("id", "api.missing.request", "unknown contract"),
            ("direction", "api.health.response", "wrong direction"),
            ("surface", "metadata.decision.input", "wrong surface"),
        ] {
            let mut endpoint = baseline;
            endpoint.obligations.body = kanban_contract::EndpointObligation::Contract(contract_id);
            let error = validate_endpoint_catalog(&[endpoint], false)
                .expect_err("adopted endpoint body contract 漂移必须失败");
            assert!(error.contains(expected), "{mutation}: {error}");
        }
    }

    #[test]
    fn partial_generated_endpoint_cannot_pass_closed_audit_with_todo_obligations() {
        let mut endpoint = *kanban_contract::endpoint_descriptor("api.claim-task")
            .expect("claim endpoint descriptor");
        assert!(matches!(
            endpoint.obligations.body,
            kanban_contract::EndpointObligation::Contract("api.claim-task.request")
        ));
        endpoint.obligations.headers = kanban_contract::EndpointObligation::Todo;

        let error = validate_endpoint_catalog(&[endpoint], true)
            .expect_err("adopted body 不能掩盖其它 Todo obligation");
        assert!(
            error.contains("api.claim-task headers"),
            "closure error 必须定位具体 obligation: {error}"
        );
    }

    #[test]
    fn nonclosure_adopted_contract_and_exact_surface_reject_family_granularity() {
        let mut adopted_family = adopted_partial_input("POST /test");
        adopted_family.granularity = ContractGranularity::Family;
        let error = audit_operation_entries(&[adopted_family], false)
            .expect_err("普通非 closure audit 也必须拒绝 Adopted Family");
        let message = error.to_string();
        for expected in [
            "contract=test.partial.input",
            "binding=exact_surface",
            "expected=exact",
            "actual=family",
        ] {
            assert!(message.contains(expected), "{message}");
        }

        let mut generated_family = exact_operation();
        generated_family.migration = MigrationState::Generated;
        generated_family.granularity = ContractGranularity::Family;
        let operations = BTreeMap::from([(generated_family.id, &generated_family)]);
        let surface = SurfaceOperation {
            key: "test exact output".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![generated_family.id],
            migration: MigrationState::Generated,
            exclusion: None,
        };
        let error = audit_surface_entries(&[surface], &operations, false)
            .expect_err("synthetic surface 的 ExactSurface+Family reference 必须失败");
        let message = error.to_string();
        for expected in [
            "surface=test exact output",
            "contract=test.exact.output",
            "binding=exact_surface",
            "expected=exact",
            "actual=family",
        ] {
            assert!(message.contains(expected), "{message}");
        }
    }

    #[test]
    fn explicit_shared_linkage_is_sufficient_without_a_catalogued_witness_key() {
        let shared = adopted_shared_operation("not-a-catalog-key");
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let surface = SurfaceOperation {
            key: "GET /api/v1/events".to_owned(),
            surface: ContractSurface::Api,
            contracts: vec![shared.id],
            migration: MigrationState::Planned,
            exclusion: None,
        };

        audit_surface_entries(std::slice::from_ref(&surface), &operations, false)
            .expect("shared linkage 不得被 exact-only audit 拒绝");
        audit_shared_component_witnesses(&[surface], &operations)
            .expect("orphan policy 是显式 linkage OR 同 surface 有效 witness");
    }

    #[test]
    fn shared_component_can_link_multiple_planned_surfaces_without_exact_adoption() {
        let mut shared = adopted_shared_operation("not-a-catalog-key");
        shared.migration = MigrationState::Generated;
        shared.adoption = None;
        let operations = BTreeMap::from([(shared.id, &shared)]);
        let surfaces = [
            SurfaceOperation {
                key: "GET /api/v1/events".to_owned(),
                surface: ContractSurface::Api,
                contracts: vec![shared.id],
                migration: MigrationState::Planned,
                exclusion: None,
            },
            SurfaceOperation {
                key: "GET /api/v1/tasks".to_owned(),
                surface: ContractSurface::Api,
                contracts: vec![shared.id],
                migration: MigrationState::Planned,
                exclusion: None,
            },
        ];

        audit_surface_entries(&surfaces, &operations, false)
            .expect("跨 surface shared reuse 不得被误计为 exact coverage");
        audit_shared_component_witnesses(&surfaces, &operations)
            .expect("跨 surface 显式 linkage 必须满足 orphan policy");
        assert!(
            surfaces
                .iter()
                .all(|surface| surface.migration == MigrationState::Planned),
            "shared linkage 不能把 surface 提升为 Generated/Adopted"
        );
    }

    #[test]
    fn final_contract_train_closure_has_no_unfinished_authority() {
        assert_eq!(endpoint_obligation_todo_count(endpoint_catalog()), 0);
        assert_eq!(unfinished_contract_count(), 0);
    }
}
