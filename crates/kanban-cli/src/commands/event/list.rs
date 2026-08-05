use clap::Args;
use kanban_client::KanbanClient;
use kanban_protocol::{CliEvent, CliEventsOutput, ListEventsQuery};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    pub(crate) task_ref: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub(crate) after: i64,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ListArgs,
) -> Result<(), CliFailure> {
    let task_id = args
        .task_ref
        .as_deref()
        .map(|selector| client.resolve_task_id(&ctx.board, selector))
        .transpose()?;
    let response = client.list_events(&ListEventsQuery {
        board: ctx.board.clone(),
        task_id,
        after: args.after,
        limit: args.limit,
    })?;

    let events = response
        .data
        .into_iter()
        .map(|event| CliEvent {
            id: event.id,
            event_id: event.event_id,
            task_id: event.task_id,
            run_id: event.run_id,
            kind: event.kind,
            actor: event.actor,
            payload: serde_json::to_value(event.payload)
                .expect("event payloads are serializable contract values"),
            created_at: event.created_at,
        })
        .collect::<Vec<_>>();

    if ctx.json {
        output::print_json(&CliEventsOutput::new(events));
    } else {
        for event in events {
            println!(
                "{} {} {}",
                event.id,
                event.kind,
                event.task_id.as_deref().unwrap_or("-")
            );
        }
    }
    Ok(())
}
