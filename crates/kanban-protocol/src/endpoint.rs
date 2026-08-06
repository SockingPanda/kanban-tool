use serde::Serialize;

use crate::{ContractSurface, MigrationState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "contract_id")]
pub enum EndpointObligation {
    Contract(&'static str),
    NotApplicable,
    Excluded { reason: &'static str },
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndpointObligations {
    pub path: EndpointObligation,
    pub query: EndpointObligation,
    pub headers: EndpointObligation,
    pub body: EndpointObligation,
    pub success: EndpointObligation,
    pub sse: EndpointObligation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndpointDescriptor {
    pub operation_id: &'static str,
    pub surface: ContractSurface,
    pub method: HttpMethod,
    pub path: &'static str,
    pub migration: MigrationState,
    pub exclusion: Option<&'static str>,
    pub shared_components: &'static [&'static str],
    pub obligations: EndpointObligations,
}

const ENDPOINTS: &[EndpointDescriptor] = &[
    EndpointDescriptor {
        operation_id: "api.health",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/health",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.health.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.health.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-board-labels",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-board-labels.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-board-labels.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-board-labels.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-board-label",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/labels",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-board-label.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-board-label.headers"),
            body: EndpointObligation::Contract("api.create-board-label.request"),
            success: EndpointObligation::Contract("api.create-board-label.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-label-semantics",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/semantics",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-label-semantics.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-label-semantics.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-label-semantics.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-label-semantics",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/:label_id/semantics",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-label-semantics.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-label-semantics.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-label-semantics.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.upsert-label-semantics",
        surface: ContractSurface::Api,
        method: HttpMethod::Put,
        path: "/api/v1/boards/:board/labels/:label_id/semantics",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.upsert-label-semantics.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.upsert-label-semantics.headers"),
            body: EndpointObligation::Contract("api.upsert-label-semantics.request"),
            success: EndpointObligation::Contract("api.upsert-label-semantics.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.delete-label-semantics",
        surface: ContractSurface::Api,
        method: HttpMethod::Delete,
        path: "/api/v1/boards/:board/labels/:label_id/semantics",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.delete-label-semantics.path"),
            query: EndpointObligation::Contract("api.delete-label-semantics.query"),
            headers: EndpointObligation::Contract("api.delete-label-semantics.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.label-semantics-delete.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-label-atoms",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/atoms",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-label-atoms.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-label-atoms.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-label-atoms.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.explain-label-atom",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.label-atom.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.explain-label-atom.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.explain-label-atom.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.label-atom-index-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/atom-index/status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.label-atom-index-status.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.label-atom-index-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.label-atom-index-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.rebuild-label-atom-index",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/labels/atom-index/rebuild",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.rebuild-label-atom-index.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.rebuild-label-atom-index.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.rebuild-label-atom-index.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.query-label-atom-index",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/labels/atom-index/query",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.query-label-atom-index.path"),
            query: EndpointObligation::Contract("api.query-label-atom-index.query"),
            headers: EndpointObligation::Contract("api.query-label-atom-index.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.query-label-atom-index.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-tasks",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/tasks",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-tasks.path"),
            query: EndpointObligation::Contract("api.list-tasks.query"),
            headers: EndpointObligation::Contract("api.list-tasks.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-tasks.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-tasks-by-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/tasks/by-status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-tasks-by-status.path"),
            query: EndpointObligation::Contract("api.list-tasks-by-status.query"),
            headers: EndpointObligation::Contract("api.list-tasks-by-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-tasks-by-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/tasks",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-task.headers"),
            body: EndpointObligation::Contract("api.create-task.request"),
            success: EndpointObligation::Contract("api.create-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/signals",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-signals.path"),
            query: EndpointObligation::Contract("api.list-signals.query"),
            headers: EndpointObligation::Contract("api.list-signals.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.review-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/signals/review",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.review-signals.path"),
            query: EndpointObligation::Contract("api.review-signals.query"),
            headers: EndpointObligation::Contract("api.review-signals.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.review-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-signal",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/signals/:signal_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-signal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-signal.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-signal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.record-signal",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/signals",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.record-signal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.record-signal.headers"),
            body: EndpointObligation::Contract("api.record-signal.request"),
            success: EndpointObligation::Contract("api.record-signal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.confirm-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/signals/confirm",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.confirm-signals.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.confirm-signals.headers"),
            body: EndpointObligation::Contract("api.confirm-signals.request"),
            success: EndpointObligation::Contract("api.confirm-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.reject-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/signals/reject",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.reject-signals.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.reject-signals.headers"),
            body: EndpointObligation::Contract("api.reject-signals.request"),
            success: EndpointObligation::Contract("api.reject-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.resolve-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/signals/resolve",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.resolve-signals.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.resolve-signals.headers"),
            body: EndpointObligation::Contract("api.resolve-signals.request"),
            success: EndpointObligation::Contract("api.resolve-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.supersede-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/signals/supersede",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.supersede-signals.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.supersede-signals.headers"),
            body: EndpointObligation::Contract("api.supersede-signals.request"),
            success: EndpointObligation::Contract("api.supersede-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.board-task-map",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/task-map",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.board-task-map.path"),
            query: EndpointObligation::Contract("api.board-task-map.query"),
            headers: EndpointObligation::Contract("api.board-task-map.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.board-task-map.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-task.path"),
            query: EndpointObligation::Contract("api.get-task.query"),
            headers: EndpointObligation::Contract("api.get-task.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.update-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Patch,
        path: "/api/v1/tasks/:task_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.update-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.update-task.headers"),
            body: EndpointObligation::Contract("api.update-task.request"),
            success: EndpointObligation::Contract("api.update-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.task-neighborhood",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/neighborhood",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.task-neighborhood.path"),
            query: EndpointObligation::Contract("api.task-neighborhood.query"),
            headers: EndpointObligation::Contract("api.task-neighborhood.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.task-neighborhood.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-task-labels",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/labels",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-task-labels.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-task-labels.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-task-labels.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.add-task-label",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/labels",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.add-task-label.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.add-task-label.headers"),
            body: EndpointObligation::Contract("api.add-task-label.request"),
            success: EndpointObligation::Contract("api.add-task-label.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.suggest-task-labels",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/labels/suggestions",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.suggest-task-labels.path"),
            query: EndpointObligation::Contract("api.label-suggestion.query"),
            headers: EndpointObligation::Contract("api.suggest-task-labels.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.suggest-task-labels.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-task-label-proposals",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/label-proposals",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-task-label-proposals.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-task-label-proposals.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-task-label-proposals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.propose-task-label",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/label-proposals",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.propose-task-label.path"),
            query: EndpointObligation::Contract("api.propose-task-label.query"),
            headers: EndpointObligation::Contract("api.propose-task-label.headers"),
            body: EndpointObligation::Contract("api.propose-task-label.request"),
            success: EndpointObligation::Contract("api.propose-task-label.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.record-label-ontology-observation",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/label-ontology/observations",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.record-label-ontology-observation.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.record-label-ontology-observation.headers"),
            body: EndpointObligation::Contract("api.record-label-ontology-observation.body"),
            success: EndpointObligation::Contract("api.record-label-ontology-observation.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-label-ontology-signals",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/label-ontology/signals",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-label-ontology-signals.path"),
            query: EndpointObligation::Contract("api.label-ontology-signal.query"),
            headers: EndpointObligation::Contract("api.list-label-ontology-signals.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-label-ontology-signals.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.review-label-ontology",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/boards/:board/label-ontology/review",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.review-label-ontology.path"),
            query: EndpointObligation::Contract("api.label-ontology-review.query"),
            headers: EndpointObligation::Contract("api.review-label-ontology.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.review-label-ontology.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-label-ontology-action",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/label-ontology/actions",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-label-ontology-action.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-label-ontology-action.headers"),
            body: EndpointObligation::Contract("api.create-label-ontology-action.request"),
            success: EndpointObligation::Contract("api.create-label-ontology-action.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.apply-label-ontology-atom",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/label-ontology/apply/atom",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.apply-label-ontology-atom.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.apply-label-ontology-atom.headers"),
            body: EndpointObligation::Contract("api.apply-label-ontology-atom.request"),
            success: EndpointObligation::Contract("api.apply-label-ontology-atom.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.revert-label-ontology-mutation",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/label-ontology/revert",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.revert-label-ontology-mutation.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.revert-label-ontology-mutation.headers"),
            body: EndpointObligation::Contract("api.revert-label-ontology-mutation.request"),
            success: EndpointObligation::Contract("api.revert-label-ontology-mutation.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.validate-label-ontology-action",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/boards/:board/label-ontology/validate",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.validate-label-ontology-action.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.validate-label-ontology-action.headers"),
            body: EndpointObligation::Contract("api.validate-label-ontology-action.request"),
            success: EndpointObligation::Contract("api.validate-label-ontology-action.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-label-ontology-signal",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/label-ontology/signals/:signal_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-label-ontology-signal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-label-ontology-signal.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-label-ontology-signal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-label-proposal",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/label-proposals/:proposal_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-label-proposal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-label-proposal.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-label-proposal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.accept-label-proposal",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/label-proposals/:proposal_id/accept",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.accept-label-proposal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.accept-label-proposal.headers"),
            body: EndpointObligation::Contract("api.accept-label-proposal.body"),
            success: EndpointObligation::Contract("api.accept-label-proposal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.reject-label-proposal",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/label-proposals/:proposal_id/reject",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.reject-label-proposal.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.reject-label-proposal.headers"),
            body: EndpointObligation::Contract("api.reject-label-proposal.body"),
            success: EndpointObligation::Contract("api.reject-label-proposal.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.remove-task-label",
        surface: ContractSurface::Api,
        method: HttpMethod::Delete,
        path: "/api/v1/tasks/:task_id/labels/:label_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.remove-task-label.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.remove-task-label.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.remove-task-label.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.specify-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/specify",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.specify-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.specify-task.headers"),
            body: EndpointObligation::Contract("api.specify-task.request"),
            success: EndpointObligation::Contract("api.specify-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.promote-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/promote",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.promote-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.promote-task.headers"),
            body: EndpointObligation::Contract("api.promote-task.request"),
            success: EndpointObligation::Contract("api.promote-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.claim-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/claim",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.claim-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.claim-task.headers"),
            body: EndpointObligation::Contract("api.claim-task.request"),
            success: EndpointObligation::Contract("api.claim-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.reopen-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/reopen",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.reopen-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.reopen-task.headers"),
            body: EndpointObligation::Contract("api.reopen-task.request"),
            success: EndpointObligation::Contract("api.reopen-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.reclaim-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/reclaim",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.reclaim-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.reclaim-task.headers"),
            body: EndpointObligation::Contract("api.reclaim-task.request"),
            success: EndpointObligation::Contract("api.reclaim-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.heartbeat-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/heartbeat",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.heartbeat-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.heartbeat-task.headers"),
            body: EndpointObligation::Contract("api.heartbeat-task.request"),
            success: EndpointObligation::Contract("api.heartbeat-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.release-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/release",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.release-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.release-task.headers"),
            body: EndpointObligation::Contract("api.release-task.request"),
            success: EndpointObligation::Contract("api.release-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.complete-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/complete",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.complete-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.complete-task.headers"),
            body: EndpointObligation::Contract("api.complete-task.request"),
            success: EndpointObligation::Contract("api.complete-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.submit-review-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/submit-review",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.submit-review-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.submit-review-task.headers"),
            body: EndpointObligation::Contract("api.submit-review-task.request"),
            success: EndpointObligation::Contract("api.submit-review-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.block-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/block",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.block-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.block-task.headers"),
            body: EndpointObligation::Contract("api.block-task.request"),
            success: EndpointObligation::Contract("api.block-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.unblock-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/unblock",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.unblock-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.unblock-task.headers"),
            body: EndpointObligation::Contract("api.unblock-task.request"),
            success: EndpointObligation::Contract("api.unblock-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.archive-task",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/transitions/archive",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.archive-task.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.archive-task.headers"),
            body: EndpointObligation::Contract("api.archive-task.request"),
            success: EndpointObligation::Contract("api.archive-task.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-dependencies",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/dependencies",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-dependencies.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-dependencies.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-dependencies.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.add-dependency",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/dependencies",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.add-dependency.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.add-dependency.headers"),
            body: EndpointObligation::Contract("api.add-dependency.request"),
            success: EndpointObligation::Contract("api.add-dependency.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.remove-dependency",
        surface: ContractSurface::Api,
        method: HttpMethod::Delete,
        path: "/api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.remove-dependency.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.remove-dependency.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.remove-dependency.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-steps",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/steps",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-steps.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-steps.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-steps.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/steps",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-step.headers"),
            body: EndpointObligation::Contract("api.create-step.request"),
            success: EndpointObligation::Contract("api.create-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.update-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Patch,
        path: "/api/v1/tasks/:task_id/steps/:step_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.update-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.update-step.headers"),
            body: EndpointObligation::Contract("api.update-step.request"),
            success: EndpointObligation::Contract("api.update-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.remove-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Delete,
        path: "/api/v1/tasks/:task_id/steps/:step_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.remove-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.remove-step.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.remove-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.complete-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/steps/:step_id/done",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.complete-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.complete-step.headers"),
            body: EndpointObligation::Contract("api.complete-step.request"),
            success: EndpointObligation::Contract("api.complete-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.skip-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/steps/:step_id/skip",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.skip-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.skip-step.headers"),
            body: EndpointObligation::Contract("api.skip-step.request"),
            success: EndpointObligation::Contract("api.skip-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.reopen-step",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/steps/:step_id/reopen",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.reopen-step.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.reopen-step.headers"),
            body: EndpointObligation::Contract("api.reopen-step.request"),
            success: EndpointObligation::Contract("api.reopen-step.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.mark-execution-plan-not-required",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/execution-plan/not-required",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.mark-execution-plan-not-required.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.mark-execution-plan-not-required.headers"),
            body: EndpointObligation::Contract("api.mark-execution-plan-not-required.request"),
            success: EndpointObligation::Contract("api.mark-execution-plan-not-required.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-runs",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/runs",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-runs.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-runs.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-runs.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-run",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/runs/:run_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-run.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-run.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-run.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-run-log",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/runs/:run_id/log",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.get-run-log.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.get-run-log.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-run-log.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-comments",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/comments",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-comments.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-comments.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-comments.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-comment",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/comments",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &["api.error.response"],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-comment.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-comment.headers"),
            body: EndpointObligation::Contract("api.create-comment.request"),
            success: EndpointObligation::Contract("api.create-comment.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-attachments",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/attachments",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.list-attachments.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.list-attachments.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-attachments.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.create-attachment",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/tasks/:task_id/attachments",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.create-attachment.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.create-attachment.headers"),
            body: EndpointObligation::Contract("api.create-attachment.request"),
            success: EndpointObligation::Contract("api.create-attachment.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.download-attachment",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/attachments/:attachment_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.download-attachment.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.download-attachment.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.download-attachment.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.delete-attachment",
        surface: ContractSurface::Api,
        method: HttpMethod::Delete,
        path: "/api/v1/tasks/:task_id/attachments/:attachment_id",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.delete-attachment.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.delete-attachment.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.delete-attachment.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-stats",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/stats",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.get-stats.query"),
            headers: EndpointObligation::Contract("api.get-stats.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.get-stats.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.search-tasks",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/search/tasks",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.search-tasks.query"),
            headers: EndpointObligation::Contract("api.search-tasks.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.search-tasks.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.search-tasks-by-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/search/tasks/by-status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.search-tasks-by-status.query"),
            headers: EndpointObligation::Contract("api.search-tasks-by-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.search-tasks-by-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.search-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/search/status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.search-status.query"),
            headers: EndpointObligation::Contract("api.search-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.search-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.rebuild-search-index",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/search/index/rebuild",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.rebuild-search-index.query"),
            headers: EndpointObligation::Contract("api.rebuild-search-index.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.rebuild-search-index.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.sync-search-index",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/search/index/sync",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.sync-search-index.query"),
            headers: EndpointObligation::Contract("api.sync-search-index.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.sync-search-index.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.build-context",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/tasks/:task_id/context",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.build-context.path"),
            query: EndpointObligation::Contract("api.build-context.query"),
            headers: EndpointObligation::Contract("api.build-context.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.build-context.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.graph-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/graph/status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.graph-status.query"),
            headers: EndpointObligation::Contract("api.graph-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.graph-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.graph-neighbors",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/graph/neighbors",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.graph-neighbors.query"),
            headers: EndpointObligation::Contract("api.graph-neighbors.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.graph-neighbors.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.graph-query",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/graph/query",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.graph-query.query"),
            headers: EndpointObligation::Contract("api.graph-query.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.graph-query.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.graph-rebuild",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/graph/rebuild",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.graph-rebuild.query"),
            headers: EndpointObligation::Contract("api.graph-rebuild.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.graph-rebuild.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.graph-sync",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/graph/sync",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.graph-sync.query"),
            headers: EndpointObligation::Contract("api.graph-sync.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.graph-sync.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-entities",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/entities",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.entity-list.query"),
            headers: EndpointObligation::Contract("api.entity-list.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.entity-list.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.upsert-entity",
        surface: ContractSurface::Api,
        method: HttpMethod::Put,
        path: "/api/v1/entities",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.upsert-entity.headers"),
            body: EndpointObligation::Contract("api.entity-upsert.request"),
            success: EndpointObligation::Contract("api.entity-upsert.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.get-entity",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/entities/:uri",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::Contract("api.entity.path"),
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.entity.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.entity.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/vector/status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.vector-status.query"),
            headers: EndpointObligation::Contract("api.vector-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.vector-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-configure",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/vector/configure",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.vector-configure.headers"),
            body: EndpointObligation::Contract("api.vector-configure.request"),
            success: EndpointObligation::Contract("api.vector-configure.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-rebuild",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/vector/rebuild",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.vector-rebuild.headers"),
            body: EndpointObligation::Contract("api.vector-rebuild.request"),
            success: EndpointObligation::Contract("api.vector-rebuild.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-sync",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/vector/sync",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.vector-sync.headers"),
            body: EndpointObligation::Contract("api.vector-sync.request"),
            success: EndpointObligation::Contract("api.vector-sync.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-query-chunks",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/vector/query-chunks",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.vector-query-chunks.query"),
            headers: EndpointObligation::Contract("api.vector-query-chunks.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.vector-query-chunks.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.vector-query-label-atoms",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/vector/query-label-atoms",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.vector-query-label-atoms.query"),
            headers: EndpointObligation::Contract("api.vector-query-label-atoms.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.vector-query-label-atoms.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.list-events",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/events",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("api.list-events.query"),
            headers: EndpointObligation::Contract("api.list-events.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.list-events.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "sse.stream-events",
        surface: ContractSurface::Sse,
        method: HttpMethod::Get,
        path: "/api/v1/stream/events",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::Contract("sse.stream-events.query"),
            headers: EndpointObligation::Excluded {
                reason: "V1 finite snapshot intentionally ignores Last-Event-ID; after query owns the cursor",
            },
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::NotApplicable,
            sse: EndpointObligation::Contract("sse.event.data"),
        },
    },
    EndpointDescriptor {
        operation_id: "api.doctor",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/maintenance/doctor",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.doctor.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.doctor.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.checkpoint",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/checkpoint",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.checkpoint.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.checkpoint.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-backup",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/backup",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-backup.headers"),
            body: EndpointObligation::Contract("api.maintenance-backup.request"),
            success: EndpointObligation::Contract("api.maintenance-backup.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-export",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/export",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-export.headers"),
            body: EndpointObligation::Contract("api.maintenance-export.request"),
            success: EndpointObligation::Contract("api.maintenance-export.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-import",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/import",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-import.headers"),
            body: EndpointObligation::Contract("api.maintenance-import.request"),
            success: EndpointObligation::Contract("api.maintenance-import.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-vacuum",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/vacuum",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-vacuum.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.maintenance-vacuum.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-status",
        surface: ContractSurface::Api,
        method: HttpMethod::Get,
        path: "/api/v1/maintenance/status",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-status.headers"),
            body: EndpointObligation::NotApplicable,
            success: EndpointObligation::Contract("api.maintenance-status.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-run",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/run",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-run.headers"),
            body: EndpointObligation::Contract("api.maintenance-run.request"),
            success: EndpointObligation::Contract("api.maintenance-run.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-rebuild",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/rebuild",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-rebuild.headers"),
            body: EndpointObligation::Contract("api.maintenance-rebuild.request"),
            success: EndpointObligation::Contract("api.maintenance-rebuild.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-cleanup",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/cleanup",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-cleanup.headers"),
            body: EndpointObligation::Contract("api.maintenance-cleanup.request"),
            success: EndpointObligation::Contract("api.maintenance-cleanup.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
    EndpointDescriptor {
        operation_id: "api.maintenance-import-v30",
        surface: ContractSurface::Api,
        method: HttpMethod::Post,
        path: "/api/v1/maintenance/import-v30",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        obligations: EndpointObligations {
            path: EndpointObligation::NotApplicable,
            query: EndpointObligation::NotApplicable,
            headers: EndpointObligation::Contract("api.maintenance-import-v30.headers"),
            body: EndpointObligation::Contract("api.maintenance-import-v30.request"),
            success: EndpointObligation::Contract("api.maintenance-import-v30.response"),
            sse: EndpointObligation::NotApplicable,
        },
    },
];

pub fn endpoint_catalog() -> &'static [EndpointDescriptor] {
    static CATALOG: std::sync::OnceLock<Vec<EndpointDescriptor>> = std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let board = crate::board_catalog::endpoint_catalog();
            let knowledge = crate::knowledge_catalog::endpoint_catalog();
            let mut catalog = Vec::with_capacity(
                ENDPOINTS.len()
                    + board.len()
                    + knowledge.len()
                    + crate::admin_catalog::endpoint_catalog().len()
                    + 40,
            );
            for endpoint in ENDPOINTS {
                if let Some(admin) =
                    crate::admin_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(admin);
                    if endpoint.operation_id == "api.health" {
                        catalog.extend(board.iter().copied());
                    }
                    continue;
                }
                if let Some(history) =
                    crate::history_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(history);
                    continue;
                }
                if let Some(dependency) =
                    crate::dependency_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(dependency);
                    continue;
                }
                if let Some(step) = crate::step_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(step);
                    continue;
                }
                if let Some(task) = crate::task_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(task);
                    continue;
                }
                if let Some(labels) =
                    crate::labels_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(labels);
                    if endpoint.operation_id == "api.list-board-labels" {
                        if let Some(board_proposals) = crate::labels_catalog::endpoint_descriptor(
                            "api.list-board-label-proposals",
                        ) {
                            catalog.push(board_proposals);
                        }
                    }
                    continue;
                }
                if let Some(knowledge) =
                    crate::knowledge_catalog::endpoint_descriptor(endpoint.operation_id)
                {
                    catalog.push(knowledge);
                    continue;
                }
                catalog.push(*endpoint);
                if endpoint.operation_id == "api.health" {
                    catalog.extend(board.iter().copied());
                }
            }
            catalog
        })
        .as_slice()
}

pub fn endpoint_descriptor(operation_id: &str) -> Option<&'static EndpointDescriptor> {
    endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

pub fn endpoint_obligation_todo_count(catalog: &[EndpointDescriptor]) -> usize {
    catalog
        .iter()
        .flat_map(|endpoint| {
            endpoint
                .obligations
                .entries()
                .into_iter()
                .map(|(_, obligation)| obligation)
        })
        .filter(|obligation| matches!(obligation, EndpointObligation::Todo))
        .count()
}

pub fn validate_endpoint_catalog(
    catalog: &[EndpointDescriptor],
    require_closed: bool,
) -> Result<(), String> {
    validate_contract_topology(catalog, crate::operation_inventory(), require_closed)
}

pub fn validate_operation_contracts(
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    validate_contract_inventory_transport(contract_inventory)?;
    validate_adopted_contract_granularity(contract_inventory)
}

fn validate_contract_inventory_transport(
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    let mut contract_ids = std::collections::BTreeMap::new();
    for (index, contract) in contract_inventory.iter().enumerate() {
        if let Some(first_index) = contract_ids.insert(contract.id, index) {
            return Err(format!(
                "duplicate operation contract id: contract={} first={} second={}",
                contract.id, first_index, index
            ));
        }
        validate_contract_transport(contract)?;
    }
    Ok(())
}

fn validate_adopted_contract_granularity(
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    for contract in contract_inventory {
        if contract.migration == MigrationState::Adopted
            && contract.granularity != crate::ContractGranularity::Exact
        {
            return Err(format!(
                "adopted contract granularity mismatch: contract={} binding={} expected=exact actual={}",
                contract.id,
                contract_binding_name(contract.binding),
                contract_granularity_name(contract.granularity)
            ));
        }
    }
    Ok(())
}

pub fn validate_contract_topology(
    catalog: &[EndpointDescriptor],
    contract_inventory: &[crate::OperationContract],
    require_closed: bool,
) -> Result<(), String> {
    // endpoint 专属诊断先于全 catalog Adopted 不变量执行，使被引用的 Family contract
    // 能精确指出无效的 endpoint obligation。
    validate_contract_inventory_transport(contract_inventory)?;

    let mut operation_ids = std::collections::BTreeMap::new();
    let mut method_paths = std::collections::BTreeMap::new();
    let contracts = contract_inventory
        .iter()
        .map(|contract| (contract.id, contract))
        .collect::<std::collections::BTreeMap<_, _>>();

    for endpoint in catalog {
        if endpoint.operation_id.is_empty() || endpoint.path.is_empty() {
            return Err("endpoint descriptor contains empty operation_id/path".to_owned());
        }
        if !matches!(
            endpoint.surface,
            ContractSurface::Api | ContractSurface::Sse
        ) {
            return Err(format!(
                "endpoint has non-transport surface: endpoint={} expected=api_or_sse actual={}",
                endpoint.operation_id,
                contract_surface_name(endpoint.surface)
            ));
        }
        if let Some((first_method, first_path)) =
            operation_ids.insert(endpoint.operation_id, (endpoint.method, endpoint.path))
        {
            return Err(format!(
                "duplicate endpoint operation_id: operation_id={} first={} {} second={} {}",
                endpoint.operation_id,
                http_method_name(first_method),
                first_path,
                http_method_name(endpoint.method),
                endpoint.path
            ));
        }
        if let Some(first_operation) =
            method_paths.insert((endpoint.method, endpoint.path), endpoint.operation_id)
        {
            return Err(format!(
                "duplicate endpoint method/path: expected=unique actual={} {} first={} second={}",
                http_method_name(endpoint.method),
                endpoint.path,
                first_operation,
                endpoint.operation_id
            ));
        }
        if endpoint
            .exclusion
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(format!(
                "endpoint exclusion reason must be non-empty: {}",
                endpoint.operation_id
            ));
        }

        for (kind, obligation) in endpoint.obligations.entries() {
            validate_obligation(endpoint, kind, obligation, &contracts, require_closed)?;
        }
        validate_shared_component_links(endpoint, &contracts)?;
        validate_endpoint_migration_honesty(endpoint, &contracts)?;
    }

    validate_adopted_contract_granularity(contract_inventory)
}

fn validate_contract_transport(contract: &crate::OperationContract) -> Result<(), String> {
    use crate::{
        ContractBinding, ContractDirection, ContractSurface, ContractTransport,
        HttpTransportLocation, WireParameterCardinality,
    };

    let (operation_key, location, parameters) = match contract.transport {
        ContractTransport::NoTransport => {
            if matches!(
                contract.surface,
                ContractSurface::Api | ContractSurface::Sse
            ) {
                return Err(format!(
                    "HTTP contract must declare transport metadata: {}",
                    contract.id
                ));
            }
            return Ok(());
        }
        ContractTransport::Http {
            operation_key,
            location,
            parameters,
        } => {
            if !matches!(
                contract.surface,
                ContractSurface::Api | ContractSurface::Sse
            ) {
                return Err(format!(
                    "non-HTTP contract must declare no_transport: {}",
                    contract.id
                ));
            }
            (operation_key, location, parameters)
        }
    };

    if contract.surface == ContractSurface::Api && location == HttpTransportLocation::Sse {
        return Err(format!(
            "transport location sse is incompatible with api surface: {}",
            contract.id
        ));
    }
    if location == HttpTransportLocation::Error
        && contract.binding != ContractBinding::SharedComponent
    {
        return Err(format!(
            "error transport requires SharedComponent binding: contract={} location=error expected=shared_component actual={}",
            contract.id,
            contract_binding_name(contract.binding)
        ));
    }
    match contract.binding {
        ContractBinding::ExactSurface if operation_key.is_none_or(|key| key.trim().is_empty()) => {
            return Err(format!(
                "ExactSurface HTTP contract must name an operation_key: {}",
                contract.id
            ));
        }
        ContractBinding::SharedComponent if operation_key.is_some() => {
            return Err(format!(
                "SharedComponent HTTP contract must not claim an exact operation_key: {}",
                contract.id
            ));
        }
        _ => {}
    }

    let expected_direction = match location {
        HttpTransportLocation::Path
        | HttpTransportLocation::Query
        | HttpTransportLocation::Headers
        | HttpTransportLocation::Body => ContractDirection::Deserialize,
        HttpTransportLocation::Success
        | HttpTransportLocation::Error
        | HttpTransportLocation::Sse => ContractDirection::Serialize,
    };
    if contract.direction != expected_direction {
        return Err(format!(
            "contract transport direction does not match location {}: contract={} location={} expected={} actual={}",
            transport_location_name(location),
            contract.id,
            transport_location_name(location),
            contract_direction_name(expected_direction),
            contract_direction_name(contract.direction)
        ));
    }

    if !matches!(
        location,
        HttpTransportLocation::Path | HttpTransportLocation::Query | HttpTransportLocation::Headers
    ) && !parameters.is_empty()
    {
        return Err(format!(
            "transport parameters forbidden: contract={} location={} expected=none actual_count={}",
            contract.id,
            transport_location_name(location),
            parameters.len()
        ));
    }

    let mut names = std::collections::BTreeMap::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let name = parameter.name.trim();
        if name.is_empty() {
            return Err(format!(
                "wire parameter name must be non-empty: contract={} location={} parameter_index={} expected=non-empty actual={:?}",
                contract.id,
                transport_location_name(location),
                index,
                parameter.name
            ));
        }
        if name != parameter.name {
            return Err(format!(
                "wire parameter name must not contain surrounding whitespace: contract={} location={} parameter_index={} expected=without_surrounding_whitespace actual={:?}",
                contract.id,
                transport_location_name(location),
                index,
                parameter.name
            ));
        }
        let identity = if location == HttpTransportLocation::Headers {
            name.to_ascii_lowercase()
        } else {
            name.to_owned()
        };
        if let Some((first_name, first_index)) = names.get(&identity) {
            return Err(format!(
                "wire parameter name conflict: contract={} location={} first={} first_index={} second={} second_index={}",
                contract.id,
                transport_location_name(location),
                first_name,
                first_index,
                name,
                index
            ));
        }
        names.insert(identity, (name, index));
        let cardinality = parameter.cardinality.ok_or_else(|| {
            format!(
                "wire parameter missing cardinality: contract={} location={} parameter={} parameter_index={} expected=some actual=none",
                contract.id,
                transport_location_name(location),
                name,
                index
            )
        })?;
        if location == HttpTransportLocation::Path
            && cardinality != WireParameterCardinality::RequiredOne
        {
            return Err(format!(
                "path parameter cardinality must be required_one: contract={} location=path parameter={} expected=required_one actual={}",
                contract.id,
                name,
                wire_parameter_cardinality_name(cardinality)
            ));
        }
    }
    Ok(())
}

fn validate_obligation(
    endpoint: &EndpointDescriptor,
    kind: EndpointObligationKind,
    obligation: EndpointObligation,
    contracts: &std::collections::BTreeMap<&str, &crate::OperationContract>,
    require_closed: bool,
) -> Result<(), String> {
    if matches!(kind, EndpointObligationKind::Path)
        && endpoint.path.contains(':')
        && matches!(obligation, EndpointObligation::NotApplicable)
    {
        return Err(format!(
            "parameterized endpoint path must remain Todo or have a path contract: {}",
            endpoint.operation_id
        ));
    }
    match obligation {
        EndpointObligation::Todo => {
            if endpoint.migration == MigrationState::Adopted {
                return Err(format!(
                    "adopted endpoint cannot retain Todo: {} {}",
                    endpoint.operation_id,
                    kind.name()
                ));
            }
            if require_closed {
                return Err(format!(
                    "endpoint obligation Todo prevents closure: {} {}",
                    endpoint.operation_id,
                    kind.name()
                ));
            }
        }
        EndpointObligation::Excluded { reason } if reason.trim().is_empty() => {
            return Err(format!(
                "endpoint obligation exclusion reason must be non-empty: {} {}",
                endpoint.operation_id,
                kind.name()
            ));
        }
        EndpointObligation::Contract(contract_id) => {
            let contract = contracts.get(contract_id).ok_or_else(|| {
                format!(
                    "endpoint obligation references unknown contract: {} {} -> {}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id
                )
            })?;
            if matches!(
                contract.migration,
                MigrationState::Planned | MigrationState::Excluded
            ) {
                return Err(format!(
                    "endpoint obligation references {} contract: {} {} -> {}",
                    migration_state_name(contract.migration),
                    endpoint.operation_id,
                    kind.name(),
                    contract_id
                ));
            }
            if contract.binding != crate::ContractBinding::ExactSurface {
                return Err(format!(
                    "endpoint obligation requires ExactSurface contract: endpoint={} obligation={} contract={} expected=exact_surface actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_binding_name(contract.binding)
                ));
            }
            if contract.granularity != crate::ContractGranularity::Exact {
                return Err(format!(
                    "endpoint obligation requires exact granularity: endpoint={} obligation={} contract={} binding={} expected=exact actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_binding_name(contract.binding),
                    contract_granularity_name(contract.granularity)
                ));
            }
            let expected_direction = if kind.is_input() {
                crate::ContractDirection::Deserialize
            } else {
                crate::ContractDirection::Serialize
            };
            if contract.direction != expected_direction {
                return Err(format!(
                    "endpoint obligation contract has wrong direction: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_direction_name(expected_direction),
                    contract_direction_name(contract.direction)
                ));
            }
            if contract.surface != endpoint.surface {
                return Err(format!(
                    "endpoint obligation contract has wrong surface: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_surface_name(endpoint.surface),
                    contract_surface_name(contract.surface)
                ));
            }
            let (operation_key, location, parameters) = match contract.transport {
                crate::ContractTransport::Http {
                    operation_key,
                    location,
                    parameters,
                } => (operation_key, location, parameters),
                crate::ContractTransport::NoTransport => {
                    return Err(format!(
                        "endpoint obligation contract lacks HTTP transport: endpoint={} obligation={} contract={} expected=http actual=no_transport",
                        endpoint.operation_id,
                        kind.name(),
                        contract_id
                    ));
                }
            };
            let expected_location = kind.location();
            if location != expected_location {
                return Err(format!(
                    "contract location {} does not match obligation {}: endpoint={} obligation={} contract={} expected={} actual={}",
                    transport_location_name(location),
                    kind.name(),
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    transport_location_name(expected_location),
                    transport_location_name(location)
                ));
            }
            let endpoint_key = endpoint_operation_key(endpoint);
            if operation_key != Some(endpoint_key.as_str()) {
                return Err(format!(
                    "contract operation does not match endpoint: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    endpoint_key,
                    operation_key.unwrap_or("<none>")
                ));
            }
            if kind == EndpointObligationKind::Path {
                validate_path_parameter_mapping(endpoint, contract_id, parameters)?;
            }
            if kind == EndpointObligationKind::Sse && endpoint.surface != ContractSurface::Sse {
                return Err(format!(
                    "SSE contract obligation is only valid on SSE endpoint: {}",
                    endpoint.operation_id
                ));
            }
        }
        _ => {}
    }
    if kind == EndpointObligationKind::Sse
        && endpoint.surface != ContractSurface::Sse
        && !matches!(
            obligation,
            EndpointObligation::NotApplicable | EndpointObligation::Excluded { .. }
        )
    {
        return Err(format!(
            "non-SSE endpoint must mark SSE obligation NotApplicable or Excluded: {}",
            endpoint.operation_id
        ));
    }
    if kind == EndpointObligationKind::Sse
        && endpoint.surface == ContractSurface::Sse
        && matches!(obligation, EndpointObligation::NotApplicable)
    {
        return Err(format!(
            "SSE endpoint must describe SSE obligation: {}",
            endpoint.operation_id
        ));
    }
    Ok(())
}

fn validate_shared_component_links(
    endpoint: &EndpointDescriptor,
    contracts: &std::collections::BTreeMap<&str, &crate::OperationContract>,
) -> Result<(), String> {
    let mut linked = std::collections::BTreeMap::new();
    for (index, contract_id) in endpoint.shared_components.iter().enumerate() {
        if let Some(first_index) = linked.insert(*contract_id, index) {
            return Err(format!(
                "duplicate shared component link: endpoint={} contract={} first={} second={}",
                endpoint.operation_id, contract_id, first_index, index
            ));
        }
        let contract = contracts.get(contract_id).ok_or_else(|| {
            format!(
                "endpoint shared component references unknown contract: {} -> {}",
                endpoint.operation_id, contract_id
            )
        })?;
        if matches!(
            contract.migration,
            MigrationState::Planned | MigrationState::Excluded
        ) {
            return Err(format!(
                "endpoint shared component references {} contract: {} -> {}",
                migration_state_name(contract.migration),
                endpoint.operation_id,
                contract_id
            ));
        }
        if contract.binding != crate::ContractBinding::SharedComponent {
            return Err(format!(
                "shared component link requires SharedComponent contract: endpoint={} contract={} expected=shared_component actual={}",
                endpoint.operation_id,
                contract_id,
                contract_binding_name(contract.binding)
            ));
        }
        if contract.surface != endpoint.surface {
            return Err(format!(
                "shared component link has wrong surface: endpoint={} contract={} expected={} actual={}",
                endpoint.operation_id,
                contract_id,
                contract_surface_name(endpoint.surface),
                contract_surface_name(contract.surface)
            ));
        }
        if !matches!(contract.transport, crate::ContractTransport::Http { .. }) {
            return Err(format!(
                "shared component link lacks HTTP transport: endpoint={} contract={} expected=http actual=no_transport",
                endpoint.operation_id, contract_id
            ));
        }
    }
    Ok(())
}

fn validate_endpoint_migration_honesty(
    endpoint: &EndpointDescriptor,
    contracts: &std::collections::BTreeMap<&str, &crate::OperationContract>,
) -> Result<(), String> {
    let exact_contracts = endpoint
        .obligations
        .entries()
        .into_iter()
        .filter_map(|(_, obligation)| match obligation {
            EndpointObligation::Contract(contract_id) => contracts.get(contract_id).copied(),
            _ => None,
        })
        .filter(|contract| contract.binding == crate::ContractBinding::ExactSurface)
        .collect::<Vec<_>>();

    match endpoint.migration {
        MigrationState::Planned if !exact_contracts.is_empty() => Err(format!(
            "planned endpoint cannot claim exact contract coverage: {}",
            endpoint.operation_id
        )),
        MigrationState::Generated if exact_contracts.is_empty() => Err(format!(
            "generated endpoint requires at least one ExactSurface contract: {}",
            endpoint.operation_id
        )),
        MigrationState::Adopted if exact_contracts.is_empty() => Err(format!(
            "adopted endpoint requires at least one ExactSurface contract: {}",
            endpoint.operation_id
        )),
        MigrationState::Adopted
            if exact_contracts
                .iter()
                .any(|contract| contract.migration != MigrationState::Adopted) =>
        {
            Err(format!(
                "adopted endpoint references non-adopted exact contract: {}",
                endpoint.operation_id
            ))
        }
        MigrationState::Excluded
            if !endpoint.shared_components.is_empty() || !exact_contracts.is_empty() =>
        {
            Err(format!(
                "excluded endpoint cannot retain contract linkage: {}",
                endpoint.operation_id
            ))
        }
        _ => Ok(()),
    }
}

fn validate_path_parameter_mapping(
    endpoint: &EndpointDescriptor,
    contract_id: &str,
    parameters: &[crate::WireParameter],
) -> Result<(), String> {
    let placeholders = endpoint
        .path
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .collect::<Vec<_>>();
    let declared = parameters
        .iter()
        .map(|parameter| parameter.name)
        .collect::<Vec<_>>();
    if declared != placeholders {
        return Err(format!(
            "path parameter set does not match endpoint placeholders: endpoint={} obligation=path contract={} declared={declared:?} expected={placeholders:?}",
            endpoint.operation_id, contract_id
        ));
    }
    Ok(())
}

fn endpoint_operation_key(endpoint: &EndpointDescriptor) -> String {
    format!("{} {}", http_method_name(endpoint.method), endpoint.path)
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

fn transport_location_name(location: crate::HttpTransportLocation) -> &'static str {
    match location {
        crate::HttpTransportLocation::Path => "path",
        crate::HttpTransportLocation::Query => "query",
        crate::HttpTransportLocation::Headers => "headers",
        crate::HttpTransportLocation::Body => "body",
        crate::HttpTransportLocation::Success => "success",
        crate::HttpTransportLocation::Error => "error",
        crate::HttpTransportLocation::Sse => "sse",
    }
}

fn contract_direction_name(direction: crate::ContractDirection) -> &'static str {
    match direction {
        crate::ContractDirection::Serialize => "serialize",
        crate::ContractDirection::Deserialize => "deserialize",
        crate::ContractDirection::Bidirectional => "bidirectional",
    }
}

fn contract_surface_name(surface: crate::ContractSurface) -> &'static str {
    match surface {
        crate::ContractSurface::Api => "api",
        crate::ContractSurface::Cli => "cli",
        crate::ContractSurface::Jsonl => "jsonl",
        crate::ContractSurface::Sse => "sse",
        crate::ContractSurface::Metadata => "metadata",
        crate::ContractSurface::Config => "config",
    }
}

fn contract_binding_name(binding: crate::ContractBinding) -> &'static str {
    match binding {
        crate::ContractBinding::ExactSurface => "exact_surface",
        crate::ContractBinding::SharedComponent => "shared_component",
    }
}

fn contract_granularity_name(granularity: crate::ContractGranularity) -> &'static str {
    match granularity {
        crate::ContractGranularity::Exact => "exact",
        crate::ContractGranularity::Family => "family",
    }
}

fn wire_parameter_cardinality_name(cardinality: crate::WireParameterCardinality) -> &'static str {
    match cardinality {
        crate::WireParameterCardinality::RequiredOne => "required_one",
        crate::WireParameterCardinality::OptionalOne => "optional_one",
        crate::WireParameterCardinality::RepeatedOrdered => "repeated_ordered",
    }
}

fn migration_state_name(state: MigrationState) -> &'static str {
    match state {
        MigrationState::Planned => "planned",
        MigrationState::Generated => "generated",
        MigrationState::Adopted => "adopted",
        MigrationState::Excluded => "excluded",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointObligationKind {
    Path,
    Query,
    Headers,
    Body,
    Success,
    Sse,
}

impl EndpointObligationKind {
    const fn is_input(self) -> bool {
        matches!(self, Self::Path | Self::Query | Self::Headers | Self::Body)
    }
    const fn name(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Headers => "headers",
            Self::Body => "body",
            Self::Success => "success",
            Self::Sse => "sse",
        }
    }
    pub(crate) const fn location(self) -> crate::HttpTransportLocation {
        match self {
            Self::Path => crate::HttpTransportLocation::Path,
            Self::Query => crate::HttpTransportLocation::Query,
            Self::Headers => crate::HttpTransportLocation::Headers,
            Self::Body => crate::HttpTransportLocation::Body,
            Self::Success => crate::HttpTransportLocation::Success,
            Self::Sse => crate::HttpTransportLocation::Sse,
        }
    }
}

impl EndpointObligations {
    pub const fn entries(self) -> [(EndpointObligationKind, EndpointObligation); 6] {
        [
            (EndpointObligationKind::Path, self.path),
            (EndpointObligationKind::Query, self.query),
            (EndpointObligationKind::Headers, self.headers),
            (EndpointObligationKind::Body, self.body),
            (EndpointObligationKind::Success, self.success),
            (EndpointObligationKind::Sse, self.sse),
        ]
    }
}
