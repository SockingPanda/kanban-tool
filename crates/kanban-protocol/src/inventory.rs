use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractSurface {
    Api,
    Cli,
    Jsonl,
    Sse,
    Metadata,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractDirection {
    Serialize,
    Deserialize,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStrictness {
    DenyUnknownFields,
    Typed,
    OpaqueExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractGranularity {
    Exact,
    Family,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractBinding {
    ExactSurface,
    SharedComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpTransportLocation {
    Path,
    Query,
    Headers,
    Body,
    Success,
    Error,
    Sse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireParameterCardinality {
    RequiredOne,
    OptionalOne,
    RepeatedOrdered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WireParameter {
    pub name: &'static str,
    pub cardinality: Option<WireParameterCardinality>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContractTransport {
    NoTransport,
    Http {
        operation_key: Option<&'static str>,
        location: HttpTransportLocation,
        parameters: &'static [WireParameter],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationState {
    Planned,
    Generated,
    Adopted,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdoptionWitness {
    pub operation: &'static str,
    pub contract_id: &'static str,
    pub surface: ContractSurface,
    pub direction: ContractDirection,
    pub package: &'static str,
    pub test_target: &'static str,
    pub exact_test: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdoptionEvidence {
    pub producer_fixture: &'static str,
    pub producer: AdoptionWitness,
    pub consumer: AdoptionWitness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OperationContract {
    pub id: &'static str,
    pub path: &'static str,
    pub surface: ContractSurface,
    pub operation: &'static str,
    pub direction: ContractDirection,
    pub granularity: ContractGranularity,
    pub strictness: ContractStrictness,
    pub schema_id: Option<&'static str>,
    pub fixture: Option<&'static str>,
    pub adoption: Option<AdoptionEvidence>,
    pub exclusion: Option<&'static str>,
    pub migration: MigrationState,
    pub transport: ContractTransport,
    pub binding: ContractBinding,
}

// Operation contracts are projected from the canonical operation declaration source.
pub fn operation_inventory() -> &'static [OperationContract] {
    static INVENTORY: std::sync::OnceLock<Vec<OperationContract>> = std::sync::OnceLock::new();
    INVENTORY
        .get_or_init(|| {
            let mut inventory =
                crate::CatalogProjection::new(crate::operation_catalog::operation_catalog())
                    .contracts();
            inventory.extend(crate::metadata_config_catalog::shared_component_contracts());
            inventory.extend(crate::admin_catalog::template_contracts());
            reorder_contracts(
                &mut inventory,
                crate::board_catalog::HISTORICAL_CONTRACT_ORDER,
            );
            inventory
        })
        .as_slice()
}

fn reorder_contracts(inventory: &mut Vec<OperationContract>, order: &[&str]) {
    let mut ordered = Vec::with_capacity(inventory.len());
    for id in order {
        if let Some(index) = inventory.iter().position(|contract| contract.id == *id) {
            ordered.push(inventory.remove(index));
        }
    }
    ordered.append(inventory);
    *inventory = ordered;
}
