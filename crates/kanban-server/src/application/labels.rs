use super::support::request_actor;
use crate::application::support::CallContext;
use crate::{
    application::tasks::support::{api_label, api_task},
    error::ApiError,
    state::AppState,
};
use kanban_protocol::{
    AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
    BootstrapTaskLabelRequest, BootstrapTaskLabelResponse, CreateBoardLabelRequest,
    CreateBoardLabelResponse, DataEnvelope, DeleteBoardLabelPath, DeleteBoardLabelQuery,
    DeleteBoardLabelResponse, DeleteBoardLabelResult, LabelAtomWire, LabelSemanticsWire,
    ListBoardLabelsResponse, ListTaskLabelsPath, ListTaskLabelsResponse, RemoveTaskLabelPath,
    RemoveTaskLabelResponse, TaskLabelSurfacePath,
};
use kanban_service::KanbanError;
use kanban_service::{
    AddTaskLabelsCommand, BootstrapTaskLabelCommand, CreateBoardLabelCommand,
    DeleteBoardLabelCommand, RemoveTaskLabelCommand,
};
pub(crate) async fn list_board_labels(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
) -> Result<ListBoardLabelsResponse, ApiError> {
    let labels = state.application().list_board_labels(&board).await?;
    Ok(DataEnvelope {
        data: labels.into_iter().map(api_label).collect(),
    })
}
pub(crate) async fn create_board_label(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
    body: CreateBoardLabelRequest,
) -> Result<CreateBoardLabelResponse, ApiError> {
    let label = state
        .application()
        .create_board_label(CreateBoardLabelCommand {
            board,
            name: body.name,
            color: body.color,
        })
        .await?;
    Ok(DataEnvelope {
        data: api_label(label),
    })
}
pub(crate) async fn delete_board_label(
    state: AppState,
    DeleteBoardLabelPath { board, label_id }: DeleteBoardLabelPath,
    headers: CallContext,
    query: DeleteBoardLabelQuery,
) -> Result<DeleteBoardLabelResponse, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let record = state
        .application()
        .delete_board_label(DeleteBoardLabelCommand {
            board,
            label_ref: label_id,
            force: query.force,
            actor,
        })
        .await?;
    Ok(DataEnvelope {
        data: DeleteBoardLabelResult {
            label: api_label(record.label),
            forced: record.forced,
            removed_task_bindings: record.removed_task_bindings,
            removed_semantics: record.removed_semantics,
            removed_atoms: record.removed_atoms,
        },
    })
}
pub(crate) async fn list_task_labels(
    state: AppState,
    ListTaskLabelsPath { task_id }: ListTaskLabelsPath,
) -> Result<ListTaskLabelsResponse, ApiError> {
    let labels = state.application().list_task_labels(&task_id).await?;
    Ok(ListTaskLabelsResponse {
        data: labels.into_iter().map(api_label).collect(),
    })
}
pub(crate) async fn add_task_labels(
    state: AppState,
    AddTaskLabelPath { task_id }: AddTaskLabelPath,
    headers: CallContext,
    body: AddTaskLabelRequest,
) -> Result<AddTaskLabelResponse, ApiError> {
    let names = body
        .label_names()
        .map_err(|error| KanbanError::InvalidInput(error.to_owned()))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let record = state
        .application()
        .add_task_labels(AddTaskLabelsCommand {
            task_id,
            names,
            create_missing: body.create_missing,
            actor,
        })
        .await?;
    let meta = (!record.created_labels.is_empty()).then(|| kanban_protocol::CreatedLabelsMeta {
        created_labels: record.created_labels.into_iter().map(api_label).collect(),
    });
    Ok(AddTaskLabelResponse {
        data: api_task(record.task)?,
        meta,
    })
}
pub(crate) async fn remove_task_label(
    state: AppState,
    RemoveTaskLabelPath { task_id, label_id }: RemoveTaskLabelPath,
    headers: CallContext,
) -> Result<RemoveTaskLabelResponse, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let task = state
        .application()
        .remove_task_label(RemoveTaskLabelCommand {
            task_id,
            label_ref: label_id,
            actor,
        })
        .await?;
    Ok(RemoveTaskLabelResponse {
        data: api_task(task)?,
    })
}
pub(crate) async fn bootstrap_task_label(
    state: AppState,
    TaskLabelSurfacePath { task_id }: TaskLabelSurfacePath,
    headers: CallContext,
    body: BootstrapTaskLabelRequest,
) -> Result<BootstrapTaskLabelResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let record = state
        .application()
        .bootstrap_task_label(BootstrapTaskLabelCommand {
            task_id,
            name: body.name,
            description: body.description,
            applies_when: body.applies_when,
            excludes_when: body.excludes_when,
            positive_examples: body.positive_examples,
            negative_examples: body.negative_examples,
            actor,
            verify: body.verify,
            min_verify_score: body.min_verify_score,
            vector_config: body.vector_config.map(|config| {
                kanban_service::VectorConfigureCommand {
                    provider: config.provider,
                    endpoint: config.endpoint,
                    model: config.model,
                    dimensions: config.dimensions,
                }
            }),
        })
        .await?;
    Ok(DataEnvelope {
        data: kanban_protocol::BootstrapTaskLabelData {
            task: api_task(record.task)?,
            semantics: api_semantics(record.semantics),
            verification: record.verification.map(api_verification),
        },
    })
}
fn api_verification(
    value: kanban_service::BootstrapTaskLabelVerification,
) -> kanban_protocol::BootstrapTaskLabelVerification {
    kanban_protocol::BootstrapTaskLabelVerification {
        label_name: value.label_name,
        score: value.score,
        source: value.source,
        min_score: value.min_score,
        degraded: value.degraded,
        diagnostics: value.diagnostics,
    }
}
fn api_semantics(value: kanban_service::LabelSemanticsRecord) -> LabelSemanticsWire {
    LabelSemanticsWire {
        label_id: value.label_id,
        board_id: value.board_id,
        label_name: value.label_name,
        semantics_hash: value.semantics_hash,
        description: value.description,
        applies_when: value.applies_when,
        excludes_when: value.excludes_when,
        positive_examples: value.positive_examples,
        negative_examples: value.negative_examples,
        created_at: value.created_at,
        updated_at: value.updated_at,
        atoms: value
            .atoms
            .into_iter()
            .map(|atom| LabelAtomWire {
                id: atom.id,
                label_id: atom.label_id,
                board_id: atom.board_id,
                label_name: atom.label_name,
                polarity: atom.polarity,
                kind: atom.kind,
                text: atom.text,
                ordinal: atom.ordinal,
                content_hash: atom.content_hash,
                created_at: atom.created_at,
                updated_at: atom.updated_at,
            })
            .collect(),
    }
}
